use axum::http::{HeaderMap, header::USER_AGENT};
use semver::Version;

const ACTON_LEGACY_MAX_VERSION: Version = Version::new(1, 2, 0);
pub const BLUEPRINT_MIN_VERSION: Version = Version::new(0, 47, 1);

/// Acton clients that predate compiler admission checks keep the previous API behavior.
pub fn is_legacy_client(headers: &HeaderMap) -> bool {
    let mut user_agents = headers.get_all(USER_AGENT).iter();
    let Some(user_agent) = user_agents.next().and_then(|value| value.to_str().ok()) else {
        return false;
    };

    if user_agents.next().is_some() {
        return false;
    }

    let Some((client, version)) = client_version(user_agent) else {
        return false;
    };

    client.eq_ignore_ascii_case("acton")
        && version.cmp_precedence(&ACTON_LEGACY_MAX_VERSION).is_le()
}

pub fn is_unsupported_blueprint_client(headers: &HeaderMap) -> bool {
    headers
        .get_all(USER_AGENT)
        .iter()
        .filter_map(|value| value.to_str().ok().and_then(client_version))
        .any(|(client, version)| {
            client.eq_ignore_ascii_case("blueprint")
                && version.cmp_precedence(&BLUEPRINT_MIN_VERSION).is_lt()
        })
}

fn client_version(user_agent: &str) -> Option<(&str, Version)> {
    let (client, version) = user_agent.split_whitespace().next()?.split_once('/')?;
    Some((client, Version::parse(version).ok()?))
}
