use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use tempfile::TempDir;

const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MINIMAL_CONFIG: &str = "[source_repository]\npath = 'source-repo'\n";

fn fixture(config: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temporary CLI directory");
    fs::write(dir.path().join("config.toml"), config).expect("CLI config");
    dir
}

fn cli(dir: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_verifier-cli"));
    command
        .current_dir(dir.path())
        .env_remove("VERIFIER_CONFIG")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    command
}

fn assert_success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).expect("UTF-8 CLI output")
}

fn assert_failure(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "{stderr}");
}

#[test]
fn prepares_sorted_unique_hashes_from_hex_and_all_base64_formats() {
    let dir = fixture(MINIMAL_CONFIG);
    prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let lower_hash = "ab".repeat(32);
    let upper_hash = "fb".repeat(32);
    let output = cli(&dir)
        .args(["remove-registry", "--dry-run", "--"])
        .args([
            upper_hash.to_uppercase(),
            STANDARD.encode([0xfb; 32]),
            STANDARD_NO_PAD.encode([0xfb; 32]),
            URL_SAFE.encode([0xfb; 32]),
            URL_SAFE_NO_PAD.encode([0xfb; 32]),
            format!("  {}\n", lower_hash.to_uppercase()),
            lower_hash.clone(),
        ])
        .output()
        .expect("run CLI");
    let stdout = assert_success(output);

    assert!(stdout.contains(&format!("Code hashes: {lower_hash}, {upper_hash}\n")));
    assert!(stdout.contains("Dry run: registry removal plan"));
    assert!(stdout.contains(&format!("  {lower_hash}: not found\n")));
    assert!(stdout.contains(&format!("  {upper_hash}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 0\nNot found: 2\n"));
    assert!(!stdout.contains("Delete:"));
    assert!(stdout.contains("Remote: origin\n"));
    assert!(stdout.contains("Storage root: sources\n"));
    assert!(stdout.contains("Branch: main\n"));
    assert!(!stdout.contains("Reason:"));
}

#[test]
fn uses_configured_repository_settings() {
    let dir = fixture(
        "[source_repository]\n\
         path = 'custom repo'\n\
         storage_root = 'verified/contracts'\n\
         remote = 'upstream'\n\
         branch = 'registry'\n\
         author_name = 'Registry Operator'\n\
         author_email = 'operator@example.invalid'\n\
         commit_enabled = false\n\
         push_enabled = false\n",
    );
    prepare_repository(
        &dir,
        "custom repo",
        "verified/contracts",
        "upstream",
        "registry",
    );
    let output = cli(&dir)
        .args(["remove-registry", "--dry-run", CODE_HASH])
        .output()
        .expect("run CLI");
    let stdout = assert_success(output);

    for expected in [
        "Repository: custom repo\n",
        "Storage root: verified/contracts\n",
        "Remote: upstream\n",
        "Branch: registry\n",
        &format!("    Path: {}\n", bundle_path("verified/contracts")),
        &format!("  {CODE_HASH}: found\n"),
    ] {
        assert!(stdout.contains(expected), "{stdout}");
    }
}

#[test]
fn config_path_uses_flag_then_environment_then_default() {
    let dir = fixture(MINIMAL_CONFIG);
    prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    for (filename, repository) in [("env.toml", "env-repo"), ("flag.toml", "flag-repo")] {
        fs::write(
            dir.path().join(filename),
            format!("[source_repository]\npath = '{repository}'\n"),
        )
        .expect("alternative config");
        prepare_repository(&dir, repository, "sources", "origin", "main");
    }

    for (environment, explicit, expected_repository) in [
        (None, None, "source-repo"),
        (Some("env.toml"), None, "env-repo"),
        (Some("missing-env.toml"), Some("flag.toml"), "flag-repo"),
    ] {
        let mut command = cli(&dir);
        command.args(["remove-registry", "--dry-run", CODE_HASH]);
        if let Some(config_path) = environment {
            command.env("VERIFIER_CONFIG", config_path);
        }
        if let Some(config_path) = explicit {
            command.args(["--config", config_path]);
        }
        let stdout = assert_success(command.output().expect("run CLI"));
        assert!(
            stdout.contains(&format!("Repository: {expected_repository}\n")),
            "{stdout}"
        );
    }
}

#[test]
fn rejects_missing_or_empty_source_repository_path() {
    for config in ["", "[source_repository]\npath = ''\n"] {
        let dir = fixture(config);
        let output = cli(&dir)
            .env("PATH", "")
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        assert_failure(
            &output,
            "source_repository.path must be set to a non-empty path in config config.toml",
        );
    }
}

#[test]
fn rejects_invalid_hashes_without_partial_output() {
    let dir = fixture(MINIMAL_CONFIG);
    for invalid in [
        String::new(),
        "  ".to_owned(),
        "not-a-hash".to_owned(),
        "g".repeat(64),
        "é".repeat(32),
        "a".repeat(63),
        "a".repeat(65),
        STANDARD.encode([0; 31]),
        STANDARD.encode([0; 33]),
    ] {
        for dry_run in [false, true] {
            let mut command = cli(&dir);
            // Invalid input must fail before Git or any other executable runs.
            command.env("PATH", "");
            command.arg("remove-registry");
            if dry_run {
                command.arg("--dry-run");
            }
            let output = command
                .args([CODE_HASH, &invalid, CODE_HASH])
                .output()
                .expect("run CLI");
            assert_failure(&output, &format!("invalid code hash {invalid:?}"));
            assert_failure(
                &output,
                "expected a 256-bit code hash in hexadecimal or base64",
            );
        }
    }
}

#[test]
fn reports_missing_config_with_its_path() {
    let dir = fixture(MINIMAL_CONFIG);
    let output = cli(&dir)
        .args([
            "remove-registry",
            "--dry-run",
            "--config",
            "missing.toml",
            CODE_HASH,
        ])
        .output()
        .expect("run CLI");
    assert_failure(&output, "failed to read config missing.toml");
}

#[test]
fn reports_config_parse_and_validation_errors() {
    for (config, expected) in [
        ("[source_repository", "failed to parse config config.toml"),
        (
            "[compiler]\nmax_concurrent_compilations = 0\n",
            "compiler max_concurrent_compilations must be -1 or a positive integer, got 0",
        ),
    ] {
        let dir = fixture(config);
        let output = cli(&dir)
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        assert_failure(&output, expected);
    }
}

#[test]
fn absent_hashes_succeed_without_commit_or_push() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let hash = "bb".repeat(32);
    let stdout = assert_success(run_preserving(&dir, &repo, &["remove-registry", &hash]));
    assert!(stdout.contains(&format!("  {hash}: not found\n")));
    assert!(stdout.contains("Removed bundles: 0\nNot found: 1\n"));
    assert!(stdout.contains("No bundles found; nothing to remove.\n"));
    assert!(!stdout.contains("Commit:"));
    assert!(!stdout.contains("Push:"));
}

