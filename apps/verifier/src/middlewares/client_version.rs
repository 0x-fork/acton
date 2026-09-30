use axum::{extract::Request, middleware::Next, response::Response};

use crate::{
    client_compatibility::{BLUEPRINT_MIN_VERSION, is_unsupported_blueprint_client},
    error::ApiError,
};

pub async fn require_supported_blueprint(
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if is_unsupported_blueprint_client(request.headers()) {
        return Err(ApiError::bad_request(format!(
            "This version of Blueprint is no longer supported. Update @ton/blueprint to version {BLUEPRINT_MIN_VERSION} or newer"
        )));
    }

    Ok(next.run(request).await)
}
