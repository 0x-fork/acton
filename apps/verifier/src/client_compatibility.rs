use axum::http::{HeaderMap, header::USER_AGENT};
use semver::Version;

const ACTON_LEGACY_MAX_VERSION: Version = Version::new(1, 2, 0);
const BLUEPRINT_LEGACY_MAX_VERSION: Version = Version::new(0, 46, 0);

/// Clients that predate compiler admission checks keep the previous API behavior.
pub fn is_legacy_client(headers: &HeaderMap) -> bool {
    let mut user_agents = headers.get_all(USER_AGENT).iter();
    let Some(user_agent) = user_agents.next().and_then(|value| value.to_str().ok()) else {
        return false;
    };

    if user_agents.next().is_some() {
        return false;
    }

    let Some((client, version)) = user_agent
        .split_whitespace()
        .next()
        .and_then(|product| product.split_once('/'))
    else {
        return false;
    };

    let Ok(version) = Version::parse(version) else {
        return false;
    };

    if client.eq_ignore_ascii_case("acton") {
        return version.cmp_precedence(&ACTON_LEGACY_MAX_VERSION).is_le();
    }

    if client.eq_ignore_ascii_case("blueprint") {
        return version
            .cmp_precedence(&BLUEPRINT_LEGACY_MAX_VERSION)
            .is_le();
    }

    false
}
