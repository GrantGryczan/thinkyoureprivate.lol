use std::{net::SocketAddr, sync::Arc};

use askama::Template;
use axum::{
    Router,
    extract::{ConnectInfo, State},
    response::{Html, IntoResponse},
    routing::get,
};
use memory_serve::{MemoryServe, load_assets};
use tokio::{net::TcpListener, sync::RwLock};

use crate::tor_bulk_exit_list::{TorBulkExitList, TorBulkExitListContains};

#[derive(Clone, Debug)]
struct AppState {
    /// A cached representation of the Tor bulk exit list from
    /// https://check.torproject.org/torbulkexitlist.
    tor_bulk_exit_list: Arc<RwLock<TorBulkExitList>>,
}

/// Runs the web server for our domain.
pub(crate) async fn serve() {
    let listener = TcpListener::bind("0.0.0.0:80").await.unwrap();

    let state = AppState {
        tor_bulk_exit_list: RwLock::new(TorBulkExitList::new().await.unwrap()).into(),
    };

    let asset_router = MemoryServe::new(load_assets!("assets"))
        .index_on_subdirectories(true)
        .into_router();
    let app = Router::new()
        .route("/display-your-info", get(display_your_info))
        .with_state(state)
        .fallback_service(asset_router)
        .into_make_service_with_connect_info::<SocketAddr>();

    println!("Web server ready!");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Template)]
#[template(path = "display-your-info.html")]
struct DisplayYourInfoTemplate<'a> {
    ip: &'a str,
    is_tor: bool,
}

async fn display_your_info(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let ip = addr.ip();
    let is_tor = state.tor_bulk_exit_list.contains(&ip).await;

    Html(
        DisplayYourInfoTemplate {
            ip: &ip.to_string(),
            is_tor,
        }
        .render()
        .unwrap(),
    )
}
