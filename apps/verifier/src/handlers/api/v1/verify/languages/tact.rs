use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use super::{ReceivedFile, SourceMetadata, string_param};
use crate::error::ApiError;

pub(super) const LANGUAGE: &str = "tact";

pub(super) fn compiler_version(
    compile_params: &Value,
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<String, ApiError> {
    string_param(compile_params, &["compiler_version"])
        .map_or_else(|| compiler_version_from_pkg(sources, files), Ok)
}

pub(super) fn entrypoint(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    pkg_entrypoint(sources)
}

fn compiler_version_from_pkg(
    sources: &[SourceMetadata],
    files: &BTreeMap<String, ReceivedFile>,
) -> Result<String, ApiError> {
    let pkg_path = pkg_entrypoint(sources)?;
    let pkg = files.get(&pkg_path).ok_or_else(|| {
        ApiError::bad_request(format!("source metadata has no uploaded file: {pkg_path}"))
    })?;
    let pkg = serde_json::from_slice::<Value>(&pkg.content)
        .map_err(|err| ApiError::bad_request(format!("invalid Tact pkg JSON: {err}")))?;

    pkg.pointer("/compiler/version")
        .and_then(Value::as_str)
        .filter(|version| !version.trim().is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            ApiError::bad_request(
                "missing Tact compiler version: provide compile_params.compiler_version or pkg.compiler.version"
                    .to_owned(),
            )
        })
}

fn pkg_entrypoint(sources: &[SourceMetadata]) -> Result<String, ApiError> {
    sources
        .iter()
        .filter(|source| {
            Path::new(&source.path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pkg"))
        })
        .min_by_key(|source| source.path.split('/').count())
        .map(|source| source.path.clone())
        .ok_or_else(|| ApiError::bad_request("missing Tact .pkg source".to_owned()))
}
