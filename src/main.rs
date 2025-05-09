use axum::{Router, routing::post};
use tokio::net::TcpListener;

mod dns_query;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/dns-query", post(dns_query::handle));

    let listener = TcpListener::bind("0.0.0.0:80").await.unwrap();

    println!("Ready!");

    axum::serve(listener, app).await.unwrap();
}
