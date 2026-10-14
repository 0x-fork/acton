mod args;
mod error;
mod remove_registry;

use std::process::ExitCode;

use args::{Cli, CliCommand};
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        CliCommand::RemoveRegistry(args) => remove_registry::run(&args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("verifier-cli: {error}");
            ExitCode::FAILURE
        }
    }
}
