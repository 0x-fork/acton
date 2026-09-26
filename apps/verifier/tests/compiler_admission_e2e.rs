mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use verifier::{app, compiler_policy::CompilerPolicy, payment::PaymentError, state::AppState};

use support::{
    MultipartPart, app_state, owned_file_part, owned_text_part, payment_error_app_state,
    post_verify, post_verify_with_api_key, post_verify_with_user_agent, recording_app_state,
    recording_payment_app_state, recovering_payment_app_state, response_json, text_part,
};

const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LEGACY_USER_AGENTS: &[&str] = &[
    "acton/0.46.0",
    "acton/1.0.0",
    "acton/1.1.99",
    "acton/1.2.0-rc.1",
    "acton/1.2.0",
    "acton/1.2.0+build.1",
    "acton/1.1.0+build.1",
    "ACTON/1.1.0 (linux)",
    "blueprint/0.1.0",
    "blueprint/0.45.0",
    "blueprint/0.46.0-rc.1",
    "blueprint/0.46.0",
    "blueprint/0.46.0+build.1",
    "blueprint/0.46.0 node/24.0.0",
];
const NON_LEGACY_USER_AGENTS: &[Option<&str>] = &[
    None,
    Some(""),
    Some("acton/1.2.1-rc.1"),
    Some("acton/1.2.1"),
    Some("acton/1.10.0"),
    Some("acton/2.0.0"),
    Some("blueprint/0.46.1"),
    Some("blueprint/0.47.0"),
    Some("blueprint/0.46.1-rc.1"),
    Some("acton/garbage"),
    Some("acton/1.1"),
    Some("blueprint/0.46.0oops"),
    Some("curl/8.0.0"),
    Some("unknown/1.1.0"),
    Some("proxy/1.0 acton/1.1.0"),
];

fn policy(entries: &[&str]) -> CompilerPolicy {
    CompilerPolicy::from_disabled(
        &entries
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("compiler policy")
}

async fn ticket(state: AppState, body: Value, user_agent: Option<&str>) -> Response {
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/take_ticket")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(user_agent) = user_agent {
        request = request.header(header::USER_AGENT, user_agent);
    }
    app::router_with_state(state)
        .oneshot(
            request
                .body(Body::from(body.to_string()))
                .expect("ticket request"),
        )
        .await
        .expect("ticket response")
}

fn parts(language: &str, version: &str) -> Vec<MultipartPart> {
    let (path, content) = match language.trim().to_ascii_lowercase().as_str() {
        "func" => ("main.fc", "() recv_internal() {}".to_owned()),
        "tact" => (
            "main.pkg",
            json!({
                "compiler": {"version": version, "parameters": {"entrypoint": "main.tact"}}
            })
            .to_string(),
        ),
        _ => ("main.tolk", "fun main() {}".to_owned()),
    };
    vec![
        text_part("code_hash", CODE_HASH),
        owned_text_part("language", language),
        owned_text_part(
            "compile_params",
            json!({"compiler_version": version}).to_string(),
        ),
        owned_text_part(
            "sources",
            json!([{"path": path, "is_entrypoint": true}]).to_string(),
        ),
        owned_file_part("files", path, "text/plain", content),
    ]
}

