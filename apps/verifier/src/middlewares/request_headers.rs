use axum::{extract::Request, http::header::USER_AGENT, middleware::Next, response::Response};

use crate::error::ApiError;

pub async fn require_user_agent(request: Request, next: Next) -> Result<Response, ApiError> {
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let accepted = user_agent.is_some_and(|value| !value.trim().is_empty());
    tracing::info!(
        method = %request.method(),
        path = %request.uri().path(),
        user_agent = ?user_agent,
        accepted,
        "checked User-Agent header"
    );

    if !accepted {
        return Err(ApiError::bad_request(
            "a non-empty User-Agent header is required".to_owned(),
        ));
    }

    Ok(next.run(request).await)
}