#[test]
fn checks_preserve_repository_and_database_files() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let files = [
        ("verifier-index.sqlite3", "registry sentinel"),
        ("verifier-payments.sqlite3", "payment ledger sentinel"),
    ];
    for (filename, contents) in files {
        fs::write(dir.path().join(filename), contents).expect("sentinel file");
    }
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stdout = assert_success(output);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        stdout.contains(&format!("Local HEAD: {head}\n")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("Remote HEAD: {head}\n")),
        "{stdout}"
    );

    for (filename, contents) in files {
        assert_eq!(
            fs::read_to_string(dir.path().join(filename)).expect("preserved file"),
            contents,
            "{filename}"
        );
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("config.toml")).expect("preserved config"),
        MINIMAL_CONFIG
    );
}

#[test]
fn rejects_missing_checkout_without_creating_it() {
    let dir = fixture(MINIMAL_CONFIG);
    let before = directory_entries(dir.path());
    for dry_run in [false, true] {
        let mut command = cli(&dir);
        command.args(["remove-registry", CODE_HASH]);
        if dry_run {
            command.arg("--dry-run");
        }
        assert_failure(&command.output().expect("run CLI"), "path does not exist");
        assert_eq!(directory_entries(dir.path()), before);
    }
}

#[test]
fn rejects_non_git_bare_and_unborn_repositories() {
    for state in ["non-git", "bare", "unborn"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = dir.path().join("source-repo");
        fs::create_dir(&repo).expect("repository directory");
        match state {
            "bare" => {
                git(&repo, &["init", "--bare"]);
            }
            "unborn" => {
                git(&repo, &["init", "-b", "main"]);
            }
            _ => {}
        }
        let before = directory_entries(dir.path());
        let output = cli(&dir)
            .args(["remove-registry", "--dry-run", CODE_HASH])
            .output()
            .expect("run CLI");
        let expected = match state {
            "bare" => "path is not inside a git work tree",
            "unborn" => "git rev-parse --verify HEAD",
            _ => "not a git repository",
        };
        assert_failure(&output, expected);
        assert_eq!(directory_entries(dir.path()), before, "{state}");
    }
}

