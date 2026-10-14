//! Source repository checks and paths shared by backend and maintenance commands.
//! Recovery remains in `GitSourceStorage`; validation never clones or synchronizes.

use std::path::{Component, Path, PathBuf};

use crate::blockchain::is_valid_code_hash;

use super::{SourceStorageError, git::Git};

/// Validates an existing source checkout without recovery or synchronization.
///
/// Backend uses the same checks around its explicit startup recovery. Maintenance
/// commands use this entry point to reject dirty storage without repairing it.
///
/// # Errors
/// Returns an error if the checkout is missing, unprepared, dirty or Git fails.
pub async fn check(repo_path: &Path, storage_root: &str) -> Result<(), SourceStorageError> {
    let git = Git::new(repo_path);
    ensure_initialized(&git, storage_root).await?;
    ensure_checkout(&git, storage_root).await
}

pub(super) async fn ensure_initialized(
    git: &Git<'_>,
    storage_root: &str,
) -> Result<(), SourceStorageError> {
    git.ensure_work_tree().await?;
    let repo_path = git.path();
    let source_attributes_rule = source_attributes_rule(storage_root);
    let roots = git
        .output(&["rev-list", "--max-parents=0", "HEAD"])
        .await
        .map_err(|source| unprepared_source_repository(repo_path, source.to_string()))?;
    let roots = roots
        .lines()
        .filter(|revision| !revision.is_empty())
        .collect::<Vec<_>>();
    let [root_revision] = roots.as_slice() else {
        return Err(unprepared_source_repository(
            repo_path,
            format!("expected exactly one root commit, found {}", roots.len()),
        ));
    };

    let root_files = git
        .output(&["ls-tree", "-r", "--name-only", root_revision])
        .await
        .map_err(|source| unprepared_source_repository(repo_path, source.to_string()))?;
    if root_files != ".gitattributes" {
        return Err(unprepared_source_repository(
            repo_path,
            "root commit must contain only `.gitattributes`",
        ));
    }

    let root_attributes_object = format!("{root_revision}:.gitattributes");
    let root_attributes = git
        .output(&["show", &root_attributes_object])
        .await
        .map_err(|source| unprepared_source_repository(repo_path, source.to_string()))?;
    if root_attributes != source_attributes_rule {
        return Err(unprepared_source_repository(
            repo_path,
            format!("root `.gitattributes` must contain exactly `{source_attributes_rule}`"),
        ));
    }

    Ok(())
}

pub(super) async fn ensure_checkout(
    git: &Git<'_>,
    storage_root: &str,
) -> Result<(), SourceStorageError> {
    git.ensure_clean().await?;
    let repo_path = git.path();
    let source_attributes_rule = source_attributes_rule(storage_root);
    let source_attributes_check_path = source_attributes_check_path(storage_root);
    let current_attributes = git
        .output(&["show", "HEAD:.gitattributes"])
        .await
        .map_err(|source| unprepared_source_repository(repo_path, source.to_string()))?;
    if !has_source_attributes_rule(&current_attributes, &source_attributes_rule) {
        return Err(unprepared_source_repository(
            repo_path,
            format!("current `.gitattributes` must contain `{source_attributes_rule}`"),
        ));
    }

    let effective_text_attribute = git
        .output(&["check-attr", "text", "--", &source_attributes_check_path])
        .await
        .map_err(|source| unprepared_source_repository(repo_path, source.to_string()))?;
    let expected_text_attribute = format!("{source_attributes_check_path}: text: unset");
    if effective_text_attribute != expected_text_attribute {
        return Err(unprepared_source_repository(
            repo_path,
            format!(
                "`{source_attributes_rule}` is not effective for `{source_attributes_check_path}`"
            ),
        ));
    }

    Ok(())
}

fn has_source_attributes_rule(attributes: &str, source_attributes_rule: &str) -> bool {
    attributes
        .lines()
        .any(|line| line.trim() == source_attributes_rule)
}

fn unprepared_source_repository(
    repo_path: &Path,
    message: impl Into<String>,
) -> SourceStorageError {
    SourceStorageError::UnpreparedSourceRepository {
        path: repo_path.to_path_buf(),
        message: message.into(),
    }
}

fn source_attributes_rule(storage_root: &str) -> String {
    format!("{storage_root}/** -text")
}

fn source_attributes_check_path(storage_root: &str) -> String {
    format!("{storage_root}/.verifier-attributes-check")
}

/// Returns the bundle path for a canonical hexadecimal code hash.
///
/// # Errors
/// Returns an error if the hash is not 64 hexadecimal characters.
pub fn bundle_relative_path(
    storage_root: &str,
    code_hash: &str,
) -> Result<String, SourceStorageError> {
    if !is_valid_code_hash(code_hash) {
        return Err(SourceStorageError::InvalidCodeHash);
    }

    let (prefix, suffix) = code_hash.split_at(2);
    Ok(format!("{storage_root}/{prefix}/{suffix}"))
}

/// Joins a relative path without accepting absolute paths or parent traversal.
/// This is a lexical check; callers must handle symbolic links where needed.
///
/// # Errors
/// Returns an error for an absolute path or an invalid path component.
pub fn checked_join(root: &Path, relative: &str) -> Result<PathBuf, SourceStorageError> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(SourceStorageError::InvalidPath {
            path: relative_path.to_path_buf(),
            message: "path must be relative".to_owned(),
        });
    }

    let mut path = root.to_path_buf();
    for component in relative_path.components() {
        match component {
            Component::Normal(component) => path.push(component),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(SourceStorageError::InvalidPath {
                    path: relative_path.to_path_buf(),
                    message: "path contains an invalid component".to_owned(),
                });
            }
        }
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_path_splits_code_hash_after_first_two_characters() {
        let code_hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let path = bundle_relative_path("sources", code_hash)
            .expect("valid code hash should produce a bundle path");

        assert_eq!(
            path,
            "sources/ab/cdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
    }

    #[test]
    fn bundle_path_rejects_invalid_code_hash() {
        assert!(matches!(
            bundle_relative_path("sources", "not-a-code-hash"),
            Err(SourceStorageError::InvalidCodeHash)
        ));
    }

    #[test]
    fn checked_join_rejects_absolute_and_parent_paths_but_preserves_valid_names() {
        let root = Path::new("checkout");
        for relative in [
            "/outside",
            "../outside",
            "sources/../../outside",
            "./sources",
        ] {
            assert!(
                matches!(
                    checked_join(root, relative),
                    Err(SourceStorageError::InvalidPath { .. })
                ),
                "{relative}"
            );
        }
        for relative in [
            "",
            "verified/contracts",
            "sources/file name.tolk",
            "sources/.metadata",
        ] {
            assert_eq!(
                checked_join(root, relative).expect("relative path"),
                root.join(relative)
            );
        }
    }
}
