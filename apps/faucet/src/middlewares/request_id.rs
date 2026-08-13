use axum::{extract::Request, middleware::Next, response::Response};
use tracing::{Instrument, info_span};
use uuid::Uuid;

pub async fn enter_request_span(mut request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4();
    request.extensions_mut().insert(request_id);

    next.run(request)
        .instrument(info_span!("request", %request_id))
        .await
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