#[test]
fn rejects_checkout_subdirectory() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    fs::write(
        dir.path().join("config.toml"),
        "[source_repository]\npath = 'source-repo/sources'\n",
    )
    .expect("config");
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    assert_failure(&output, "must point to the checkout root");
}

#[test]
fn rejects_detached_head_even_with_configured_branch() {
    for branch in ["", "branch = 'main'\n"] {
        let dir = fixture(&format!("{MINIMAL_CONFIG}{branch}"));
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        git(&repo, &["checkout", "--detach"]);
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "has detached HEAD; check out a named branch");
    }
}

#[test]
fn rejects_unexpected_branch() {
    let dir = fixture(&format!("{MINIMAL_CONFIG}branch = 'registry'\n"));
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    assert_failure(&output, "branch mismatch: expected registry, current main");
}

#[test]
fn rejects_dirty_checkout_without_startup_recovery() {
    for state in ["unstaged", "staged", "untracked", "deleted", "mixed"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let source = repo.join(bundle_path("sources")).join("source.tolk");
        match state {
            "untracked" => fs::write(source.with_file_name("pending.tolk"), "untracked\n"),
            "deleted" => fs::remove_file(&source),
            _ => fs::write(&source, "modified\n"),
        }
        .expect("dirty checkout");
        if matches!(state, "staged" | "mixed") {
            git(&repo, &["add", "--all"]);
        }
        if state == "mixed" {
            fs::write(&source, "unstaged after staging\n").expect("unstaged content");
            fs::write(source.with_file_name("pending.tolk"), "untracked\n")
                .expect("untracked content");
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "has uncommitted changes");
    }
}

#[test]
fn uses_shared_source_repository_validation() {
    for state in ["invalid-root", "missing-rule", "overridden-rule"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let attributes = match state {
            "overridden-rule" => "sources/** -text\nsources/** text\n",
            _ => "sources/** text\n",
        };
        if state == "invalid-root" {
            let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
            git(&repo, &["checkout", "-b", "invalid-root", &root]);
        }
        fs::write(repo.join(".gitattributes"), attributes).expect("attributes");
        git(&repo, &["add", "--all"]);
        if state == "invalid-root" {
            git(
                &repo,
                &["commit", "--amend", "-m", "Invalid initial attributes"],
            );
        } else {
            git(&repo, &["commit", "-m", "Invalid current attributes"]);
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "is not prepared");
        assert_failure(
            &output,
            match state {
                "invalid-root" => "root `.gitattributes` must contain exactly",
                "missing-rule" => "current `.gitattributes` must contain",
                _ => "is not effective",
            },
        );
    }
}

#[test]
fn allows_missing_storage_and_bundle_directories() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&repo, &["checkout", "-b", "empty", &root]);
    git(&repo, &["push", "origin", "HEAD:empty"]);
    assert!(!repo.join("sources").exists());
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stdout = assert_success(output);
    assert!(stdout.contains(&format!("  {CODE_HASH}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 0\nNot found: 1\n"));
    assert!(!repo.join("sources").exists());
}

