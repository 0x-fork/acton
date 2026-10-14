use std::{collections::BTreeSet, path::PathBuf};

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
            bundles.push(BundleRemoval {
                code_hash,
                path,
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
        self.bundles
            .iter()
            .filter(|bundle| !bundle.files.is_empty())
            .count()
    }

    async fn execute(&self) -> Result<(), CliError> {
        let files = self
            .bundles
            .iter()
            .flat_map(|bundle| bundle.files.iter().map(String::as_str))
            .collect::<Vec<_>>();
        if files.is_empty() {
            return Ok(());
        }

        let git = Git::new(&self.repository.path);
        let result: Result<(), SourceStorageError> = async {
            git.remove_files(&files).await?;
            git.ensure_staged_deletions(&self.checked.local_head, &files)
                .await
        }
        .await;

        match result {
            Ok(()) => Ok(()),
            Err(operation) => match git.restore_files(&self.checked.local_head, &files).await {
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

pub async fn run(args: &RemoveRegistryArgs) -> Result<(), CliError> {
    let plan = RemovalPlan::prepare(RemoveRegistryRequest::prepare(args)?).await?;
    if !args.dry_run {
        plan.execute().await?;
    }

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
        let status = if bundle.files.is_empty() {
            "not found"
        } else if args.dry_run {
            "found"
        } else {
            "removed"
        };
        println!("  {}: {status}", bundle.code_hash);
        println!("    Path: {}", bundle.path);
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
        } else {
            println!("Deletions staged in Git.");
        }
    }

    Ok(())
}
