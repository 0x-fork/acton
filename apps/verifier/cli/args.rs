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
    /// Remove selected contracts from the registry (not implemented yet)
    RemoveRegistry(RemoveRegistryArgs),
}

#[derive(Args)]
pub struct RemoveRegistryArgs {
    /// Contract code hashes in hex or base64 (not validated yet)
    #[arg(value_name = "CODE_HASH", required = true, num_args = 1..)]
    pub code_hashes: Vec<String>,

    /// Path to the verifier configuration file (not read yet)
    #[arg(
        long,
        value_name = "PATH",
        env = "VERIFIER_CONFIG",
        default_value = "config.toml"
    )]
    pub config: PathBuf,

    /// Request an eventual removal plan without applying changes
    #[arg(long)]
    pub dry_run: bool,

    /// Optional reason to include in the eventual commit message
    #[arg(long, value_name = "TEXT")]
    pub reason: Option<String>,
}