#[test]
fn rejects_local_head_behind_ahead_or_diverged_from_remote() {
    for state in ["behind", "ahead", "diverged"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let remote = dir.path().join("source-repo.git");
        let original = git(&repo, &["rev-parse", "HEAD"]);
        if matches!(state, "behind" | "diverged") {
            git(&repo, &["checkout", "-b", "other-writer"]);
            git(
                &repo,
                &["commit", "--allow-empty", "-m", "Remote advancement"],
            );
            // Push by URL so the local tracking reference stays stale.
            git(
                &repo,
                &["push", remote.to_str().expect("remote path"), "HEAD:main"],
            );
            git(&repo, &["checkout", "main"]);
        }
        if matches!(state, "ahead" | "diverged") {
            git(
                &repo,
                &["commit", "--allow-empty", "-m", "Local advancement"],
            );
        }
        assert_eq!(
            git(&repo, &["rev-parse", "refs/remotes/origin/main"]),
            original
        );
        let local_head = git(&repo, &["rev-parse", "HEAD"]);
        let remote_head = git(&remote, &["rev-parse", "refs/heads/main"]);
        assert_ne!(local_head, remote_head);
        for dry_run in [false, true] {
            let args = if dry_run {
                vec!["remove-registry", "--dry-run", CODE_HASH]
            } else {
                vec!["remove-registry", CODE_HASH]
            };
            let output = run_preserving(&dir, &repo, &args);
            assert_failure(&output, "HEAD does not match origin/main");
            assert_failure(&output, &format!("local HEAD={local_head}"));
            assert_failure(&output, &format!("remote HEAD={remote_head}"));
        }
    }
}

#[test]
fn accepts_matching_remote_even_with_stale_or_missing_tracking_reference() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&repo, &["update-ref", "refs/remotes/origin/main", &root]);
    for missing in [false, true] {
        if missing {
            git(&repo, &["update-ref", "-d", "refs/remotes/origin/main"]);
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_success(output);
    }
}

#[test]
fn rejects_unavailable_remote_and_missing_branch() {
    for state in ["unavailable", "missing-branch", "unconfigured"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let remote = dir.path().join("source-repo.git");
        match state {
            "missing-branch" => {
                git(&remote, &["update-ref", "-d", "refs/heads/main"]);
            }
            "unconfigured" => {
                git(&repo, &["remote", "remove", "origin"]);
            }
            _ => {
                git(
                    &repo,
                    &["remote", "set-url", "origin", "../unavailable.git"],
                );
            }
        }
        let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
        assert_failure(&output, "git command failed");
        assert_failure(
            &output,
            "git ls-remote --exit-code --refs -- origin refs/heads/main",
        );
        assert!(!dir.path().join("unavailable.git").exists());
    }
}

