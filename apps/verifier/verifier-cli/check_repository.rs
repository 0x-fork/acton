use std::path::Path;

use verifier::source_storage::{SourceStorageError, git::Git, repository};

use crate::error::CliError;

pub struct CheckedRepository {
    pub branch: String,
    pub local_head: String,
    pub remote_head: Option<String>,
}

pub async fn check(
    path: &Path,
    storage_root: &str,
    remote: &str,
    expected_branch: Option<&str>,
    check_remote: bool,
) -> Result<CheckedRepository, CliError> {
    eprintln!("Checking repository: {}", path.display());
    let git = Git::new(path);
    git.ensure_work_tree().await?;
    if !git
        .output(&["rev-parse", "--show-prefix"])
        .await?
        .is_empty()
    {
        return Err(CliError::NotCheckoutRoot {
            path: path.to_path_buf(),
        });
    }
    let local_head = git.head().await?;
    let branch = git.current_branch().await.map_err(|error| match error {
        SourceStorageError::DetachedHead => CliError::DetachedHead,
        error => error.into(),
    })?;
    if let Some(expected) = expected_branch
        && expected != branch
    {
        return Err(CliError::BranchMismatch {
            expected: expected.to_owned(),
            current: branch,
        });
    }

    repository::check(path, storage_root).await?;

    let remote_head = if check_remote {
        eprintln!("Checking remote: {remote}/{branch}");
        let remote_head = remote_head(&git, remote, &branch).await?;
        if local_head != remote_head {
            return Err(CliError::HeadMismatch {
                remote: remote.to_owned(),
                branch,
                local_head,
                remote_head,
            });
        }
        Some(remote_head)
    } else {
        None
    };
    Ok(CheckedRepository {
        branch,
        local_head,
        remote_head,
    })
}

async fn remote_head(git: &Git<'_>, remote: &str, branch: &str) -> Result<String, CliError> {
    let reference = format!("refs/heads/{branch}");
    let output = git
        .output(&[
            "ls-remote",
            "--exit-code",
            "--refs",
            "--",
            remote,
            &reference,
        ])
        .await?;
    // Match the full ref, not remote HEAD or a cached remote-tracking reference.
    if let Some((sha, returned_ref)) = output.split_once('\t')
        && returned_ref == reference
        && matches!(sha.len(), 40 | 64)
        && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Ok(sha.to_owned());
    }
    Err(CliError::InvalidRemoteHead {
        remote: remote.to_owned(),
        branch: branch.to_owned(),
    })
}
