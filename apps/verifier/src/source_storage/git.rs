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

    pub(super) const fn path(&self) -> &'a Path {
        self.path
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

    /// Checks that the existing path belongs to a Git work tree.
    ///
    /// # Errors
    /// Returns an error for a missing path, a bare repository or a failed Git command.
    pub async fn ensure_work_tree(&self) -> Result<(), SourceStorageError> {
        if !self.path.exists() {
            return Err(SourceStorageError::InvalidPath {
                path: self.path.to_path_buf(),
                message: "path does not exist".to_owned(),
            });
        }
        if self.output(&["rev-parse", "--is-inside-work-tree"]).await? == "true" {
            return Ok(());
        }
        Err(SourceStorageError::InvalidPath {
            path: self.path.to_path_buf(),
            message: "path is not inside a git work tree".to_owned(),
        })
    }

    /// Returns the local HEAD without contacting a remote.
    ///
    /// # Errors
    /// Returns an error if HEAD does not exist or Git fails.
    pub async fn head(&self) -> Result<String, SourceStorageError> {
        self.output(&["rev-parse", "--verify", "HEAD"]).await
    }

    /// Checks for files committed under a path at HEAD.
    ///
    /// # Errors
    /// Returns an error if HEAD does not exist or Git fails.
    pub async fn has_committed_files(&self, path: &str) -> Result<bool, SourceStorageError> {
        let files = self
            .output(&["ls-tree", "-r", "--name-only", "HEAD", "--", path])
            .await?;
        Ok(!files.is_empty())
    }

    /// Returns sorted tracked file paths under a literal directory path.
    ///
    /// # Errors
    /// Returns an error if Git fails or file paths are not UTF-8.
    pub async fn tracked_files(&self, directory: &str) -> Result<Vec<String>, SourceStorageError> {
        // A trailing slash selects the directory, not similarly named siblings.
        let path = format!("{directory}/");
        let output = self
            .output_untrimmed(&[
                "--literal-pathspecs",
                "ls-files",
                "--cached",
                "-z",
                "--",
                &path,
            ])
            .await?;
        let mut files = output
            .split_terminator('\0')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        files.sort();
        Ok(files)
    }

    /// Removes and stages exactly the supplied file paths, without pathspec expansion.
    ///
    /// # Errors
    /// Returns an error if Git cannot remove the files.
    pub async fn remove_files(&self, files: &[&str]) -> Result<(), SourceStorageError> {
        let mut args = vec!["--literal-pathspecs", "rm", "--"];
        args.extend_from_slice(files);
        self.run(&args).await
    }

    /// Checks that the entire staged diff consists of exactly the planned deletions.
    ///
    /// # Errors
    /// Returns an error if the staged diff differs from the plan or Git fails.
    pub async fn ensure_staged_deletions(
        &self,
        revision: &str,
        files: &[&str],
    ) -> Result<(), SourceStorageError> {
        let output = self
            .output_untrimmed(&[
                "diff",
                "--cached",
                "--no-renames",
                "--name-status",
                "-z",
                revision,
                "--",
            ])
            .await?;
        let fields = output.split_terminator('\0').collect::<Vec<_>>();
        let mut deleted = Vec::new();
        for change in fields.chunks(2) {
            let ["D", path] = change else {
                return Err(SourceStorageError::Operation(
                    "staged diff does not match the removal plan".to_owned(),
                ));
            };
            deleted.push(*path);
        }
        deleted.sort_unstable();
        let mut expected = files.to_vec();
        expected.sort_unstable();
        if deleted != expected {
            return Err(SourceStorageError::Operation(
                "staged diff does not match the removal plan".to_owned(),
            ));
        }
        Ok(())
    }

    /// Restores only the supplied paths in the index and work tree from a revision.
    ///
    /// # Errors
    /// Returns an error if Git cannot restore the files.
    pub async fn restore_files(
        &self,
        revision: &str,
        files: &[&str],
    ) -> Result<(), SourceStorageError> {
        let mut args = vec![
            "--literal-pathspecs",
            "restore",
            "--source",
            revision,
            "--staged",
            "--worktree",
            "--",
        ];
        args.extend_from_slice(files);
        self.run(&args).await
    }

    /// Pushes HEAD to an explicit remote branch, using the current branch as a default.
    ///
    /// # Errors
    /// Returns an error for detached HEAD without a configured branch or a failed push.
    pub async fn push_head(
        &self,
        remote: &str,
        branch: Option<&str>,
    ) -> Result<(), SourceStorageError> {
        let branch = match branch {
            Some(branch) => branch.to_owned(),
            None => self.current_branch().await?,
        };
        self.run(&["push", remote, &format!("HEAD:{branch}")]).await
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

    #[tokio::test]
    async fn work_tree_checks_do_not_initialize_paths_and_head_requires_a_commit() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        git.ensure_work_tree().await.expect("existing work tree");
        assert!(matches!(
            git.head().await,
            Err(SourceStorageError::Git { .. })
        ));
        assert!(matches!(
            git.has_committed_files("missing").await,
            Err(SourceStorageError::Git { .. })
        ));
        commit(&git).await;
        assert_eq!(
            git.head().await.expect("HEAD"),
            git.output(&["log", "-1", "--format=%H"])
                .await
                .expect("commit")
        );
        assert!(
            !git.has_committed_files("missing")
                .await
                .expect("absent path")
        );

        let child = dir.path().join("child");
        fs::create_dir(&child).expect("nested directory");
        Git::new(&child)
            .ensure_work_tree()
            .await
            .expect("backend accepts a path inside the work tree");

        let outside = tempfile::tempdir().expect("non-repository directory");
        assert!(matches!(
            Git::new(outside.path()).ensure_work_tree().await,
            Err(SourceStorageError::Git { .. })
        ));
        let missing = outside.path().join("missing");
        assert!(matches!(
            Git::new(&missing).ensure_work_tree().await,
            Err(SourceStorageError::InvalidPath { .. })
        ));
        assert!(!missing.exists(), "checking must not create a checkout");

        Git::new(outside.path())
            .run(&["init", "--bare"])
            .await
            .expect("bare repository");
        assert!(matches!(
            Git::new(outside.path()).ensure_work_tree().await,
            Err(SourceStorageError::InvalidPath { .. })
        ));
    }

    #[tokio::test]
    async fn commits_bundle_additions_and_deletions_without_including_other_staged_files() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        commit(&git).await;
        fs::create_dir(dir.path().join("bundle")).expect("bundle directory");
        for name in ["bundle/first.txt", "bundle/second.txt", "unrelated.txt"] {
            fs::write(dir.path().join(name), name).expect("source file");
        }
        git.run(&["add", "--", "."]).await.expect("stage files");
        assert!(
            !git.has_committed_files("bundle/first.txt")
                .await
                .expect("staged is not committed")
        );
        let timestamp = 1_600_000_000;
        git.run_with_author(
            &["commit", "-m", "Bundle additions", "--", "bundle"],
            "Operator",
            "operator@example.invalid",
            timestamp,
        )
        .await
        .expect("commit selected additions");
        assert_eq!(
            git.output(&["show", "--format=", "--name-status", "HEAD"])
                .await
                .expect("commit files"),
            "A\tbundle/first.txt\nA\tbundle/second.txt"
        );
        assert!(
            git.has_committed_files("bundle/first.txt")
                .await
                .expect("committed file")
        );
        assert_eq!(
            git.output(&["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%at %ct"])
                .await
                .expect("identity"),
            format!(
                "Operator <operator@example.invalid>\nOperator <operator@example.invalid>\n{timestamp} {timestamp}"
            )
        );

        git.run(&["rm", "--", "bundle/first.txt", "bundle/second.txt"])
            .await
            .expect("stage removals");
        git.run_with_author(
            &["commit", "-m", "Bundle removals", "--", "bundle"],
            "Operator",
            "operator@example.invalid",
            timestamp,
        )
        .await
        .expect("commit selected removals");
        assert_eq!(
            git.output(&["show", "--format=", "--name-status", "HEAD"])
                .await
                .expect("commit files"),
            "D\tbundle/first.txt\nD\tbundle/second.txt"
        );
        assert!(
            !git.has_committed_files("bundle/first.txt")
                .await
                .expect("deleted file")
        );
        assert_eq!(
            git.output(&["diff", "--cached", "--name-only"])
                .await
                .expect("remaining staged files"),
            "unrelated.txt"
        );
    }

    #[tokio::test]
    async fn push_uses_selected_remote_and_branch_and_preserves_non_fast_forward_errors() {
        let dir = repository().await;
        let git = Git::new(dir.path());
        commit(&git).await;
        let remote_dir = tempfile::tempdir().expect("remote directory");
        let remote = Git::new(remote_dir.path());
        remote
            .run(&["init", "--bare"])
            .await
            .expect("initialize remote");
        git.run(&[
            "remote",
            "add",
            "upstream",
            remote_dir.path().to_str().expect("remote path"),
        ])
        .await
        .expect("configure remote");
        git.push_head("upstream", None)
            .await
            .expect("push current branch");
        let first = git.head().await.expect("first commit");
        assert_eq!(
            remote
                .output(&["rev-parse", "refs/heads/main"])
                .await
                .expect("remote branch"),
            first
        );

        git.push_head("upstream", Some("registry"))
            .await
            .expect("create target branch from the named local branch");

        git.run(&["checkout", "--detach"])
            .await
            .expect("detach HEAD");
        assert!(matches!(
            git.push_head("upstream", None).await,
            Err(SourceStorageError::DetachedHead)
        ));
        commit(&git).await;
        let second = git.head().await.expect("second commit");
        git.push_head("upstream", Some("registry"))
            .await
            .expect("explicit branch works with detached HEAD");
        assert_eq!(
            remote
                .output(&["rev-parse", "refs/heads/registry"])
                .await
                .expect("target branch"),
            second
        );
        assert_eq!(
            remote
                .output(&["rev-parse", "refs/heads/main"])
                .await
                .expect("other branch"),
            first
        );

        git.run(&["checkout", "--detach", &first])
            .await
            .expect("older HEAD");
        let error = git
            .push_head("upstream", Some("registry"))
            .await
            .expect_err("no force push");
        assert!(
            matches!(&error, SourceStorageError::Git { command, .. } if command == "git push upstream HEAD:registry")
        );
        assert!(crate::error::ApiError::from(error).is_payment_retryable());
        assert_eq!(
            remote
                .output(&["rev-parse", "refs/heads/registry"])
                .await
                .expect("remote remains ahead"),
            second
        );
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