#[test]
fn checks_the_configured_branch_instead_of_remote_head() {
    let dir = fixture(&format!("{MINIMAL_CONFIG}branch = 'registry/production'\n"));
    let repo = prepare_repository(
        &dir,
        "source-repo",
        "sources",
        "origin",
        "registry/production",
    );
    let remote = dir.path().join("source-repo.git");
    let root = git(&repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(&remote, &["update-ref", "refs/heads/unrelated", &root]);
    git(&remote, &["symbolic-ref", "HEAD", "refs/heads/unrelated"]);
    let output = run_preserving(&dir, &repo, &["remove-registry", "--dry-run", CODE_HASH]);
    let stdout = assert_success(output);
    assert!(stdout.contains("Branch: registry/production\n"), "{stdout}");
}

#[test]
fn dry_run_prints_a_stable_complete_plan_for_present_and_absent_bundles() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let first = bundle_path("sources");
    let second_hash = "bb".repeat(32);
    let second = bundle_path_for("sources", &second_hash);
    let absent_hash = "ab".repeat(32);
    let absent = bundle_path_for("sources", &absent_hash);
    for (relative, contents) in [
        (format!("{first}/abi.json"), "{}"),
        (format!("{first}/src/lib.tolk"), "// nested source"),
        // No manifest: the directory layout alone identifies a bundle.
        (format!("{second}/source.tolk"), "// second source"),
        (
            format!("{first}-sibling/source.tolk"),
            "// outside selected directory",
        ),
        (
            format!("{absent}-sibling/source.tolk"),
            "// not the absent bundle",
        ),
        (
            format!(
                "{}/source.tolk",
                bundle_path_for("sources", &"cc".repeat(32))
            ),
            "// unselected bundle",
        ),
    ] {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent directory")).expect("source directory");
        fs::write(path, contents).expect("source file");
    }
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add plan fixtures"]);
    git(&repo, &["push", "origin", "HEAD:main"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let expected = format!(
        "Dry run: registry removal plan\n\
         Config: config.toml\n\
         Repository: source-repo\n\
         Storage root: sources\n\
         Remote: origin\n\
         Branch: main\n\
         Local HEAD: {head}\n\
         Remote HEAD: {head}\n\
         Code hashes: {CODE_HASH}, {absent_hash}, {second_hash}\n\
         Bundles:\n  {CODE_HASH}: found\n    Path: {first}\n  {absent_hash}: not found\n    Path: {absent}\n  {second_hash}: found\n    Path: {second}\n\
         Bundles to remove: 2\n\
         Not found: 1\n"
    );
    for hashes in [
        vec![second_hash.as_str(), absent_hash.as_str(), CODE_HASH],
        vec![
            CODE_HASH,
            absent_hash.as_str(),
            second_hash.as_str(),
            CODE_HASH,
        ],
    ] {
        let mut args = vec!["remove-registry", "--dry-run"];
        args.extend(hashes);
        let stdout = assert_success(run_preserving(&dir, &repo, &args));
        assert_eq!(stdout, expected);
    }
}

#[test]
fn plans_and_removes_only_tracked_files_in_a_valid_repository() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    fs::write(repo.join(".git/info/exclude"), "*.cache\n").expect("ignore rules");
    let absent_hash = "bb".repeat(32);
    let absent = bundle_path_for("sources", &absent_hash);
    let selected = bundle_path("sources");
    let ignored = [
        "local.cache".to_owned(),
        format!("{selected}/pending.cache"),
        format!("{selected}/nested/pending.cache"),
        format!("{absent}/pending.cache"),
        format!("{selected}-sibling/pending.cache"),
    ];
    for relative in &ignored {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent directory")).expect("ignored directory");
        fs::write(path, "local contents").expect("ignored file");
    }
    let stdout = assert_success(run_preserving(
        &dir,
        &repo,
        &["remove-registry", "--dry-run", CODE_HASH, &absent_hash],
    ));
    assert!(stdout.contains(&format!("  {absent_hash}: not found\n")));
    assert!(stdout.contains("Bundles to remove: 1\nNot found: 1\n"));
    assert!(!stdout.contains(".cache"));
    assert_success(
        cli(&dir)
            .args(["remove-registry", CODE_HASH, &absent_hash])
            .output()
            .expect("run CLI"),
    );
    for relative in &ignored {
        assert_eq!(
            fs::read_to_string(repo.join(relative)).expect("ignored contents"),
            "local contents"
        );
    }
    assert_eq!(git(&repo, &["ls-files"]), ".gitattributes");
}

#[test]
fn plans_and_removes_literal_paths_with_git_pathspec_characters() {
    for (root, other_root) in [("sources*", "sources-other"), (":(glob)sources", "sources")] {
        let dir = fixture(&format!("{MINIMAL_CONFIG}storage_root = '{root}'\n"));
        let repo = prepare_repository(&dir, "source-repo", root, "origin", "main");
        let selected = bundle_path(root);
        let other = bundle_path(other_root);
        fs::create_dir_all(repo.join(&other)).expect("other storage directory");
        fs::write(
            repo.join(format!("{other}/outside.tolk")),
            "// not selected",
        )
        .expect("other source");
        for filename in ["contract[1]*?.tolk", "source with spaces.tolk "] {
            fs::write(repo.join(&selected).join(filename), "// selected").expect("source filename");
        }
        git(&repo, &["add", "--all"]);
        git(&repo, &["commit", "-m", "Add literal path fixtures"]);
        git(&repo, &["push", "origin", "HEAD:main"]);
        let stdout = assert_success(run_preserving(
            &dir,
            &repo,
            &["remove-registry", "--dry-run", CODE_HASH],
        ));
        assert!(stdout.contains(&format!("  {CODE_HASH}: found\n    Path: {selected}\n")));
        assert_success(
            cli(&dir)
                .args(["remove-registry", CODE_HASH])
                .output()
                .expect("run CLI"),
        );
        assert!(!repo.join(selected).exists());
        assert_eq!(
            fs::read_to_string(repo.join(format!("{other}/outside.tolk")))
                .expect("unselected source"),
            "// not selected"
        );
        assert_eq!(git(&repo, &["diff", "--name-only"]), "");
    }
}

#[test]
fn removes_selected_bundles_without_changing_head_remote_or_unrelated_files() {
    let dir = fixture(
        "[source_repository]\n\
         path = 'source-repo'\n\
         storage_root = 'verified/contracts'\n\
         remote = 'upstream'\n\
         branch = 'registry'\n\
         commit_enabled = false\n\
         push_enabled = false\n",
    );
    let root = "verified/contracts";
    let repo = prepare_repository(&dir, "source-repo", root, "upstream", "registry");
    let remote = dir.path().join("source-repo.git");
    let first = bundle_path(root);
    let second_hash = "bb".repeat(32);
    let second = bundle_path_for(root, &second_hash);
    let absent_hash = "ab".repeat(32);
    let absent = bundle_path_for(root, &absent_hash);
    let remaining_hash = "cc".repeat(32);
    let remaining = bundle_path_for(root, &remaining_hash);
    for (relative, contents) in [
        (format!("{first}/abi.json"), "{}"),
        (format!("{first}/src/lib.tolk"), "// nested source"),
        (format!("{second}/source.tolk"), "// no manifest"),
        (format!("{first}-sibling/source.tolk"), "// sibling"),
        (format!("{absent}-sibling/source.tolk"), "// absent sibling"),
        (format!("{remaining}/source.tolk"), "// remaining bundle"),
    ] {
        let path = repo.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("bundle directory");
        fs::write(path, contents).expect("source file");
    }
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add removal fixtures"]);
    git(&repo, &["push", "upstream", "HEAD:registry"]);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let remote_before = directory_entries(&remote);
    let refs_before = git(&repo, &["show-ref"]);
    let config = fs::read(dir.path().join("config.toml")).expect("config bytes");
    for filename in ["registry.sqlite", "payments.sqlite", "blocklist.txt"] {
        fs::write(dir.path().join(filename), "unrelated contents").expect("unrelated file");
    }

    let stdout = assert_success(
        cli(&dir)
            .args([
                "remove-registry",
                &second_hash,
                &absent_hash,
                CODE_HASH,
                CODE_HASH,
            ])
            .output()
            .expect("run CLI"),
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&repo, &["show-ref"]), refs_before);
    assert_eq!(directory_entries(&remote), remote_before);
    assert_eq!(git(&repo, &["diff", "--name-only"]), "");
    assert_eq!(
        git(
            &repo,
            &["diff", "--cached", "--no-renames", "--name-status"]
        ),
        format!(
            "D\t{first}/abi.json\nD\t{first}/manifest.json\nD\t{first}/source.tolk\nD\t{first}/src/lib.tolk\nD\t{second}/source.tolk"
        )
    );
    assert!(!repo.join(&first).exists());
    assert!(!repo.join(second).exists());
    for (relative, expected) in [
        (format!("{remaining}/source.tolk"), "// remaining bundle"),
        (format!("{first}-sibling/source.tolk"), "// sibling"),
        (format!("{absent}-sibling/source.tolk"), "// absent sibling"),
    ] {
        assert_eq!(
            fs::read_to_string(repo.join(relative)).expect("unselected source"),
            expected
        );
    }
    assert!(stdout.contains(&format!("  {absent_hash}: not found\n")));
    assert!(stdout.contains("Removed bundles: 2\nNot found: 1\n"));
    assert!(stdout.contains("Deletions staged in Git.\n"));
    for removed_field in [
        "Commit:",
        "Push:",
        "Author:",
        "source_bundle_hash:",
        "manifest_hash:",
    ] {
        assert!(!stdout.contains(removed_field), "{stdout}");
    }
    assert_eq!(
        fs::read(dir.path().join("config.toml")).expect("config bytes"),
        config
    );
    for filename in ["registry.sqlite", "payments.sqlite", "blocklist.txt"] {
        assert_eq!(
            fs::read_to_string(dir.path().join(filename)).expect("unrelated file"),
            "unrelated contents"
        );
    }
    // Staged deletions remain dirty until the operator commits or restores them.
    let output = run_preserving(&dir, &repo, &["remove-registry", CODE_HASH]);
    assert_failure(&output, "uncommitted changes");
}

