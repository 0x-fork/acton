use std::{collections::BTreeSet, path::PathBuf};

use verifier::{blockchain::parse_code_hash, config::Config};

use crate::{args::RemoveRegistryArgs, error::CliError};

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
    reason: Option<String>,
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
            reason: args.reason.clone(),
        })
    }
}

pub fn run(args: &RemoveRegistryArgs) -> Result<(), CliError> {
    let request = RemoveRegistryRequest::prepare(args)?;
    if !args.dry_run {
        return Err(CliError::RemovalNotImplemented);
    }

    println!(
        "Request prepared only; repository checks and removal planning are not implemented yet."
    );
    println!("Config: {}", args.config.display());
    println!("Repository: {}", request.repository.path.display());
    println!("Storage root: {}", request.repository.storage_root);
    println!("Remote: {}", request.repository.remote);
    println!(
        "Branch: {}",
        request
            .repository
            .branch
            .as_deref()
            .unwrap_or("current branch (not resolved yet)")
    );
    println!(
        "Author: {} <{}>",
        request.repository.author_name, request.repository.author_email
    );
    if let Some(reason) = request.reason {
        println!("Reason: {reason}");
    }
    println!("Code hashes: {}", request.code_hashes.join(", "));

    Ok(())
}
