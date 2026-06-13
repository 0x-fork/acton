#![allow(dead_code)]

use std::sync::Mutex;
use std::{borrow::Cow, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, header::CONTENT_TYPE},
    response::Response,
};
use serde::de::DeserializeOwned;
use tower::ServiceExt;
use verifier::{
    app,
    blockchain::ToncenterClient,
    compilers::CompileRequest,
    registry::{RegisterBundleRequest, SharedRegistryClient},
    source_storage::SharedSourceStorage,
    state::AppState,
};

mod mock_blockchain;
mod mock_compiler;
mod mock_registry;
mod mock_source_storage;

const MULTIPART_BOUNDARY: &str = "verifier-test-boundary";

pub fn app_state(code_hashes: &[(&str, &str)], compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    AppState::new(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_registry::MockRegistryClient::confirmed()),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn toncenter_app_state(base_url: &str, compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    AppState::new(
        Arc::new(ToncenterClient::new(base_url.to_owned(), None)),
        Arc::new(compiler_service),
        Arc::new(mock_registry::MockRegistryClient::confirmed()),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn toncenter_app_state_with_registry(
    base_url: &str,
    compiled_code_hash: &str,
    registry_client: SharedRegistryClient,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    AppState::new(
        Arc::new(ToncenterClient::new(base_url.to_owned(), None)),
        Arc::new(compiler_service),
        registry_client,
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn toncenter_app_state_with_registry_and_source_storage(
    base_url: &str,
    compiled_code_hash: &str,
    registry_client: SharedRegistryClient,
    source_storage: SharedSourceStorage,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    AppState::new(
        Arc::new(ToncenterClient::new(base_url.to_owned(), None)),
        Arc::new(compiler_service),
        registry_client,
        source_storage,
    )
}

pub fn recording_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<CompileRequest>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let recorded_requests = compiler_service.recorded_requests();

    (
        AppState::new(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(mock_registry::MockRegistryClient::confirmed()),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
        ),
        recorded_requests,
    )
}

pub fn recording_registry_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<RegisterBundleRequest>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let registry_client = mock_registry::MockRegistryClient::confirmed();
    let recorded_requests = registry_client.recorded_requests();

    (
        AppState::new(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(registry_client),
            Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
        ),
        recorded_requests,
    )
}

pub fn recording_source_storage_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (
    AppState,
    Arc<Mutex<Vec<mock_source_storage::RecordedSourceStorageRequest>>>,
) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let source_storage = mock_source_storage::MockSourceStorage::confirmed();
    let recorded_requests = source_storage.recorded_requests();

    (
        AppState::new(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(mock_registry::MockRegistryClient::confirmed()),
            Arc::new(source_storage),
        ),
        recorded_requests,
    )
}

pub fn failing_registry_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);

    AppState::new(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_registry::MockRegistryClient::failing(
            "registry sender failed",
        )),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn unverified_registry_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);

    AppState::new(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_registry::MockRegistryClient::unverified()),
        Arc::new(mock_source_storage::MockSourceStorage::confirmed()),
    )
}

pub fn failing_source_storage_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);

    AppState::new(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
        Arc::new(mock_registry::MockRegistryClient::confirmed()),
        Arc::new(mock_source_storage::MockSourceStorage::failing(
            "source storage failed",
        )),
    )
}

pub fn failing_source_storage_recording_registry_app_state(
    code_hashes: &[(&str, &str)],
    compiled_code_hash: &str,
) -> (AppState, Arc<Mutex<Vec<RegisterBundleRequest>>>) {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    let registry_client = mock_registry::MockRegistryClient::confirmed();
    let recorded_requests = registry_client.recorded_requests();

    (
        AppState::new(
            Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
            Arc::new(compiler_service),
            Arc::new(registry_client),
            Arc::new(mock_source_storage::MockSourceStorage::failing(
                "source storage failed",
            )),
        ),
        recorded_requests,
    )
}

pub async fn get(state: AppState, path: &str) -> Response {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .expect("GET request should be valid");

    app::router_with_state(state)
        .oneshot(request)
        .await
        .expect("router should handle GET request")
}

pub async fn post_verify(state: AppState, parts: Vec<MultipartPart>) -> Response {
    let body = multipart_body(parts);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/verify")
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}"),
        )
        .body(Body::from(body))
        .expect("POST /api/v1/verify request should be valid");

    app::router_with_state(state)
        .oneshot(request)
        .await
        .expect("router should handle POST /api/v1/verify request")
}

pub async fn response_json<T>(response: Response) -> T
where
    T: DeserializeOwned,
{
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&bytes).expect("response body should be valid JSON")
}

pub const fn text_part(name: &'static str, value: &'static str) -> MultipartPart {
    MultipartPart::Text {
        name,
        value: Cow::Borrowed(value),
    }
}

pub fn owned_text_part(name: &'static str, value: impl Into<String>) -> MultipartPart {
    MultipartPart::Text {
        name,
        value: Cow::Owned(value.into()),
    }
}

pub const fn file_part(
    name: &'static str,
    file_name: &'static str,
    content_type: &'static str,
    content: &'static str,
) -> MultipartPart {
    MultipartPart::File {
        name,
        file_name: Cow::Borrowed(file_name),
        content_type,
        content: Cow::Borrowed(content),
    }
}

fn multipart_body(parts: Vec<MultipartPart>) -> Vec<u8> {
    let mut body = Vec::new();

    for part in parts {
        body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}\r\n").as_bytes());

        match part {
            MultipartPart::Text { name, value } => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
                );
                body.extend_from_slice(value.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
            MultipartPart::File {
                name,
                file_name,
                content_type,
                content,
            } => {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n"
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
                body.extend_from_slice(content.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
        }
    }

    body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}--\r\n").as_bytes());
    body
}

pub enum MultipartPart {
    Text {
        name: &'static str,
        value: Cow<'static, str>,
    },
    File {
        name: &'static str,
        file_name: Cow<'static, str>,
        content_type: &'static str,
        content: Cow<'static, str>,
    },
}
