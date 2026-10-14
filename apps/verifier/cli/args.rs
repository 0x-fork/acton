use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "verifier-cli", version, about = "Verifier maintenance CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand)]
pub enum CliCommand {
    /// Prepare removal of selected registry contracts (execution is not implemented yet)
    RemoveRegistry(RemoveRegistryArgs),
}

#[derive(Args)]
pub struct RemoveRegistryArgs {
    /// Contract code hashes in hex or base64
    #[arg(value_name = "CODE_HASH", required = true, num_args = 1..)]
    pub code_hashes: Vec<String>,

    /// Path to the verifier configuration file
    #[arg(
        long,
        value_name = "PATH",
        env = "VERIFIER_CONFIG",
        default_value = "config.toml"
    )]
    pub config: PathBuf,

    /// Prepare the request only (repository checks and removal planning are not implemented yet)
    #[arg(long)]
    pub dry_run: bool,

    /// Optional reason to include in the commit message
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,
}
