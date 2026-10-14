use std::{collections::BTreeSet, path::PathBuf};

use verifier::{
    blockchain::parse_code_hash,
    config::Config,
    source_storage::{git::Git, repository},
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
}

pub async fn run(args: &RemoveRegistryArgs) -> Result<(), CliError> {
    let plan = RemovalPlan::prepare(RemoveRegistryRequest::prepare(args)?).await?;
    if !args.dry_run {
        return Err(CliError::RemovalNotImplemented);
    }

    println!("Dry run: registry removal plan");
    println!("Config: {}", args.config.display());
    println!("Repository: {}", plan.repository.path.display());
    println!("Storage root: {}", plan.repository.storage_root);
    println!("Remote: {}", plan.repository.remote);
    println!("Branch: {}", plan.checked.branch);
    println!("Local HEAD: {}", plan.checked.local_head);
    println!("Remote HEAD: {}", plan.checked.remote_head);
    println!(
        "Author: {} <{}>",
        plan.repository.author_name, plan.repository.author_email
    );
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
        } else {
            "remove"
        };
        println!("  {}: {status}", bundle.code_hash);
        println!("    Path: {}", bundle.path);
        for file in &bundle.files {
            println!("    Delete: {file}");
        }
    }
    let to_remove = plan
        .bundles
        .iter()
        .filter(|bundle| !bundle.files.is_empty())
        .count();
    let files = plan
        .bundles
        .iter()
        .map(|bundle| bundle.files.len())
        .sum::<usize>();
    println!("Bundles to remove: {to_remove}");
    println!("Not found: {}", plan.bundles.len() - to_remove);
    println!("Files to remove: {files}");

    Ok(())
}
