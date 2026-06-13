use std::sync::Arc;
use std::sync::Mutex;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, header::CONTENT_TYPE},
    response::Response,
};
use serde::de::DeserializeOwned;
use tower::ServiceExt;
use verifier::{app, compilers::CompileRequest, state::AppState};

mod mock_blockchain;
mod mock_compiler;

const MULTIPART_BOUNDARY: &str = "verifier-test-boundary";

pub fn app_state(code_hashes: &[(&str, &str)], compiled_code_hash: &str) -> AppState {
    let compiler_service = mock_compiler::MockCompilerService::new(compiled_code_hash);
    AppState::new(
        Arc::new(mock_blockchain::MockBlockchainClient::new(code_hashes)),
        Arc::new(compiler_service),
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
    MultipartPart::Text { name, value }
}

pub const fn file_part(
    name: &'static str,
    file_name: &'static str,
    content_type: &'static str,
    content: &'static str,
) -> MultipartPart {
    MultipartPart::File {
        name,
        file_name,
        content_type,
        content,
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
        value: &'static str,
    },
    File {
        name: &'static str,
        file_name: &'static str,
        content_type: &'static str,
        content: &'static str,
    },
}
