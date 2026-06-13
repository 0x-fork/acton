use std::{
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::json;
use tempfile::TempDir;
use verifier::{
    config::Config,
    source_storage::{
        GitSourceStorage, SourceStorage, SourceStorageFile, SourceStorageProvider,
        SourceStorageSource, StoreSourceBundleRequest,
    },
};

const CODE_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SOURCE_BUNDLE_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[tokio::test]
async fn git_source_storage_commits_and_pushes_bundle() -> Result<(), Box<dyn Error>> {
    let fixture = GitFixture::new()?;
    let config_path = fixture.write_config()?;
    let config = Config::load_from_path(config_path)?;
    let storage = GitSourceStorage::from_config(&config);

    let receipt = storage
        .store_bundle(StoreSourceBundleRequest {
            address: Some("EQD0000000000000000000000000000000000000000000000".to_owned()),
            code_hash: CODE_HASH.to_owned(),
            source_bundle_hash: SOURCE_BUNDLE_HASH.to_owned(),
            language: "tolk".to_owned(),
            compiler_version: "1.4.1".to_owned(),
            entrypoint: "main.tolk".to_owned(),
            compile_params: json!({"compiler_version": "1.4.1"}),
            sources: vec![
                SourceStorageSource {
                    path: "main.tolk".to_owned(),
                    is_entrypoint: true,
                    include_in_command: None,
                    is_stdlib: None,
                    has_include_directives: None,
                },
                SourceStorageSource {
                    path: "imports/lib.tolk".to_owned(),
                    is_entrypoint: false,
                    include_in_command: None,
                    is_stdlib: None,
                    has_include_directives: None,
                },
            ],
            files: vec![
                SourceStorageFile {
                    path: "main.tolk".to_owned(),
                    content: b"import \"imports/lib.tolk\";".to_vec(),
                },
                SourceStorageFile {
                    path: "imports/lib.tolk".to_owned(),
                    content: b"fun helper() {}".to_vec(),
                },
            ],
        })
        .await?;

    assert!(matches!(receipt.provider, SourceStorageProvider::Git));
    assert_eq!(
        receipt.bundle_path,
        format!("sources/{CODE_HASH}/{SOURCE_BUNDLE_HASH}")
    );
    assert_eq!(receipt.commit.len(), 40);

    let stored_main = fixture
        .repo_path
        .join(&receipt.bundle_path)
        .join("files/main.tolk");
    let stored_lib = fixture
        .repo_path
        .join(&receipt.bundle_path)
        .join("files/imports/lib.tolk");
    let manifest = fixture
        .repo_path
        .join(&receipt.bundle_path)
        .join("manifest.json");

    assert_eq!(
        fs::read_to_string(stored_main)?,
        "import \"imports/lib.tolk\";"
    );
    assert_eq!(fs::read_to_string(stored_lib)?, "fun helper() {}");

    let manifest = serde_json::from_slice::<serde_json::Value>(&fs::read(manifest)?)?;
    assert_eq!(manifest["code_hash"], CODE_HASH);
    assert_eq!(manifest["source_bundle_hash"], SOURCE_BUNDLE_HASH);
    assert_eq!(manifest["entrypoint"], "main.tolk");
    assert_eq!(manifest["files"].as_array().map(Vec::len), Some(2));

    let stored_bundles = storage.list_bundles(CODE_HASH).await?;
    assert_eq!(stored_bundles.len(), 1);
    assert_eq!(stored_bundles[0].manifest.code_hash, CODE_HASH);
    assert_eq!(
        stored_bundles[0].manifest.source_bundle_hash,
        SOURCE_BUNDLE_HASH
    );
    assert_eq!(
        stored_bundles[0].commit.as_deref(),
        Some(receipt.commit.as_str())
    );
    assert_eq!(stored_bundles[0].files.len(), 2);
    assert_eq!(stored_bundles[0].files[0].path, "imports/lib.tolk");
    assert_eq!(
        stored_bundles[0].files[0].content_text.as_deref(),
        Some("fun helper() {}")
    );
    assert_eq!(stored_bundles[0].files[1].path, "main.tolk");
    assert_eq!(
        stored_bundles[0].files[1].content_text.as_deref(),
        Some("import \"imports/lib.tolk\";")
    );

    let remote_head = git_output(
        fixture.temp.path(),
        [
            "--git-dir",
            fixture
                .remote_path
                .to_str()
                .expect("remote path should be UTF-8"),
            "rev-parse",
            "refs/heads/main",
        ],
    )?;
    assert_eq!(remote_head, receipt.commit);

    Ok(())
}

struct GitFixture {
    temp: TempDir,
    repo_path: PathBuf,
    remote_path: PathBuf,
}

impl GitFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let temp = TempDir::new()?;
        let repo_path = temp.path().join("repo");
        let remote_path = temp.path().join("remote.git");

        assert_success(
            run_command(
                temp.path(),
                "git",
                ["init", "--bare", path_str(&remote_path)?],
            )?,
            "git init --bare",
        )?;
        assert_success(
            run_command(
                temp.path(),
                "git",
                ["init", "-b", "main", path_str(&repo_path)?],
            )?,
            "git init -b main",
        )?;
        assert_success(
            run_command(
                &repo_path,
                "git",
                ["remote", "add", "origin", path_str(&remote_path)?],
            )?,
            "git remote add",
        )?;

        Ok(Self {
            temp,
            repo_path,
            remote_path,
        })
    }

    fn write_config(&self) -> Result<PathBuf, Box<dyn Error>> {
        let path = self.temp.path().join("verifier.toml");
        fs::write(
            &path,
            format!(
                r#"
[source_repository]
path = "{}"
remote = "origin"
branch = "main"
author_name = "Verifier Bot"
author_email = "verifier@example.com"
"#,
                self.repo_path.display()
            ),
        )?;
        Ok(path)
    }
}

fn path_str(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.to_str()
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()).into())
}

fn run_command<I, S>(dir: &Path, program: &str, args: I) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(Command::new(program).args(args).current_dir(dir).output()?)
}

fn git_output<I, S>(dir: &Path, args: I) -> Result<String, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_command(dir, "git", args)?;
    let output = assert_success(output, "git output")?;
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn assert_success(output: Output, command: &str) -> Result<Output, Box<dyn Error>> {
    if output.status.success() {
        return Ok(output);
    }

    Err(format!(
        "{command} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
    .into())
}
