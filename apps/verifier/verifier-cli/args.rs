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
    /// Remove selected registry bundles, commit each deletion and push when enabled
    ///
    /// Requires `server.read_only = true` and confirmation that the backend is stopped.
    /// Prompts again after inspection to confirm the number of bundles to remove.
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

    /// Show the removal plan without changes; check remote only when push is enabled
    #[arg(long)]
    pub dry_run: bool,
}
