use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{fs, process::Command, sync::Mutex};

use crate::config::Config;

const STORAGE_ROOT: &str = "sources";

#[async_trait]
pub trait SourceStorage: Send + Sync + 'static {
    async fn store_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError>;

    async fn list_bundles(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, SourceStorageError>;

    async fn list_code_hashes(&self) -> Result<Vec<String>, SourceStorageError>;

    async fn current_revision(&self) -> Result<Option<String>, SourceStorageError>;
}

pub type SharedSourceStorage = Arc<dyn SourceStorage>;

pub struct StoreSourceBundleRequest {
    pub code_hash: String,
    pub source_bundle_hash: String,
    pub compiler: CompilerMetadata,
    pub files: Vec<SourceStorageFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompilerMetadata {
    pub language: String,
    pub version: String,
    pub entrypoint: String,
    pub params: Value,
}

#[derive(Clone)]
pub struct SourceStorageFile {
    pub path: String,
    pub content: String,
    pub include_in_command: Option<bool>,
    pub is_stdlib: Option<bool>,
    pub has_include_directives: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceStorageReceipt {
    pub revision: String,
}

#[derive(Clone, Default)]
pub struct DisabledSourceStorage;

#[async_trait]
impl SourceStorage for DisabledSourceStorage {
    async fn store_bundle(
        &self,
        _request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError> {
        Err(SourceStorageError::MissingConfig("source_repository.path"))
    }

    async fn list_bundles(
        &self,
        _code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, SourceStorageError> {
        Err(SourceStorageError::MissingConfig("source_repository.path"))
    }

    async fn list_code_hashes(&self) -> Result<Vec<String>, SourceStorageError> {
        Err(SourceStorageError::MissingConfig("source_repository.path"))
    }

    async fn current_revision(&self) -> Result<Option<String>, SourceStorageError> {
        Err(SourceStorageError::MissingConfig("source_repository.path"))
    }
}

#[derive(Clone)]
pub struct GitSourceStorage {
    repo_path: Option<PathBuf>,
    remote: String,
    branch: Option<String>,
    author_name: String,
    author_email: String,
    lock: Arc<Mutex<()>>,
}

impl GitSourceStorage {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            repo_path: config.source_repository_path().map(ToOwned::to_owned),
            remote: config.source_repository_remote().to_owned(),
            branch: config.source_repository_branch().map(ToOwned::to_owned),
            author_name: config.source_repository_author_name().to_owned(),
            author_email: config.source_repository_author_email().to_owned(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn store_bundle_locked(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError> {
        let repo_path = self
            .repo_path
            .as_deref()
            .ok_or(SourceStorageError::MissingConfig("source_repository.path"))?;
        ensure_git_repo(repo_path).await?;

        let bundle_path = bundle_relative_path(&request.code_hash, &request.source_bundle_hash);
        let bundle_dir = repo_path.join(&bundle_path);
        let files_dir = bundle_dir.join("files");
        fs::create_dir_all(&files_dir)
            .await
            .map_err(|source| SourceStorageError::CreateDir {
                path: files_dir.clone(),
                source,
            })?;

        write_bundle_files(&files_dir, &request.files).await?;
        write_manifest(&bundle_dir, &request).await?;

        git(repo_path, &["add", "--", &bundle_path]).await?;

        let staged = git_has_staged_changes(repo_path, &bundle_path).await?;
        if staged {
            let message = commit_message(&request);
            git_with_author(
                repo_path,
                &["commit", "-m", &message, "--", &bundle_path],
                self,
            )
            .await?;
        }
        let revision = git_output(repo_path, &["rev-parse", "HEAD"]).await?;

        let branch = match &self.branch {
            Some(branch) => branch.clone(),
            None => current_branch(repo_path).await?,
        };
        let refspec = format!("HEAD:{branch}");
        git(repo_path, &["push", &self.remote, &refspec]).await?;

        Ok(SourceStorageReceipt { revision })
    }

    async fn list_bundles_locked(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, SourceStorageError> {
        let repo_path = self
            .repo_path
            .as_deref()
            .ok_or(SourceStorageError::MissingConfig("source_repository.path"))?;
        ensure_git_repo(repo_path).await?;

        let code_hash_dir = repo_path.join(STORAGE_ROOT).join(code_hash);
        if !fs::try_exists(&code_hash_dir)
            .await
            .map_err(|source| SourceStorageError::ReadDir {
                path: code_hash_dir.clone(),
                source,
            })?
        {
            return Ok(Vec::new());
        }

        let mut read_dir =
            fs::read_dir(&code_hash_dir)
                .await
                .map_err(|source| SourceStorageError::ReadDir {
                    path: code_hash_dir.clone(),
                    source,
                })?;
        let mut bundles = Vec::new();
        while let Some(entry) =
            read_dir
                .next_entry()
                .await
                .map_err(|source| SourceStorageError::ReadDir {
                    path: code_hash_dir.clone(),
                    source,
                })?
        {
            let file_type =
                entry
                    .file_type()
                    .await
                    .map_err(|source| SourceStorageError::ReadDir {
                        path: entry.path(),
                        source,
                    })?;
            if !file_type.is_dir() {
                continue;
            }

            let source_bundle_hash = entry.file_name().to_string_lossy().into_owned();
            let bundle_path = bundle_relative_path(code_hash, &source_bundle_hash);
            bundles.push(read_bundle(repo_path, &bundle_path).await?);
        }

        bundles.sort_by(|left, right| {
            left.manifest
                .source_bundle_hash
                .cmp(&right.manifest.source_bundle_hash)
        });
        Ok(bundles)
    }

    async fn list_code_hashes_locked(&self) -> Result<Vec<String>, SourceStorageError> {
        let repo_path = self
            .repo_path
            .as_deref()
            .ok_or(SourceStorageError::MissingConfig("source_repository.path"))?;
        ensure_git_repo(repo_path).await?;

        let storage_dir = repo_path.join(STORAGE_ROOT);
        if !fs::try_exists(&storage_dir)
            .await
            .map_err(|source| SourceStorageError::ReadDir {
                path: storage_dir.clone(),
                source,
            })?
        {
            return Ok(Vec::new());
        }

        let mut read_dir =
            fs::read_dir(&storage_dir)
                .await
                .map_err(|source| SourceStorageError::ReadDir {
                    path: storage_dir.clone(),
                    source,
                })?;
        let mut code_hashes = Vec::new();
        while let Some(entry) =
            read_dir
                .next_entry()
                .await
                .map_err(|source| SourceStorageError::ReadDir {
                    path: storage_dir.clone(),
                    source,
                })?
        {
            let file_type =
                entry
                    .file_type()
                    .await
                    .map_err(|source| SourceStorageError::ReadDir {
                        path: entry.path(),
                        source,
                    })?;
            if file_type.is_dir() {
                code_hashes.push(entry.file_name().to_string_lossy().into_owned());
            }
        }

        code_hashes.sort();
        Ok(code_hashes)
    }

    async fn current_revision_locked(&self) -> Result<Option<String>, SourceStorageError> {
        let repo_path = self
            .repo_path
            .as_deref()
            .ok_or(SourceStorageError::MissingConfig("source_repository.path"))?;
        ensure_git_repo(repo_path).await?;

        match git_output(repo_path, &["rev-parse", "--verify", "HEAD"]).await {
            Ok(revision) => Ok(Some(revision)),
            Err(SourceStorageError::Git { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[async_trait]
impl SourceStorage for GitSourceStorage {
    async fn store_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError> {
        let _guard = self.lock.lock().await;
        self.store_bundle_locked(request).await
    }

    async fn list_bundles(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, SourceStorageError> {
        let _guard = self.lock.lock().await;
        self.list_bundles_locked(code_hash).await
    }

    async fn list_code_hashes(&self) -> Result<Vec<String>, SourceStorageError> {
        let _guard = self.lock.lock().await;
        self.list_code_hashes_locked().await
    }

    async fn current_revision(&self) -> Result<Option<String>, SourceStorageError> {
        let _guard = self.lock.lock().await;
        self.current_revision_locked().await
    }
}

#[derive(Debug, Error)]
pub enum SourceStorageError {
    #[error("missing source storage configuration: {0}")]
    MissingConfig(&'static str),
    #[error("invalid source storage path {path}: {message}", path = path.display())]
    InvalidPath { path: PathBuf, message: String },
    #[error("failed to create directory {path}: {source}", path = path.display())]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write file {path}: {source}", path = path.display())]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read directory {path}: {source}", path = path.display())]
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read file {path}: {source}", path = path.display())]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("source file is not valid UTF-8: {path}: {source}", path = path.display())]
    ReadFileUtf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    #[error("failed to serialize source manifest: {0}")]
    SerializeManifest(serde_json::Error),
    #[error("failed to deserialize source manifest {path}: {source}", path = path.display())]
    DeserializeManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("stored source file hash mismatch for {path}: expected={expected}, actual={actual}", path = path.display())]
    FileHashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("git command failed: {command}: status={status}, stderr={stderr}")]
    Git {
        command: String,
        status: String,
        stderr: String,
    },
    #[error("failed to spawn git command {command}: {source}")]
    GitSpawn {
        command: String,
        source: std::io::Error,
    },
    #[error("git output was not valid UTF-8 for {command}: {source}")]
    GitOutputUtf8 {
        command: String,
        source: std::string::FromUtf8Error,
    },
    #[error("git repository has detached HEAD; configure source_repository.branch")]
    DetachedHead,
    #[error("{0}")]
    Operation(String),
}

async fn ensure_git_repo(repo_path: &Path) -> Result<(), SourceStorageError> {
    if !repo_path.exists() {
        return Err(SourceStorageError::InvalidPath {
            path: repo_path.to_path_buf(),
            message: "path does not exist".to_owned(),
        });
    }

    let output = git_output(repo_path, &["rev-parse", "--is-inside-work-tree"]).await?;
    if output == "true" {
        return Ok(());
    }

    Err(SourceStorageError::InvalidPath {
        path: repo_path.to_path_buf(),
        message: "path is not inside a git work tree".to_owned(),
    })
}

fn bundle_relative_path(code_hash: &str, source_bundle_hash: &str) -> String {
    format!("{STORAGE_ROOT}/{code_hash}/{source_bundle_hash}")
}

async fn write_bundle_files(
    files_dir: &Path,
    files: &[SourceStorageFile],
) -> Result<(), SourceStorageError> {
    for file in files {
        let path = checked_join(files_dir, &file.path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| SourceStorageError::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
        }
        fs::write(&path, file.content.as_bytes())
            .await
            .map_err(|source| SourceStorageError::WriteFile { path, source })?;
    }

    Ok(())
}

async fn write_manifest(
    bundle_dir: &Path,
    request: &StoreSourceBundleRequest,
) -> Result<(), SourceStorageError> {
    let verified_at = match read_existing_verified_at(bundle_dir).await? {
        Some(verified_at) => verified_at,
        None => current_unix_timestamp()?,
    };

    let mut files = request
        .files
        .iter()
        .map(|file| DiskManifestFile {
            path: file.path.clone(),
            content_hash: hex::encode(Sha256::digest(file.content.as_bytes())),
            include_in_command: file.include_in_command,
            is_stdlib: file.is_stdlib,
            has_include_directives: file.has_include_directives,
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = DiskSourceBundleManifest {
        code_hash: request.code_hash.clone(),
        source_bundle_hash: request.source_bundle_hash.clone(),
        verified_at,
        compiler: request.compiler.clone(),
        files,
    };
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(SourceStorageError::SerializeManifest)?;
    let path = bundle_dir.join("manifest.json");
    fs::write(&path, bytes)
        .await
        .map_err(|source| SourceStorageError::WriteFile { path, source })
}

async fn read_existing_verified_at(bundle_dir: &Path) -> Result<Option<u64>, SourceStorageError> {
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_bytes = match fs::read(&manifest_path).await {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SourceStorageError::ReadFile {
                path: manifest_path,
                source,
            });
        }
    };

    let manifest =
        serde_json::from_slice::<DiskSourceBundleManifest>(&manifest_bytes).map_err(|source| {
            SourceStorageError::DeserializeManifest {
                path: manifest_path,
                source,
            }
        })?;

    Ok(Some(manifest.verified_at))
}

fn current_unix_timestamp() -> Result<u64, SourceStorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| SourceStorageError::Operation(source.to_string()))
        .map(|duration| duration.as_secs())
}

async fn read_bundle(
    repo_path: &Path,
    bundle_path: &str,
) -> Result<StoredSourceBundle, SourceStorageError> {
    let bundle_dir = repo_path.join(bundle_path);
    let manifest_path = bundle_dir.join("manifest.json");
    let manifest_bytes =
        fs::read(&manifest_path)
            .await
            .map_err(|source| SourceStorageError::ReadFile {
                path: manifest_path.clone(),
                source,
            })?;
    let disk_manifest = serde_json::from_slice::<DiskSourceBundleManifest>(&manifest_bytes)
        .map_err(|source| SourceStorageError::DeserializeManifest {
            path: manifest_path,
            source,
        })?;
    let files_dir = bundle_dir.join("files");
    let files = read_bundle_files(&files_dir, &disk_manifest.files).await?;
    let storage_revision = git_output(
        repo_path,
        &["log", "-n", "1", "--format=%H", "--", bundle_path],
    )
    .await?;
    if storage_revision.is_empty() {
        return Err(SourceStorageError::Operation(format!(
            "stored bundle has no storage revision: {bundle_path}"
        )));
    }

    Ok(StoredSourceBundle {
        storage_revision,
        manifest: SourceBundleManifest {
            code_hash: disk_manifest.code_hash,
            source_bundle_hash: disk_manifest.source_bundle_hash,
            verified_at: disk_manifest.verified_at,
            compiler: disk_manifest.compiler,
        },
        files,
    })
}

async fn read_bundle_files(
    files_dir: &Path,
    manifest_files: &[DiskManifestFile],
) -> Result<Vec<StoredSourceFile>, SourceStorageError> {
    let mut files = Vec::with_capacity(manifest_files.len());
    for manifest_file in manifest_files {
        let path = checked_join(files_dir, &manifest_file.path)?;
        let content = fs::read(&path)
            .await
            .map_err(|source| SourceStorageError::ReadFile {
                path: path.clone(),
                source,
            })?;
        let actual_content_hash = hex::encode(Sha256::digest(&content));
        if actual_content_hash != manifest_file.content_hash {
            return Err(SourceStorageError::FileHashMismatch {
                path,
                expected: manifest_file.content_hash.clone(),
                actual: actual_content_hash,
            });
        }

        let content =
            String::from_utf8(content).map_err(|source| SourceStorageError::ReadFileUtf8 {
                path: path.clone(),
                source,
            })?;
        files.push(StoredSourceFile {
            path: manifest_file.path.clone(),
            content_hash: manifest_file.content_hash.clone(),
            content,
            include_in_command: manifest_file.include_in_command,
            is_stdlib: manifest_file.is_stdlib,
            has_include_directives: manifest_file.has_include_directives,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn checked_join(root: &Path, relative: &str) -> Result<PathBuf, SourceStorageError> {
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

async fn git(repo_path: &Path, args: &[&str]) -> Result<(), SourceStorageError> {
    let output = git_command(repo_path, args).await?;
    if output.status.success() {
        return Ok(());
    }

    Err(git_error(args, &output))
}

async fn git_output(repo_path: &Path, args: &[&str]) -> Result<String, SourceStorageError> {
    let output = git_command(repo_path, args).await?;
    if !output.status.success() {
        return Err(git_error(args, &output));
    }

    String::from_utf8(output.stdout)
        .map(|output| output.trim().to_owned())
        .map_err(|source| SourceStorageError::GitOutputUtf8 {
            command: git_command_string(args),
            source,
        })
}

async fn git_has_staged_changes(
    repo_path: &Path,
    bundle_path: &str,
) -> Result<bool, SourceStorageError> {
    let output = git_command(
        repo_path,
        &["diff", "--cached", "--quiet", "--", bundle_path],
    )
    .await?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(git_error(
            &["diff", "--cached", "--quiet", "--", bundle_path],
            &output,
        )),
    }
}

async fn git_with_author(
    repo_path: &Path,
    args: &[&str],
    storage: &GitSourceStorage,
) -> Result<(), SourceStorageError> {
    let command = git_command_string(args);
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .env("GIT_AUTHOR_NAME", &storage.author_name)
        .env("GIT_AUTHOR_EMAIL", &storage.author_email)
        .env("GIT_COMMITTER_NAME", &storage.author_name)
        .env("GIT_COMMITTER_EMAIL", &storage.author_email)
        .output()
        .await
        .map_err(|source| SourceStorageError::GitSpawn {
            command: command.clone(),
            source,
        })?;

    if output.status.success() {
        return Ok(());
    }

    Err(git_error(args, &output))
}

async fn git_command(
    repo_path: &Path,
    args: &[&str],
) -> Result<std::process::Output, SourceStorageError> {
    let command = git_command_string(args);
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .map_err(|source| SourceStorageError::GitSpawn { command, source })
}

async fn current_branch(repo_path: &Path) -> Result<String, SourceStorageError> {
    let branch = git_output(repo_path, &["branch", "--show-current"]).await?;
    if branch.is_empty() {
        return Err(SourceStorageError::DetachedHead);
    }

    Ok(branch)
}

fn commit_message(request: &StoreSourceBundleRequest) -> String {
    format!(
        "Verify source bundle {}\n\ncode_hash: {}\nsource_bundle_hash: {}",
        request.source_bundle_hash, request.code_hash, request.source_bundle_hash
    )
}

fn git_error(args: &[&str], output: &std::process::Output) -> SourceStorageError {
    SourceStorageError::Git {
        command: git_command_string(args),
        status: output.status.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
}

fn git_command_string(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StoredSourceBundle {
    pub storage_revision: String,
    pub manifest: SourceBundleManifest,
    pub files: Vec<StoredSourceFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StoredSourceFile {
    pub path: String,
    pub content_hash: String,
    pub content: String,
    pub include_in_command: Option<bool>,
    pub is_stdlib: Option<bool>,
    pub has_include_directives: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceBundleManifest {
    pub code_hash: String,
    pub source_bundle_hash: String,
    pub verified_at: u64,
    pub compiler: CompilerMetadata,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DiskSourceBundleManifest {
    code_hash: String,
    source_bundle_hash: String,
    verified_at: u64,
    compiler: CompilerMetadata,
    files: Vec<DiskManifestFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DiskManifestFile {
    pub path: String,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_in_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_stdlib: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_include_directives: Option<bool>,
}
