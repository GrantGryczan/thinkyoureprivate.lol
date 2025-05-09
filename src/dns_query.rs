use axum::{extract::Query, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PostQuery {
    subdomain: String,
}

pub(crate) async fn handle(Query(query): Query<PostQuery>, ip_addr: String) -> impl IntoResponse {
    println!("{}\n{}", query.subdomain, ip_addr);

    StatusCode::OK
}