#[tokio::test]
async fn only_legacy_clients_can_omit_ticket_compiler_metadata() {
    for &user_agent in LEGACY_USER_AGENTS {
        for body in [
            json!({"code_hash": CODE_HASH}),
            json!({"code_hash": CODE_HASH, "compiler": null, "compiler_version": null}),
        ] {
            let response = ticket(app_state(&[], CODE_HASH), body, Some(user_agent)).await;
            assert_eq!(response.status(), StatusCode::OK, "{user_agent}");
        }
    }
    for &user_agent in NON_LEGACY_USER_AGENTS {
        let response = ticket(
            app_state(&[], CODE_HASH),
            json!({"code_hash": CODE_HASH}),
            user_agent,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{user_agent:?}");
        assert_eq!(
            response_json::<Value>(response).await["error"],
            "compiler and compiler_version are required"
        );
    }
}

#[tokio::test]
async fn duplicate_or_non_text_user_agents_do_not_grant_legacy_exceptions() {
    for values in [
        vec![b"acton/1.1.0".as_slice(), b"acton/1.2.0"],
        vec![b"\xff".as_slice()],
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/take_ticket")
            .header(header::CONTENT_TYPE, "application/json");
        for value in values {
            request = request.header(header::USER_AGENT, value);
        }
        let response = app::router_with_state(app_state(&[], CODE_HASH))
            .oneshot(
                request
                    .body(Body::from(json!({"code_hash": CODE_HASH}).to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn non_legacy_tickets_check_only_the_deny_list_before_payment_quotes() {
    for &user_agent in NON_LEGACY_USER_AGENTS {
        for (compiler, version) in [
            ("tolk", "1.4.2"),
            (" FUNc ", "0.4.6-wasmfix.0"),
            ("tact", "1.6.13"),
            ("", "1.4.2"),
            ("unknown", "1.0.0"),
            ("tolk", ""),
            ("tolk", " "),
            ("tolk", "99.0.0"),
            ("func", "1.4.2"),
            ("tact", "^1.6.13"),
            ("tolk", "v1.4.2"),
            ("tolk", "1.4.2 "),
            ("func", "0.4.6-wasmfix.1"),
        ] {
            let response = ticket(
                app_state(&[], CODE_HASH),
                json!({
                    "code_hash": CODE_HASH, "compiler": compiler, "compiler_version": version,
                }),
                user_agent,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{user_agent:?}: {compiler}@{version}"
            );
        }
        for body in [
            json!({"code_hash": CODE_HASH, "compiler": "tolk"}),
            json!({"code_hash": CODE_HASH, "compiler_version": "1.4.2"}),
        ] {
            let response = ticket(app_state(&[], CODE_HASH), body, user_agent).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{user_agent:?}");
        }
        for (compiler, version) in [
            ("TACT", "1.6.13"),
            ("func", "0.4.4"),
            (" tolk ", "1.4.1"),
            ("unknown", "nightly"),
        ] {
            let state = recovering_payment_app_state(CODE_HASH).with_compiler_policy(policy(&[
                "tact",
                "func@0.4.4",
                "tolk@1.4.1",
                "unknown@nightly",
            ]));
            let response = ticket(
                state,
                json!({
                    "code_hash": CODE_HASH, "compiler": compiler, "compiler_version": version,
                }),
                user_agent,
            )
            .await;
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{user_agent:?}: {compiler}@{version}"
            );
            assert!(
                response_json::<Value>(response).await["error"]
                    .as_str()
                    .expect("error")
                    .starts_with("compiler_disabled:")
            );
        }
    }
}

#[tokio::test]
async fn verification_rejects_disabled_compilers_without_running_them() {
    for (language, version, rule) in [
        ("tolk", "1.4.1", "tolk"),
        (" TOLK ", "1.4.1", "tolk@1.4.1"),
        ("func", "0.4.6", "func@0.4.6"),
        ("tact", "1.6.13", "tact"),
        ("tolk", "nightly", "tolk@nightly"),
    ] {
        let (state, requests) = recording_app_state(&[], CODE_HASH);
        let response = post_verify(
            state.with_compiler_policy(policy(&[rule])),
            parts(language, version),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{language}@{version}"
        );
        assert!(requests.lock().expect("compiler requests").is_empty());
    }
}

#[tokio::test]
async fn verification_passes_unlisted_versions_to_the_compiler_without_format_validation() {
    for (language, version) in [
        ("tolk", "99.0.0"),
        ("func", "1.4.2"),
        ("tact", "99.0.0"),
        ("tolk", "nightly"),
        ("func", "v0.4.6"),
        ("tact", "^1.6.13"),
    ] {
        let (state, requests) = recording_app_state(&[], CODE_HASH);
        let state =
            state.with_compiler_policy(policy(&["tolk@1.4.1", "func@0.4.4", "tact@1.6.13"]));
        let response = post_verify(state, parts(language, version)).await;
        assert_eq!(response.status(), StatusCode::OK, "{language}@{version}");
        let requests = requests.lock().expect("compiler requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].language, language);
        assert_eq!(requests[0].compiler_version, version);
        drop(requests);
    }
}

#[tokio::test]
async fn rejected_compilers_do_not_claim_or_consume_payment() {
    for (version, rule) in [("1.4.1", "tolk@1.4.1"), ("nightly", "tolk@nightly")] {
        let state = payment_error_app_state(CODE_HASH, PaymentError::AlreadyUsed)
            .with_compiler_policy(policy(&[rule]));
        let response = post_verify(state.clone(), parts("tolk", version)).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        // The one-shot claim error must still be present after the rejected request.
        let response = post_verify(
            state.with_compiler_policy(CompilerPolicy::default()),
            parts("tolk", "1.4.1"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let (state, outcomes) = recording_payment_app_state(CODE_HASH);
        let response = post_verify(
            state.with_compiler_policy(policy(&[rule])),
            parts("tolk", version),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(outcomes.lock().expect("payment outcomes").is_empty());
    }
}

#[tokio::test]
async fn legacy_clients_can_get_tickets_and_verify_despite_compiler_restrictions() {
    for &user_agent in LEGACY_USER_AGENTS {
        for (compiler, version, rule) in [
            ("tolk", "1.4.1", "tolk@1.4.1"),
            ("func", "0.4.6", "func@0.4.6"),
            ("tact", "1.6.13", "tact"),
        ] {
            let (state, requests) = recording_app_state(&[], CODE_HASH);
            let state = state.with_compiler_policy(policy(&[rule]));
            for body in [
                json!({"code_hash": CODE_HASH}),
                json!({"code_hash": CODE_HASH, "compiler": compiler, "compiler_version": version}),
            ] {
                let response = ticket(state.clone(), body, Some(user_agent)).await;
                assert_eq!(response.status(), StatusCode::OK, "{user_agent}: {rule}");
            }
            let mut request_parts = parts(compiler, version);
            if compiler == "tact" {
                request_parts.remove(2); // Legacy Tact requests get their version from .pkg.
            }
            let response = post_verify_with_user_agent(state, request_parts, user_agent).await;
            assert_eq!(response.status(), StatusCode::OK, "{user_agent}: {rule}");
            let requests = requests.lock().expect("compiler requests");
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].compiler_version, version);
            drop(requests);
        }
    }
}

#[tokio::test]
async fn non_legacy_verification_rejects_disabled_compilers_before_payment() {
    for &user_agent in NON_LEGACY_USER_AGENTS {
        let state = payment_error_app_state(CODE_HASH, PaymentError::AlreadyUsed)
            .with_compiler_policy(policy(&["tolk"]));
        let response = match user_agent {
            Some(user_agent) => {
                post_verify_with_user_agent(state, parts("tolk", "1.4.1"), user_agent).await
            }
            None => post_verify(state, parts("tolk", "1.4.1")).await,
        };
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{user_agent:?}");
    }
}

#[tokio::test]
async fn api_keys_do_not_bypass_compiler_restrictions() {
    let state = app_state(&[], CODE_HASH).with_compiler_policy(policy(&["tolk"]));
    let response = post_verify_with_api_key(
        state.with_api_key(Some("key")),
        parts("tolk", "1.4.1"),
        "key",
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn tact_version_from_package_is_checked_before_payment() {
    let (state, outcomes) = recording_payment_app_state(CODE_HASH);
    let state = state.with_compiler_policy(policy(&["tact@1.6.13"]));
    let mut request_parts = parts("tact", "1.6.13");
    request_parts.remove(2); // No explicit compile_params; the version comes from .pkg.
    let response = post_verify(state, request_parts).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(outcomes.lock().expect("payment outcomes").is_empty());
}

#[tokio::test]
async fn exact_rules_allow_other_versions_and_cached_bundles_remain_accessible() {
    let state = app_state(&[], CODE_HASH).with_compiler_policy(policy(&["tolk@1.4.1"]));
    let response = post_verify(state.clone(), parts("tolk", "1.4.2")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let state = state.with_compiler_policy(policy(&["tolk"]));
    let response = ticket(
        state.clone(),
        json!({"code_hash": CODE_HASH}),
        Some("acton/1.2.0"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<Value>(response).await["status"],
        "already_verified"
    );
    let response = post_verify(state, parts("tolk", "1.4.2")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json::<Value>(response).await["verification_result"],
        "already_verified"
    );
}