#[test]
fn removes_the_last_bundle_without_creating_a_commit() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    assert_success(
        cli(&dir)
            .args(["remove-registry", CODE_HASH])
            .output()
            .expect("run CLI"),
    );
    assert_eq!(git(&repo, &["ls-files"]), ".gitattributes");
    assert!(!repo.join(bundle_path("sources")).exists());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        original
    );
}

#[cfg(unix)]
#[test]
fn partial_removal_and_unexpected_staged_changes_restore_only_planned_paths() {
    for failure in ["remove", "staged-diff"] {
        let dir = fixture(MINIMAL_CONFIG);
        let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
        let original = git(&repo, &["rev-parse", "HEAD"]);
        let bundle = repo.join(bundle_path("sources"));
        let before = directory_entries(&bundle);
        let output = cli_with_git_script(
            &dir,
            r#"
if [ "$1" = --literal-pathspecs ] && [ "$2" = rm ]; then
    if [ "$CLI_TEST_FAILURE" = remove ]; then
        "$CLI_TEST_REAL_GIT" --literal-pathspecs rm -- "$CLI_TEST_FIRST_FILE"
    else
        "$CLI_TEST_REAL_GIT" "$@"
    fi
    printf 'unrelated staged contents\n' > unrelated.txt
    "$CLI_TEST_REAL_GIT" add -- unrelated.txt
    printf 'unrelated working contents\n' >> unrelated.txt
    if [ "$CLI_TEST_FAILURE" = remove ]; then
        echo 'injected removal failure' >&2
        exit 1
    fi
    exit 0
fi
"#,
        )
        .env("CLI_TEST_FAILURE", failure)
        .env(
            "CLI_TEST_FIRST_FILE",
            format!("{}/source.tolk", bundle_path("sources")),
        )
        .args(["remove-registry", CODE_HASH])
        .output()
        .expect("run CLI");
        assert_failure(
            &output,
            if failure == "remove" {
                "injected removal failure"
            } else {
                "staged diff does not match the removal plan"
            },
        );
        assert_failure(&output, "planned files and index entries were restored");
        assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
        assert_eq!(
            git(
                &dir.path().join("source-repo.git"),
                &["rev-parse", "refs/heads/main"]
            ),
            original
        );
        assert_eq!(directory_entries(&bundle), before);
        assert_eq!(
            git(&repo, &["diff", "--cached", "--name-status"]),
            "A\tunrelated.txt"
        );
        assert_eq!(
            git(&repo, &["show", ":unrelated.txt"]),
            "unrelated staged contents"
        );
        assert_eq!(
            fs::read_to_string(repo.join("unrelated.txt")).expect("unrelated worktree file"),
            "unrelated staged contents\nunrelated working contents\n"
        );
    }
}

