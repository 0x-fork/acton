use axum::{extract::Request, http::header::USER_AGENT, middleware::Next, response::Response};
use tracing::{Instrument, info_span};
use uuid::Uuid;

use super::request_headers::ACTON_CLIENT_HEADER;

pub async fn enter_request_span(mut request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4();
    request.extensions_mut().insert(request_id);
    let client = request
        .headers()
        .get(ACTON_CLIENT_HEADER)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
        });
    let span = info_span!(
        "request",
        %request_id,
        client,
    );

    next.run(request).instrument(span).await
}

#[cfg(test)]
mod tests {
    use axum::{Extension, Router, body::Body, middleware, routing::get};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn inserts_request_id() {
        let app = Router::new()
            .route(
                "/",
                get(|Extension(request_id): Extension<Uuid>| async move { request_id.to_string() }),
            )
            .layer(middleware::from_fn(enter_request_span));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert!(Uuid::parse_str(std::str::from_utf8(&body).unwrap()).is_ok());
    }
}
