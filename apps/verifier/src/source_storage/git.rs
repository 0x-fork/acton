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
    use std::{fs, io::ErrorKind};

    use tempfile::TempDir;

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

    async fn repository() -> TempDir {
        let dir = tempfile::Builder::new()
            .prefix("verifier git tests ")
            .tempdir()
            .expect("temporary repository");
        let git = Git::new(dir.path());
        git.run(&["init", "--initial-branch=main"])
            .await
            .expect("initialize repository");
        for (key, value) in [
            ("user.name", "Repository Author"),
            ("user.email", "repository@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", "disabled-hooks"),
        ] {
            git.run(&["config", key, value])
                .await
                .expect("configure repository");
        }
        dir
    }

    async fn commit(git: &Git<'_>) {
        git.run(&[
            "commit",
            "--allow-empty",
            "--no-gpg-sign",
            "-m",
            "Test commit",
        ])
        .await
        .expect("commit fixture");
    }

    async fn blob(git: &Git<'_>, path: &Path, bytes: &[u8]) -> String {
        fs::write(path.join("blob.bin"), bytes).expect("blob contents");
        git.output(&["hash-object", "-w", "--no-filters", "--", "blob.bin"])
            .await
            .expect("write Git blob")
    }

    fn assert_git_failure(error: SourceStorageError, command: &str, output: &Output) {
        let SourceStorageError::Git {
            command: actual_command,
            status,
            stderr,
        } = error
        else {
            panic!("expected a Git failure, got {error:?}");
        };
        assert_eq!(actual_command, command);
        assert_eq!(status, output.status.to_string());
        assert_eq!(stderr, String::from_utf8_lossy(&output.stderr).trim());
        assert!(!stderr.is_empty(), "failure must retain Git diagnostics");
    }

    #[tokio::test]
    async fn current_branch_reads_unborn_and_switched_branches() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        assert_eq!(git.current_branch().await.expect("unborn branch"), "main");
        commit(&git).await;
        git.run(&["checkout", "-b", "feature/registry"])
            .await
            .expect("switch branch");
        assert_eq!(
            git.current_branch().await.expect("current branch"),
            "feature/registry"
        );
    }

    #[tokio::test]
    async fn current_branch_rejects_detached_head() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        commit(&git).await;
        git.run(&["checkout", "--detach"])
            .await
            .expect("detach HEAD");
        assert!(matches!(
            git.current_branch().await,
            Err(SourceStorageError::DetachedHead)
        ));
    }

    #[tokio::test]
    async fn failed_commands_preserve_status_stderr_and_command_context() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        let args = ["rev-parse", "--verify", "refs/heads/missing"];
        let command = "git rev-parse --verify refs/heads/missing";
        let output = git.command(&args).await.expect("Git process starts");
        assert!(!output.status.success());

        assert_git_failure(
            git.run(&args).await.expect_err("run fails"),
            command,
            &output,
        );
        assert_git_failure(
            git.output(&args).await.expect_err("output fails"),
            command,
            &output,
        );
        assert_git_failure(
            git.output_untrimmed(&args)
                .await
                .expect_err("untrimmed output fails"),
            command,
            &output,
        );
    }

    #[tokio::test]
    async fn branch_and_clean_checks_propagate_git_errors() {
        let dir = tempfile::tempdir().expect("directory without Git metadata");
        let git = Git::new(dir.path());
        let branch_args = ["branch", "--show-current"];
        let output = git.command(&branch_args).await.expect("Git process starts");
        assert!(!output.status.success());
        assert_git_failure(
            git.current_branch().await.expect_err("not a repository"),
            "git branch --show-current",
            &output,
        );

        let status_args = ["status", "--porcelain=v1", "--untracked-files=all"];
        let output = git.command(&status_args).await.expect("Git process starts");
        assert!(!output.status.success());
        assert_git_failure(
            git.ensure_clean().await.expect_err("not a repository"),
            "git status --porcelain=v1 --untracked-files=all",
            &output,
        );
    }

    #[tokio::test]
    async fn output_methods_preserve_bytes_or_trim_only_surrounding_whitespace() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        let contents = " \t\r\n  исходный текст\n  second line  \t\r\n";
        let hash = blob(&git, dir.path(), contents.as_bytes()).await;
        let args = ["cat-file", "blob", &hash];
        let output = git.command(&args).await.expect("raw output");
        assert!(output.status.success());
        assert_eq!(output.stdout, contents.as_bytes());
        assert_eq!(
            git.output_untrimmed(&args).await.expect("UTF-8 output"),
            contents
        );
        assert_eq!(
            git.output(&args).await.expect("trimmed output"),
            "исходный текст\n  second line"
        );
    }

    #[tokio::test]
    async fn text_output_rejects_invalid_utf8_while_raw_commands_accept_it() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        let contents = [0xff, b'a', 0xfe];
        let hash = blob(&git, dir.path(), &contents).await;
        let args = ["cat-file", "blob", &hash];
        let output = git.command(&args).await.expect("raw binary output");
        assert!(output.status.success());
        assert_eq!(output.stdout, contents);
        git.run(&args)
            .await
            .expect("run checks only the exit status");

        for error in [
            git.output(&args).await.expect_err("invalid UTF-8"),
            git.output_untrimmed(&args)
                .await
                .expect_err("invalid UTF-8"),
        ] {
            let SourceStorageError::GitOutputUtf8 { command, source } = error else {
                panic!("expected UTF-8 error, got {error:?}");
            };
            assert_eq!(command, format!("git cat-file blob {hash}"));
            assert_eq!(source.as_bytes(), contents);
        }
    }

    #[tokio::test]
    async fn missing_working_directory_returns_spawn_error_with_command_context() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let missing = dir.path().join("missing repository");
        let error = Git::new(&missing)
            .command(&["status"])
            .await
            .expect_err("working directory does not exist");
        let SourceStorageError::GitSpawn { command, source } = error else {
            panic!("expected spawn error, got {error:?}");
        };
        assert_eq!(command, "git status");
        assert_eq!(source.kind(), ErrorKind::NotFound);
        assert!(!missing.exists());
    }

    #[tokio::test]
    async fn missing_executable_returns_spawn_error_with_command_context() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let command = Command::new(dir.path().join("missing-git"));
        let error = run_git_command(
            command,
            &["push", "origin", "HEAD:main"],
            Duration::from_secs(3),
        )
        .await
        .expect_err("executable does not exist");
        let SourceStorageError::GitSpawn { command, source } = error else {
            panic!("expected spawn error, got {error:?}");
        };
        assert_eq!(command, "git push origin HEAD:main");
        assert_eq!(source.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn clean_check_reports_changes_without_recovering_them() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        commit(&git).await;
        git.ensure_clean().await.expect("clean repository");

        let path = dir.path().join("pending.txt");
        fs::write(&path, "original\n").expect("untracked file");
        for (changes, contents) in [
            ("?? pending.txt", "original\n"),
            ("A  pending.txt", "original\n"),
            (" M pending.txt", "modified\n"),
        ] {
            if changes == "A  pending.txt" {
                git.run(&["add", "--", "pending.txt"])
                    .await
                    .expect("stage file");
            } else if changes == " M pending.txt" {
                commit(&git).await;
                fs::write(&path, contents).expect("modify tracked file");
            }
            let head = git
                .output(&["rev-parse", "HEAD"])
                .await
                .expect("HEAD before check");
            let staged = git
                .output(&["diff", "--cached", "--name-status"])
                .await
                .expect("staged changes before check");
            let error = git.ensure_clean().await.expect_err("dirty repository");
            let SourceStorageError::DirtySourceRepository {
                path: actual_path,
                changes: actual_changes,
            } = error
            else {
                panic!("expected dirty repository error, got {error:?}");
            };
            assert_eq!(actual_path, dir.path());
            assert_eq!(actual_changes, changes);
            assert_eq!(fs::read_to_string(&path).expect("preserved file"), contents);
            assert_eq!(
                git.output(&["rev-parse", "HEAD"])
                    .await
                    .expect("HEAD after check"),
                head
            );
            assert_eq!(
                git.output(&["diff", "--cached", "--name-status"])
                    .await
                    .expect("staged changes after check"),
                staged
            );
        }
    }

    #[tokio::test]
    async fn explicit_author_and_timestamp_override_repository_config_and_preserve_commit_errors() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        let timestamp = 1_600_000_000;
        git.run_with_author(
            &[
                "commit",
                "--allow-empty",
                "--no-gpg-sign",
                "-m",
                "Explicit author",
            ],
            "Explicit Author",
            "explicit@example.invalid",
            timestamp,
        )
        .await
        .expect("commit with explicit identity");
        let identity = git
            .output(&["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%at %ct"])
            .await
            .expect("commit identity");
        assert_eq!(
            identity,
            format!(
                "Explicit Author <explicit@example.invalid>\nExplicit Author <explicit@example.invalid>\n{timestamp} {timestamp}"
            )
        );
        let head = git
            .output(&["rev-parse", "HEAD"])
            .await
            .expect("HEAD before failed commit");
        let args = ["commit", "--no-gpg-sign", "--definitely-not-a-git-option"];
        let output = git.command(&args).await.expect("failed Git command");
        assert!(!output.status.success());
        let error = git
            .run_with_author(
                &args,
                "Another Author",
                "another@example.invalid",
                timestamp,
            )
            .await
            .expect_err("commit must fail");
        assert_git_failure(
            error,
            "git commit --no-gpg-sign --definitely-not-a-git-option",
            &output,
        );
        assert_eq!(
            git.output(&["rev-parse", "HEAD"])
                .await
                .expect("HEAD after failed commit"),
            head
        );
    }
}
