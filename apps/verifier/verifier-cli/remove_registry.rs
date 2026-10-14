use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, IsTerminal, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use inquire::Text;
use sha2::{Digest, Sha256};
use tokio::fs;
use verifier::{
    blockchain::parse_code_hash,
    config::{Config, SourceRepositorySettings},
    source_storage::{SourceStorageError, git::Git, repository},
};

use crate::{args::RemoveRegistryArgs, check_repository, error::CliError};

struct RemoveRegistryRequest<'a> {
    repository: &'a SourceRepositorySettings,
    path: &'a Path,
    code_hashes: Vec<String>,
}

struct RemovalPlan<'a> {
    repository: &'a SourceRepositorySettings,
    path: &'a Path,
    checked: check_repository::CheckedRepository,
    bundles: Vec<BundleRemoval>,
}

struct BundleRemoval {
    code_hash: String,
    path: String,
    found: bool,
    files: Vec<String>,
}

impl<'a> RemoveRegistryRequest<'a> {
    fn prepare(args: &RemoveRegistryArgs, config: &'a Config) -> Result<Self, CliError> {
        let repository = config.source_repository();
        let path = repository
            .path
            .as_deref()
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

        if !args.dry_run && !config.read_only() {
            return Err(CliError::ReadOnlyRequired {
                config_path: args.config.clone(),
            });
        }

        Ok(Self {
            repository,
            path,
            code_hashes,
        })
    }
}

impl<'a> RemovalPlan<'a> {
    async fn prepare(request: RemoveRegistryRequest<'a>) -> Result<Self, CliError> {
        let checked = check_repository::check(
            request.path,
            &request.repository.storage_root,
            &request.repository.remote,
            request.repository.branch.as_deref(),
            request.repository.push_enabled,
        )
        .await?;

        eprintln!("Branch: {}", checked.branch);
        eprintln!("Local HEAD: {}", checked.local_head);
        if let Some(remote_head) = &checked.remote_head {
            eprintln!("Remote HEAD: {remote_head}");
        } else {
            eprintln!("Remote HEAD: not checked (push_enabled = false)");
        }

        let git = Git::new(request.path);
        let total = request.code_hashes.len();
        let mut bundles = Vec::with_capacity(total);
        for (index, code_hash) in request.code_hashes.into_iter().enumerate() {
            eprintln!("[check {}/{total}] Inspecting {code_hash}...", index + 1);
            let path =
                repository::bundle_relative_path(&request.repository.storage_root, &code_hash)?;
            let files = git.tracked_files(&path).await?;
            let directory = request.path.join(&path);
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
            path: request.path,
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
        let total = self.bundles.len();
        for (index, bundle) in self.bundles.iter().enumerate() {
            let current = index + 1;
            let code_hash = &bundle.code_hash;
            if !bundle.found {
                eprintln!("[remove {current}/{total}] {code_hash}: not found; skipping");
                continue;
            }
            eprintln!(
                "[remove {current}/{total}] Removing {code_hash} ({} tracked files)...",
                bundle.files.len()
            );
            match self.remove_bundle(bundle, &head).await {
                Ok(Some(commit)) => {
                    eprintln!("[remove {current}/{total}] Removed {code_hash}; commit {commit}");
                    head.clone_from(&commit);
                    commits.insert(bundle.code_hash.clone(), commit);
                }
                Ok(None) => {
                    eprintln!(
                        "[remove {current}/{total}] Removed {code_hash}; no tracked files to commit"
                    );
                }
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
        if self.repository.push_enabled && !commits.is_empty() {
            eprintln!(
                "Pushing {} commits to {}/{}...",
                commits.len(),
                self.repository.remote,
                self.checked.branch
            );
            Git::new(self.path)
                .push_head(&self.repository.remote, Some(&self.checked.branch))
                .await
                .map_err(|source| CliError::PushFailed {
                    remote: self.repository.remote.clone(),
                    branch: self.checked.branch.clone(),
                    head,
                    source: Box::new(source),
                })?;
            eprintln!("Push completed.");
        }
        Ok(commits)
    }

    async fn remove_bundle(
        &self,
        bundle: &BundleRemoval,
        parent: &str,
    ) -> Result<Option<String>, CliError> {
        let git = Git::new(self.path);
        let files = bundle.files.iter().map(String::as_str).collect::<Vec<_>>();
        let message = bundle.commit_message(self.path).await;
        let result: Result<Option<String>, SourceStorageError> = async {
            let path = self.path.join(&bundle.path);
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
    let config = Config::load_from_path(&args.config)?;
    let request = RemoveRegistryRequest::prepare(args, &config)?;
    eprintln!("Config: {}", args.config.display());
    eprintln!("Repository: {}", request.path.display());
    eprintln!("Storage root: {}", request.repository.storage_root);
    eprintln!("Remote: {}", request.repository.remote);
    eprintln!("Push enabled: {}", request.repository.push_enabled);
    eprintln!("Dry run: {}", args.dry_run);
    eprintln!("Unique code hashes: {}", request.code_hashes.len());
    if !request.repository.push_enabled {
        eprintln!(
            "Warning: push_enabled = false; commits will remain local and will not be pushed."
        );
    }
    if !args.dry_run {
        confirm(
            "Backend must be stopped. Type 'yes' to confirm:",
            CliError::BackendNotConfirmed,
        )?;
    }
    // Check the repository after confirmation, once the operator has stopped the backend.
    let plan = RemovalPlan::prepare(request).await?;
    let found = plan.found_count();
    eprintln!(
        "Removal plan: {found} found, {} not found",
        plan.bundles.len() - found
    );
    let commits = if args.dry_run {
        BTreeMap::new()
    } else {
        if found > 0 {
            confirm(
                &format!("Remove {found} bundles? Type 'yes' to confirm:"),
                CliError::RemovalNotConfirmed,
            )?;
        }
        plan.execute().await?
    };

    if args.dry_run {
        println!("Dry run: registry removal plan");
    } else {
        println!("Bundles removed from checkout");
    }
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
            if plan.repository.push_enabled {
                println!("Push: {}/{}", plan.repository.remote, plan.checked.branch);
            } else {
                println!("Push skipped: push_enabled = false; commits kept locally.");
            }
        }
    }

    Ok(())
}

fn confirm(prompt: &str, cancellation: CliError) -> Result<(), CliError> {
    let confirmation = if io::stdin().is_terminal() {
        Text::new(prompt).prompt()?
    } else {
        eprint!("{prompt} ");
        io::stderr().flush()?;
        let mut confirmation = String::new();
        io::stdin().read_line(&mut confirmation)?;
        confirmation
    };
    if confirmation.trim() != "yes" {
        return Err(cancellation);
    }
    Ok(())
}