#[cfg(unix)]
#[test]
fn recovery_failure_reports_both_errors_and_leaves_head_unchanged() {
    let dir = fixture(MINIMAL_CONFIG);
    let repo = prepare_repository(&dir, "source-repo", "sources", "origin", "main");
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let output = cli_with_git_script(
        &dir,
        r#"
if [ "$1" = --literal-pathspecs ]; then
    case "$2" in
        rm) "$CLI_TEST_REAL_GIT" "$@"; echo 'injected removal failure' >&2; exit 1 ;;
        restore) echo 'injected restore failure' >&2; exit 1 ;;
    esac
fi
"#,
    )
    .args(["remove-registry", CODE_HASH])
    .output()
    .expect("run CLI");
    assert_failure(&output, "injected removal failure");
    assert_failure(&output, "injected restore failure");
    assert_failure(&output, "inspect git status before retrying");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(
        git(
            &dir.path().join("source-repo.git"),
            &["rev-parse", "refs/heads/main"]
        ),
        original
    );
    assert!(!git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
}

#[cfg(unix)]
fn executable_script(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, script).expect("test script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable script");
}

#[cfg(unix)]
fn cli_with_git_script(dir: &TempDir, script: &str) -> Command {
    let original_path = std::env::var_os("PATH").expect("PATH");
    let paths = std::env::split_paths(&original_path).collect::<Vec<_>>();
    let real_git = paths
        .iter()
        .map(|path| path.join("git"))
        .find(|path| path.is_file())
        .expect("Git executable");
    let scripts = dir.path().join("test-bin");
    fs::create_dir(&scripts).expect("test scripts directory");
    executable_script(
        &scripts.join("git"),
        &format!("#!/bin/sh\nset -eu\n{script}\nexec \"$CLI_TEST_REAL_GIT\" \"$@\"\n"),
    );
    let path = std::env::join_paths(std::iter::once(scripts).chain(paths)).expect("test PATH");
    let mut command = cli(dir);
    command.env("PATH", path).env("CLI_TEST_REAL_GIT", real_git);
    command
}

