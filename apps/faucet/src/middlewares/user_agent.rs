use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode, header::USER_AGENT},
    middleware::Next,
    response::{IntoResponse, Response},
};

const ALLOWED_USER_AGENT_PREFIX: &str = "acton/";

pub async fn require_acton_user_agent(request: Request, next: Next) -> Response {
    if request
        .headers()
        .get(USER_AGENT)
        .is_some_and(is_allowed_user_agent)
    {
        return next.run(request).await;
    }

    StatusCode::BAD_REQUEST.into_response()
}

fn is_allowed_user_agent(value: &HeaderValue) -> bool {
    value.to_str().is_ok_and(|user_agent| {
        user_agent
            .strip_prefix(ALLOWED_USER_AGENT_PREFIX)
            .is_some_and(|version| !version.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::is_allowed_user_agent;

    #[test]
    fn allows_acton_package_version_user_agent() {
        assert!(is_allowed_user_agent(&"acton/0.1.0".parse().unwrap()));
        assert!(is_allowed_user_agent(
            &"acton/1.2.3-beta.1+build.5".parse().unwrap()
        ));
        assert!(is_allowed_user_agent(
            &"acton/0.1.0 (debug)".parse().unwrap()
        ));
    }

    #[test]
    fn rejects_missing_or_non_acton_version() {
        assert!(!is_allowed_user_agent(&"acton/".parse().unwrap()));
        assert!(!is_allowed_user_agent(&"faucet/0.1.0".parse().unwrap()));
    }
}
