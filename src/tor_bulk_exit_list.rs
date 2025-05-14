use std::{
    collections::BTreeSet,
    net::IpAddr,
    time::{Duration, Instant},
};

use tokio::sync::RwLock;

/// A cached representation of the Tor bulk exit list from
/// https://check.torproject.org/torbulkexitlist.
#[derive(Clone, Debug)]
pub(crate) struct TorBulkExitList {
    pub(crate) last_updated_at: Instant,
    ip_addrs: BTreeSet<IpAddr>,
}

impl TorBulkExitList {
    /// How long the data upstream can be cached for.
    pub(crate) const MAX_AGE: Duration = Duration::from_secs(60 * 30);

    /// Creates a new `TorBulkExitList` by requesting current upstream data.
    pub(crate) async fn new() -> reqwest::Result<Self> {
        let ip_addrs = reqwest::get("https://check.torproject.org/torbulkexitlist")
            .await?
            .text()
            .await?
            .lines()
            .map(|line| {
                line.parse()
                    .expect("Tor bulk exit list IP address should be valid")
            })
            .collect();

        Ok(Self {
            last_updated_at: Instant::now(),
            ip_addrs,
        })
    }

    /// Updates the `TorBulkExitList` by requesting current upstream data.
    pub(crate) async fn update(&mut self) -> reqwest::Result<()> {
        *self = TorBulkExitList::new().await?;

        Ok(())
    }

    /// Checks if the current version of the `TorBulkExitList` in memory contains a particular
    /// [`IpAddr`].
    pub(crate) fn cache_contains(&self, ip_addr: &IpAddr) -> bool {
        self.ip_addrs.contains(ip_addr)
    }
}

pub(crate) trait TorBulkExitListContains {
    /// Checks if a [`TorBulkExitList`] contains a particular [`IpAddr`], updating the list first if
    /// it's older than [`TorBulkExitList::MAX_AGE`]. If an update request fails, the cached version
    /// in memory is checked instead.
    async fn contains(&self, ip_addr: &IpAddr) -> bool;
}

impl TorBulkExitListContains for RwLock<TorBulkExitList> {
    async fn contains(&self, ip_addr: &IpAddr) -> bool {
        let list = self.read().await;

        if list.last_updated_at.elapsed() <= TorBulkExitList::MAX_AGE {
            return list.cache_contains(ip_addr);
        }

        // Prevent a deadlock where the below write guard would wait for this read guard to be
        // dropped at the end of this function.
        drop(list);

        let mut list = self.write().await;
        let _ = list.update().await;

        list.cache_contains(ip_addr)
    }
}
