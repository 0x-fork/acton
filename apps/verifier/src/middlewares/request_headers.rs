use axum::{extract::Request, http::header::USER_AGENT, middleware::Next, response::Response};

use crate::error::ApiError;

pub async fn require_user_agent(request: Request, next: Next) -> Result<Response, ApiError> {
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());

    if user_agent.is_none_or(|value| value.trim().is_empty()) {
        return Err(ApiError::bad_request(
            "a non-empty User-Agent header is required".to_owned(),
        ));
    }

    Ok(next.run(request).await)
}
