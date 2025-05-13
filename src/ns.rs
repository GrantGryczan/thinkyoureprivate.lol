use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use gethostname::gethostname;
use hickory_server::{
    ServerFuture,
    authority::{Catalog, ZoneType},
    proto::rr::{
        Name, RData, Record, RecordSet, RecordType, RrKey,
        rdata::{self, SOA},
    },
    server::{Request, RequestHandler, ResponseHandler, ResponseInfo},
    store::in_memory::InMemoryAuthority,
};
use tokio::net::{TcpListener, UdpSocket};

/// Runs the authoritative name server for our domain.
pub(crate) async fn serve() {
    let socket = UdpSocket::bind("0.0.0.0:53").await.unwrap();
    let listener = TcpListener::bind("0.0.0.0:53").await.unwrap();

    let zone_name = Name::from_str(gethostname().to_string_lossy().as_ref())
        .expect("hostname from `compose.override.yaml` should be a valid DNS zone name")
        .append_name(&Name::root())
        .unwrap();

    let catalog = build_catalog(&zone_name).await;
    let handler = Handler { zone_name, catalog };

    let mut server = ServerFuture::new(handler);
    server.register_socket(socket);
    server.register_listener(listener, Duration::from_secs(5));

    println!("Name server ready!");
    server.block_until_done().await.unwrap();
}

macro_rules! insert_ip_addr_record_sets {
    ($rtype:ident, $records:expr, $zone_name:expr, $wildcard_name:expr, $ip_addr:expr$(,)?) => {
        let records: &mut BTreeMap<RrKey, RecordSet> = $records;
        let zone_name: Name = $zone_name;
        let wildcard_name: Name = $wildcard_name;

        let rdata = RData::$rtype(rdata::$rtype($ip_addr));

        const TTL: u32 = 60;

        // The hell is up with all this repetition? This is difficult to read. Why did they design
        // their API like this? Surely there's a nicer API I don't know about? Then I might not need
        // this macro.
        let mut record_set = RecordSet::new(zone_name.clone(), RecordType::$rtype, 0);
        record_set.insert(Record::from_rdata(zone_name.clone(), TTL, rdata.clone()), 0);
        records.insert(RrKey::new(zone_name.into(), RecordType::$rtype), record_set);

        let mut record_set = RecordSet::new(wildcard_name.clone(), RecordType::$rtype, 0);
        record_set.insert(Record::from_rdata(wildcard_name.clone(), TTL, rdata), 0);
        records.insert(
            RrKey::new(wildcard_name.into(), RecordType::$rtype),
            record_set,
        );
    };
}

fn insert_soa_record_set(records: &mut BTreeMap<RrKey, RecordSet>, zone_name: Name) {
    let mname = zone_name.prepend_label("ns").unwrap();
    let rname = zone_name.prepend_label("admin").unwrap();
    const SERIAL: u32 = 1;
    const REFRESH: u32 = 60 * 60 * 24;
    const RETRY: i32 = 60;
    const EXPIRE: i32 = 60 * 60 * 24 * 30;
    const MINIMUM: u32 = 0;

    let rdata = RData::SOA(SOA::new(
        mname,
        rname,
        SERIAL,
        REFRESH as i32,
        RETRY,
        EXPIRE,
        MINIMUM,
    ));

    let mut record_set = RecordSet::new(zone_name.clone(), RecordType::SOA, 0);
    record_set.insert(Record::from_rdata(zone_name.clone(), REFRESH, rdata), 0);

    records.insert(RrKey::new(zone_name.into(), RecordType::SOA), record_set);
}

async fn build_catalog(zone_name: &Name) -> Catalog {
    let ipv4_addr_request = async {
        match reqwest::get("https://ipv4.icanhazip.com/").await {
            Ok(response) => Ok(response
                .error_for_status()
                .expect("IPv4 address response status should be successful")
                .text()
                .await
                .expect("IPv4 address response body should be text")
                .trim()
                .parse::<Ipv4Addr>()
                .expect("IPv4 address response should be a valid IPv4 address")),
            Err(error) => Err(error),
        }
    };

    let ipv6_addr_request = async {
        match reqwest::get("https://ipv6.icanhazip.com/").await {
            Ok(response) => Ok(response
                .error_for_status()
                .expect("IPv6 address response status should be successful")
                .text()
                .await
                .expect("IPv6 address response body should be text")
                .trim()
                .parse::<Ipv6Addr>()
                .expect("IPv6 address response should be a valid IPv6 address")),
            Err(error) => Err(error),
        }
    };

    let (ipv4_addr, ipv6_addr) = tokio::join!(ipv4_addr_request, ipv6_addr_request);

    if let (Err(ipv4_error), Err(ipv6_error)) = (&ipv4_addr, &ipv6_addr) {
        panic!(
            "couldn't obtain public IP address;\n\
            IPv4 request failed: {ipv4_error};\n\
            IPv6 request failed: {ipv6_error}"
        );
    }

    let mut records = BTreeMap::new();
    let wildcard_name = zone_name.prepend_label("*").unwrap();

    if let Ok(ipv4_addr) = ipv4_addr {
        insert_ip_addr_record_sets!(
            A,
            &mut records,
            zone_name.clone(),
            wildcard_name.clone(),
            ipv4_addr,
        );
    }

    if let Ok(ipv6_addr) = ipv6_addr {
        insert_ip_addr_record_sets!(
            AAAA,
            &mut records,
            zone_name.clone(),
            wildcard_name,
            ipv6_addr,
        );
    }

    insert_soa_record_set(&mut records, zone_name.clone());

    let authority =
        InMemoryAuthority::new(zone_name.clone(), records, ZoneType::Primary, false).unwrap();

    let mut catalog = Catalog::new();
    catalog.upsert(zone_name.into(), vec![Arc::new(authority)]);
    catalog
}

/// A DNS [`RequestHandler`] that mirrors the behavior of a [`Catalog`] but also records which DNS
/// provider made a request.
struct Handler {
    zone_name: Name,
    catalog: Catalog,
}

#[async_trait::async_trait]
impl RequestHandler for Handler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        response_handle: R,
    ) -> ResponseInfo {
        for query in request.queries() {
            let name = query.name().to_string();
            let Some(subdomain) = name.strip_suffix(&format!(".{}", self.zone_name)) else {
                continue;
            };

            let ip = request.src().ip();

            dbg!(subdomain, ip);
        }

        self.catalog.handle_request(request, response_handle).await
    }
}