fn bundle_path(storage_root: &str) -> String {
    bundle_path_for(storage_root, CODE_HASH)
}

fn bundle_path_for(storage_root: &str, hash: &str) -> String {
    format!("{storage_root}/{}/{}", &hash[..2], &hash[2..])
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Verifier CLI tests")
        .env("GIT_AUTHOR_EMAIL", "verifier@example.invalid")
        .env("GIT_COMMITTER_NAME", "Verifier CLI tests")
        .env("GIT_COMMITTER_EMAIL", "verifier@example.invalid")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .expect("run Git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF-8 Git output")
        .trim()
        .to_owned()
}

fn prepare_repository(
    dir: &TempDir,
    name: &str,
    storage_root: &str,
    remote: &str,
    branch: &str,
) -> PathBuf {
    let repo = dir.path().join(name);
    fs::create_dir(&repo).expect("checkout directory");
    git(&repo, &["init", "-b", branch]);
    fs::write(
        repo.join(".gitattributes"),
        format!("{storage_root}/** -text\n"),
    )
    .expect("initial attributes");
    git(&repo, &["add", ".gitattributes"]);
    git(&repo, &["commit", "-m", "Initialize source repository"]);
    let bundle = repo.join(bundle_path(storage_root));
    fs::create_dir_all(&bundle).expect("bundle directory");
    fs::write(bundle.join("source.tolk"), "// source fixture\n").expect("source file");
    // Checkout checks do not require a valid manifest or invoke compilers.
    fs::write(bundle.join("manifest.json"), "broken manifest\n").expect("manifest file");
    git(&repo, &["add", "--all"]);
    git(&repo, &["commit", "-m", "Add fixture bundle"]);
    let remote_path = dir.path().join(format!("{name}.git"));
    fs::create_dir(&remote_path).expect("remote directory");
    git(&remote_path, &["init", "--bare"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            remote,
            remote_path.to_str().expect("remote path"),
        ],
    );
    git(&repo, &["push", remote, &format!("HEAD:{branch}")]);
    repo
}

#[derive(Debug, PartialEq, Eq)]
enum EntrySnapshot {
    Directory,
    File(Vec<u8>, u32),
    Symlink(PathBuf),
}

fn directory_entries(path: &Path) -> BTreeMap<PathBuf, EntrySnapshot> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(path).expect("read fixture directory") {
        let entry = entry.expect("directory entry");
        let kind = entry.file_type().expect("entry type");
        let snapshot = if kind.is_dir() {
            entries.extend(directory_entries(&entry.path()));
            EntrySnapshot::Directory
        } else if kind.is_symlink() {
            EntrySnapshot::Symlink(fs::read_link(entry.path()).expect("symlink target"))
        } else {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                entry
                    .metadata()
                    .expect("file metadata")
                    .permissions()
                    .mode()
            };
            #[cfg(not(unix))]
            let mode = 0;
            EntrySnapshot::File(fs::read(entry.path()).expect("file bytes"), mode)
        };
        entries.insert(entry.path(), snapshot);
    }
    entries
}

fn run_preserving(dir: &TempDir, repo: &Path, args: &[&str]) -> Output {
    let index = repo.join(".git/index");
    let staged_before = git(repo, &["ls-files", "--stage", "-z"]);
    let mut before = directory_entries(dir.path());
    // git status may refresh stat cache; compare staged contents and modes separately.
    before.remove(&index);
    let output = cli(dir).args(args).output().expect("run CLI");
    let mut after = directory_entries(dir.path());
    after.remove(&index);
    assert_eq!(
        after, before,
        "CLI must preserve checkout, refs, remote and databases"
    );
    assert_eq!(
        git(repo, &["ls-files", "--stage", "-z"]),
        staged_before,
        "CLI must preserve staged state"
    );
    output
}
