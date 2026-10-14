use std::{path::Path, process::Output, time::Duration};

use tokio::process::Command;

use super::SourceStorageError;

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_mins(1);

/// Git commands shared by source storage and maintenance tools, without startup recovery.
pub struct Git<'a> {
    path: &'a Path,
}

impl<'a> Git<'a> {
    #[must_use]
    pub const fn new(path: &'a Path) -> Self {
        Self { path }
    }

    /// Runs Git and returns its exit status and raw output, including on failure.
    ///
    /// # Errors
    /// Returns an error if the process cannot start or exceeds its deadline.
    pub async fn command(&self, args: &[&str]) -> Result<Output, SourceStorageError> {
        let command = self.prepare_command(args);
        run_git_command(command, args, GIT_COMMAND_TIMEOUT).await
    }

    fn prepare_command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.path);
        command
    }

    /// Runs Git and requires a successful exit status.
    ///
    /// # Errors
    /// Returns an error if Git fails, cannot start or exceeds its deadline.
    pub async fn run(&self, args: &[&str]) -> Result<(), SourceStorageError> {
        let output = self.command(args).await?;
        ensure_success(args, &output)
    }

    /// Runs Git with an explicit author, committer and Unix timestamp in UTC.
    ///
    /// # Errors
    /// Returns an error if Git fails, cannot start or exceeds its deadline.
    pub async fn run_with_author(
        &self,
        args: &[&str],
        author_name: &str,
        author_email: &str,
        timestamp: u64,
    ) -> Result<(), SourceStorageError> {
        let git_date = format!("{timestamp} +0000");
        let mut command = self.prepare_command(args);
        command
            .env("GIT_AUTHOR_NAME", author_name)
            .env("GIT_AUTHOR_EMAIL", author_email)
            .env("GIT_AUTHOR_DATE", &git_date)
            .env("GIT_COMMITTER_NAME", author_name)
            .env("GIT_COMMITTER_EMAIL", author_email)
            .env("GIT_COMMITTER_DATE", &git_date);
        let output = run_git_command(command, args, GIT_COMMAND_TIMEOUT).await?;
        ensure_success(args, &output)
    }

    /// Returns successful Git output with surrounding whitespace removed.
    ///
    /// # Errors
    /// Returns an error if Git fails or its output is not UTF-8.
    pub async fn output(&self, args: &[&str]) -> Result<String, SourceStorageError> {
        self.output_untrimmed(args)
            .await
            .map(|output| output.trim().to_owned())
    }

    /// Returns successful Git output without changing whitespace or path names.
    ///
    /// # Errors
    /// Returns an error if Git fails or its output is not UTF-8.
    pub async fn output_untrimmed(&self, args: &[&str]) -> Result<String, SourceStorageError> {
        let output = self.command(args).await?;
        decode_output(args, output)
    }

    /// Returns the current named branch without consulting the remote.
    ///
    /// # Errors
    /// Returns an error for detached HEAD or a failed Git command.
    pub async fn current_branch(&self) -> Result<String, SourceStorageError> {
        let branch = self.output(&["branch", "--show-current"]).await?;
        if branch.is_empty() {
            return Err(SourceStorageError::DetachedHead);
        }
        Ok(branch)
    }

    /// Checks staged, unstaged and untracked changes without performing recovery.
    ///
    /// # Errors
    /// Returns an error if the checkout is dirty or Git fails.
    pub async fn ensure_clean(&self) -> Result<(), SourceStorageError> {
        let changes = self
            .output_untrimmed(&["status", "--porcelain=v1", "--untracked-files=all"])
            .await?
            .trim_end_matches(['\r', '\n'])
            .to_owned();
        if changes.is_empty() {
            return Ok(());
        }
        Err(SourceStorageError::DirtySourceRepository {
            path: self.path.to_path_buf(),
            changes,
        })
    }
}

fn ensure_success(args: &[&str], output: &Output) -> Result<(), SourceStorageError> {
    if !output.status.success() {
        return Err(SourceStorageError::Git {
            command: git_command_string(args),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

fn decode_output(args: &[&str], output: Output) -> Result<String, SourceStorageError> {
    ensure_success(args, &output)?;
    String::from_utf8(output.stdout).map_err(|source| SourceStorageError::GitOutputUtf8 {
        command: git_command_string(args),
        source,
    })
}

// Storage holds a mutex across Git operations. A stalled remote or credential
// prompt must not block all subsequent writes and registry reads indefinitely.
async fn run_git_command(
    mut command: Command,
    args: &[&str],
    timeout: Duration,
) -> Result<Output, SourceStorageError> {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let command_name = git_command_string(args);
    tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| SourceStorageError::Git {
            command: command_name.clone(),
            status: "timed out".to_owned(),
            stderr: format!("Git exceeded its {} ms deadline", timeout.as_millis()),
        })?
        .map_err(|source| SourceStorageError::GitSpawn {
            command: command_name,
            source,
        })
}

fn git_command_string(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn git_process_has_closed_stdin_and_disables_credential_prompts() {
        let mut command = Command::new("node");
        command.args([
            "--eval",
            "process.stdin.on('data', () => { throw new Error('unexpected input') }); \
             process.stdin.on('end', () => process.stdout.write(process.env.GIT_TERMINAL_PROMPT))",
        ]);
        let output = run_git_command(command, &["probe"], Duration::from_secs(3))
            .await
            .expect("process completes");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"0");
    }

    #[tokio::test]
    async fn stalled_git_process_releases_storage_with_retryable_push_context() {
        let mut command = Command::new("node");
        command.args(["--eval", "setInterval(() => {}, 1000)"]);
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            run_git_command(
                command,
                &["push", "origin", "HEAD:main"],
                Duration::from_millis(100),
            ),
        )
        .await
        .expect("deadline must cover a stalled process");
        let error = result.expect_err("process must time out");
        assert!(
            matches!(&error, SourceStorageError::Git { command, status, .. } if command == "git push origin HEAD:main" && status == "timed out")
        );
        assert!(crate::error::ApiError::from(error).is_payment_retryable());
    }
}
