use std::{
    fs,
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
        // Request preparation must work without Git, Node.js or other executables.
        .env("PATH", "");
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
    assert!(stdout.contains("Request prepared only"));
    assert!(stdout.contains("repository checks and removal planning are not implemented yet"));
    assert!(stdout.contains("Remote: origin\n"));
    assert!(stdout.contains("Storage root: sources\n"));
    assert!(stdout.contains("Branch: current branch (not resolved yet)\n"));
    assert!(stdout.contains("Author: ton-verifier <ton-verifier@example.invalid>\n"));
    assert!(!stdout.contains("Reason:"));
    assert_eq!(fs::read_dir(dir.path()).expect("CLI directory").count(), 1);
}

#[test]
fn uses_configured_repository_settings_and_preserves_reason() {
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
    let output = cli(&dir)
        .args([
            "remove-registry",
            "--dry-run",
            "--reason",
            "Удаление тестовых публикаций",
            CODE_HASH,
        ])
        .output()
        .expect("run CLI");
    let stdout = assert_success(output);

    for expected in [
        "Repository: custom repo\n",
        "Storage root: verified/contracts\n",
        "Remote: upstream\n",
        "Branch: registry\n",
        "Author: Registry Operator <operator@example.invalid>\n",
        "Reason: Удаление тестовых публикаций\n",
    ] {
        assert!(stdout.contains(expected), "{stdout}");
    }
}

#[test]
fn config_path_uses_flag_then_environment_then_default() {
    let dir = fixture(MINIMAL_CONFIG);
    for (filename, repository) in [("env.toml", "env-repo"), ("flag.toml", "flag-repo")] {
        fs::write(
            dir.path().join(filename),
            format!("[source_repository]\npath = '{repository}'\n"),
        )
        .expect("alternative config");
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
fn refuses_execution_until_removal_is_implemented() {
    let dir = fixture(MINIMAL_CONFIG);
    let output = cli(&dir)
        .args(["remove-registry", CODE_HASH])
        .output()
        .expect("run CLI");
    assert_failure(&output, "registry removal is not implemented yet");
}

#[test]
fn preparation_preserves_repository_and_database_files() {
    let dir = fixture(MINIMAL_CONFIG);
    fs::create_dir_all(dir.path().join("source-repo/.git")).expect("repository directory");
    let files = [
        ("source-repo/.git/HEAD", "ref: refs/heads/main\n"),
        ("source-repo/.git/index", "Git index sentinel"),
        ("source-repo/pending.txt", "uncommitted data"),
        ("verifier-index.sqlite3", "registry sentinel"),
        ("verifier-payments.sqlite3", "payment ledger sentinel"),
    ];
    for (filename, contents) in files {
        fs::write(dir.path().join(filename), contents).expect("sentinel file");
    }
    let output = cli(&dir)
        .args(["remove-registry", "--dry-run", CODE_HASH])
        .output()
        .expect("run CLI");
    assert_success(output);

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
