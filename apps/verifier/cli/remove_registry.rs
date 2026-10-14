use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use tokio::fs;
use verifier::{
    blockchain::parse_code_hash,
    config::Config,
    source_storage::{SourceStorageError, git::Git, repository},
};

use crate::{args::RemoveRegistryArgs, check_repository, error::CliError};

struct SourceRepositorySettings {
    path: PathBuf,
    storage_root: String,
    remote: String,
    branch: Option<String>,
    author_name: String,
    author_email: String,
}

struct RemoveRegistryRequest {
    repository: SourceRepositorySettings,
    code_hashes: Vec<String>,
}

struct RemovalPlan {
    repository: SourceRepositorySettings,
    checked: check_repository::CheckedRepository,
    bundles: Vec<BundleRemoval>,
}

struct BundleRemoval {
    code_hash: String,
    path: String,
    found: bool,
    files: Vec<String>,
}

impl RemoveRegistryRequest {
    fn prepare(args: &RemoveRegistryArgs) -> Result<Self, CliError> {
        let config = Config::load_from_path(&args.config)?;
        let path = config
            .source_repository_path()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| CliError::MissingSourceRepository {
                config_path: args.config.clone(),
            })?;

        let code_hashes = args
            .code_hashes
            .iter()
            .map(|value| {
                parse_code_hash(value).ok_or_else(|| CliError::InvalidCodeHash {
                    value: value.clone(),
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .collect();

        Ok(Self {
            repository: SourceRepositorySettings {
                path: path.to_path_buf(),
                storage_root: config.source_repository_storage_root().to_owned(),
                remote: config.source_repository_remote().to_owned(),
                branch: config.source_repository_branch().map(ToOwned::to_owned),
                author_name: config.source_repository_author_name().to_owned(),
                author_email: config.source_repository_author_email().to_owned(),
            },
            code_hashes,
        })
    }
}

impl RemovalPlan {
    async fn prepare(request: RemoveRegistryRequest) -> Result<Self, CliError> {
        let checked = check_repository::check(
            &request.repository.path,
            &request.repository.storage_root,
            &request.repository.remote,
            request.repository.branch.as_deref(),
        )
        .await?;

        let git = Git::new(&request.repository.path);
        let mut bundles = Vec::with_capacity(request.code_hashes.len());
        for code_hash in request.code_hashes {
            let path =
                repository::bundle_relative_path(&request.repository.storage_root, &code_hash)?;
            let files = git.tracked_files(&path).await?;
            let directory = request.repository.path.join(&path);
            let found =
                fs::try_exists(&directory)
                    .await
                    .map_err(|source| SourceStorageError::ReadDir {
                        path: directory,
                        source,
                    })?;

            bundles.push(BundleRemoval {
                code_hash,
                path,
                found,
                files,
            });
        }

        Ok(Self {
            repository: request.repository,
            checked,
            bundles,
        })
    }

    fn found_count(&self) -> usize {
        self.bundles.iter().filter(|bundle| bundle.found).count()
    }

    async fn execute(&self) -> Result<BTreeMap<String, String>, CliError> {
        let mut commits = BTreeMap::new();
        let mut head = self.checked.local_head.clone();
        for bundle in self.bundles.iter().filter(|bundle| bundle.found) {
            match self.remove_bundle(bundle, &head).await {
                Ok(Some(commit)) => {
                    head.clone_from(&commit);
                    commits.insert(bundle.code_hash.clone(), commit);
                }
                Ok(None) => {}
                Err(source) if commits.is_empty() => return Err(source),
                Err(source) => {
                    return Err(CliError::PartialRemoval {
                        code_hash: bundle.code_hash.clone(),
                        head,
                        source: Box::new(source),
                    });
                }
            }
        }
        if !commits.is_empty() {
            Git::new(&self.repository.path)
                .push_head(&self.repository.remote, Some(&self.checked.branch))
                .await
                .map_err(|source| CliError::PushFailed {
                    remote: self.repository.remote.clone(),
                    branch: self.checked.branch.clone(),
                    head,
                    source: Box::new(source),
                })?;
        }
        Ok(commits)
    }

    async fn remove_bundle(
        &self,
        bundle: &BundleRemoval,
        parent: &str,
    ) -> Result<Option<String>, CliError> {
        let git = Git::new(&self.repository.path);
        let files = bundle.files.iter().map(String::as_str).collect::<Vec<_>>();
        let message = bundle.commit_message(&self.repository.path).await;
        let result: Result<Option<String>, SourceStorageError> = async {
            let path = self.repository.path.join(&bundle.path);
            if let Err(source) = fs::remove_dir_all(&path).await
                && source.kind() != std::io::ErrorKind::NotFound
            {
                return Err(SourceStorageError::RemoveDir { path, source });
            }
            if !files.is_empty() {
                git.remove_files(&files).await?;
            }
            git.ensure_staged_deletions(parent, &files).await?;
            if files.is_empty() {
                return Ok(None);
            }
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|source| SourceStorageError::Operation(source.to_string()))?
                .as_secs();
            git.run_with_author(
                &["commit", "--cleanup=verbatim", "-m", &message],
                &self.repository.author_name,
                &self.repository.author_email,
                timestamp,
            )
            .await?;
            git.head().await.map(Some)
        }
        .await;

        let operation = match result {
            Ok(commit) => return Ok(commit),
            Err(operation) => operation,
        };
        match git.head().await {
            Err(check) => Err(CliError::HeadCheckFailed {
                operation: Box::new(operation),
                check: Box::new(check),
            }),
            Ok(head) if head != parent => Err(CliError::LocalCommitPreserved {
                source: Box::new(operation),
                head,
            }),
            Ok(_) if files.is_empty() => Err(operation.into()),
            Ok(_) => match git.restore_files(parent, &files).await {
                Ok(()) => Err(CliError::RemovalFailed {
                    source: Box::new(operation),
                }),
                Err(recovery) => Err(CliError::RecoveryFailed {
                    operation: Box::new(operation),
                    recovery: Box::new(recovery),
                }),
            },
        }
    }
}

impl BundleRemoval {
    async fn commit_message(&self, repository: &Path) -> String {
        let manifest = fs::read(repository.join(&self.path).join("manifest.json"))
            .await
            .ok();
        let manifest_hash = manifest.as_ref().map_or_else(
            || "unavailable".to_owned(),
            |bytes| hex::encode(Sha256::digest(bytes)),
        );
        let source_bundle_hash = manifest
            .as_deref()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(bytes).ok())
            .and_then(|manifest| {
                manifest
                    .get("source_bundle_hash")?
                    .as_str()
                    .map(ToOwned::to_owned)
            })
            .filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .unwrap_or_else(|| "unavailable".to_owned());
        format!(
            "Revoke verification for code hash {}\n\ncode_hash: {}\nsource_bundle_hash: {source_bundle_hash}\nmanifest_hash: {manifest_hash}",
            self.code_hash, self.code_hash
        )
    }
}

pub async fn run(args: &RemoveRegistryArgs) -> Result<(), CliError> {
    let plan = RemovalPlan::prepare(RemoveRegistryRequest::prepare(args)?).await?;
    let commits = if args.dry_run {
        BTreeMap::new()
    } else {
        plan.execute().await?
    };

    if args.dry_run {
        println!("Dry run: registry removal plan");
    } else {
        println!("Bundles removed from checkout");
    }
    println!("Config: {}", args.config.display());
    println!("Repository: {}", plan.repository.path.display());
    println!("Storage root: {}", plan.repository.storage_root);
    println!("Remote: {}", plan.repository.remote);
    println!("Branch: {}", plan.checked.branch);
    println!("Local HEAD: {}", plan.checked.local_head);
    println!("Remote HEAD: {}", plan.checked.remote_head);
    let code_hashes = plan
        .bundles
        .iter()
        .map(|bundle| bundle.code_hash.as_str())
        .collect::<Vec<_>>();
    println!("Code hashes: {}", code_hashes.join(", "));
    println!("Bundles:");
    for bundle in &plan.bundles {
        let status = if !bundle.found {
            "not found"
        } else if args.dry_run {
            "found"
        } else {
            "removed"
        };
        println!("  {}: {status}", bundle.code_hash);
        println!("    Path: {}", bundle.path);
        if let Some(commit) = commits.get(&bundle.code_hash) {
            println!("    Commit: {commit}");
        }
    }
    let found = plan.found_count();
    if args.dry_run {
        println!("Bundles to remove: {found}");
    } else {
        println!("Removed bundles: {found}");
    }
    println!("Not found: {}", plan.bundles.len() - found);
    if !args.dry_run {
        if found == 0 {
            println!("No bundles found; nothing to remove.");
        } else if !commits.is_empty() {
            println!("Push: {}/{}", plan.repository.remote, plan.checked.branch);
        }
    }

    Ok(())
}
