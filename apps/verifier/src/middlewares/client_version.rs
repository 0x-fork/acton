use axum::{extract::Request, http::header::USER_AGENT, middleware::Next, response::Response};
use semver::Version;

use crate::{
    client_compatibility::BLUEPRINT_MIN_VERSION,
    error::ApiError,
};

pub async fn require_supported_blueprint(
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let user_agents = request
        .headers()
        .get_all(USER_AGENT)
        .iter()
        .map(|value| value.to_str().unwrap_or("<invalid>"))
        .collect::<Vec<_>>();

    let unsupported = user_agents
        .iter()
        .filter_map(|user_agent| {
            let (client, version) = user_agent.split_whitespace().next()?.split_once('/')?;
            Some((client, Version::parse(version).ok()?))
        })
        .any(|(client, version)| {
            client.eq_ignore_ascii_case("blueprint")
                && version.cmp_precedence(&BLUEPRINT_MIN_VERSION).is_lt()
        });

    tracing::info!(
        method = %request.method(),
        path = %request.uri().path(),
        user_agents = ?user_agents,
        unsupported_blueprint_client = unsupported,
        "checked Blueprint client version"
    );

    if unsupported {
        return Err(ApiError::bad_request(format!(
            "This version of Blueprint is no longer supported. Update @ton/blueprint to version {BLUEPRINT_MIN_VERSION} or newer"
        )));
    }

    Ok(next.run(request).await)
}
