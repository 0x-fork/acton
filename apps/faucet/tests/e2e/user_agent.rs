use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header::USER_AGENT},
    middleware,
    routing::get,
};
use faucet_backend::middlewares::require_acton_user_agent;
use tower::ServiceExt;

#[tokio::test]
async fn requires_acton_user_agent_on_protected_route() {
    let response = request_with_user_agent(Some("acton/0.1.0")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_body(response).await, "ok");

    let response = request_with_user_agent(None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_user_agent(Some("acton/")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_user_agent(Some("faucet/0.1.0")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

async fn request_with_user_agent(user_agent: Option<&str>) -> axum::response::Response {
    let app = Router::new()
        .route("/challenge", get(|| async { "ok" }))
        .route_layer(middleware::from_fn(require_acton_user_agent));

    let mut request = Request::builder().uri("/challenge");

    if let Some(user_agent) = user_agent {
        request = request.header(USER_AGENT, user_agent);
    }

    app.oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
