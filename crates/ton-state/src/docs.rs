//! Interactive documentation for the methods served by this process.

#[cfg(test)]
mod tests;

use axum::response::{Html, Redirect};
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

use crate::{api, streaming, submit};

#[derive(OpenApi)]
#[openapi(
    info(title = "TON State API", version = env!("CARGO_PKG_VERSION")),
    servers((url = "/")),
    components(schemas(streaming::AccountStateEvent, streaming::StorageUpdateEvent)),
    paths(
        api::masterchain_info,
        api::address_information,
        api::address_balance,
        api::transactions::get_transactions,
        submit::send_boc,
        submit::send_boc_and_wait_transaction,
        submit::send_boc_and_wait_trace,
        streaming::subscribe,
    ),
)]
struct ApiDoc;

/// Serves one flat reference with requests directed to the current HTTP server.
pub(crate) fn router() -> Router {
    let mut document = ApiDoc::openapi();

    // Rust module names are not API categories.
    for path in document.paths.paths.values_mut() {
        for operation in [&mut path.get, &mut path.post].into_iter().flatten() {
            operation.tags = None;
        }
    }

    Router::new()
        .route("/", get(|| async { Redirect::temporary("/docs") }))
        .route("/docs", get(|| async { Html(include_str!("docs.html")) }))
        .route(
            "/account",
            get(|| async { Html(include_str!("account.html")) }),
        )
        .route(
            "/storage",
            get(|| async { Html(include_str!("storage.html")) }),
        )
        .route(
            "/storage-decoder.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("storage-decoder.js"),
                )
            }),
        )
        .route("/openapi.json", get(move || async { Json(document) }))
}
