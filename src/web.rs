use axum::Router;
use tokio::net::TcpListener;

/// Runs the web server for our domain.
pub(crate) async fn serve() {
    let listener = TcpListener::bind("0.0.0.0:80").await.unwrap();

    let app = Router::new();

    println!("Web server ready!");
    axum::serve(listener, app).await.unwrap();
}
