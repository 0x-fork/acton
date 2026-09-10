use std::path::Path;

use axum::{body::Bytes, extract::multipart::Field};

use super::ReceivedFile;
use crate::{config::UploadLimits, error::ApiError};

const MAX_UPLOADED_FILES: usize = 256;

pub(super) fn ensure_file_slot(uploaded_file_count: usize) -> Result<(), ApiError> {
    if uploaded_file_count >= MAX_UPLOADED_FILES {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_UPLOADED_FILES} source files may be uploaded"
        )));
    }
    Ok(())
}

pub(super) async fn read_json_part(
    field: Field<'_>,
    max_bytes: Option<usize>,
    description: &str,
) -> Result<String, ApiError> {
    let content = read_limited_part(field, max_bytes, description).await?;
    Ok(String::from_utf8_lossy(&content).into_owned())
}

pub(super) async fn read_file_part(
    field: Field<'_>,
    upload_limits: UploadLimits,
) -> Result<ReceivedFile, ApiError> {
    let file_name = field.file_name().map(ToOwned::to_owned);
    let max_bytes = max_file_bytes(upload_limits, file_name.as_deref());
    let description = file_name.as_deref().map_or_else(
        || "uploaded file".to_owned(),
        |file_name| format!("uploaded file {file_name}"),
    );
    let content = read_limited_part(field, max_bytes, &description).await?;

    Ok(ReceivedFile { file_name, content })
}

async fn read_limited_part(
    mut field: Field<'_>,
    max_bytes: Option<usize>,
    description: &str,
) -> Result<Bytes, ApiError> {
    let mut content = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(ApiError::from)? {
        let next_len = content.len().saturating_add(chunk.len());
        if let Some(max_bytes) = max_bytes
            && next_len > max_bytes
        {
            return Err(ApiError::payload_too_large(format!(
                "{description} exceeds the configured limit of {max_bytes} bytes"
            )));
        }
        content.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(content))
}

fn max_file_bytes(upload_limits: UploadLimits, file_name: Option<&str>) -> Option<usize> {
    let extension = Path::new(file_name?).extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("json") || extension.eq_ignore_ascii_case("pkg") {
        upload_limits.max_json_file_bytes()
    } else if extension.eq_ignore_ascii_case("tolk") {
        upload_limits.max_tolk_file_bytes()
    } else if extension.eq_ignore_ascii_case("fc") || extension.eq_ignore_ascii_case("func") {
        upload_limits.max_func_file_bytes()
    } else if extension.eq_ignore_ascii_case("tact") {
        upload_limits.max_tact_file_bytes()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_file_limit_by_extension_case_insensitively() {
        let limits = UploadLimits::new(100, Some(10), Some(20), Some(30), Some(40));

        for path in ["file.json", "file.pkg", "FILE.JSON"] {
            assert_eq!(max_file_bytes(limits, Some(path)), Some(10));
        }
        for path in ["file.tolk", "FILE.TOLK"] {
            assert_eq!(max_file_bytes(limits, Some(path)), Some(20));
        }
        for path in ["file.fc", "file.func", "FILE.FC"] {
            assert_eq!(max_file_bytes(limits, Some(path)), Some(30));
        }
        for path in ["file.tact", "FILE.TACT"] {
            assert_eq!(max_file_bytes(limits, Some(path)), Some(40));
        }
        assert_eq!(max_file_bytes(limits, Some("file.txt")), None);
        assert_eq!(max_file_bytes(limits, None), None);
    }
}
