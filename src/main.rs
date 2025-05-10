mod ns;
mod web;

#[tokio::main]
async fn main() {
    let ns_server = tokio::spawn(ns::serve());
    let web_server = tokio::spawn(web::serve());

    tokio::try_join!(ns_server, web_server).unwrap();
}
