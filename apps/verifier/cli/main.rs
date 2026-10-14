mod args;

use args::{Cli, CliCommand};
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        CliCommand::RemoveRegistry(args) => {
            println!("CLI scaffold: arguments parsed only; remove-registry is not implemented.");
            println!("Config: {}", args.config.display());
            println!("Dry run: {}", args.dry_run);
            if let Some(reason) = args.reason {
                println!("Reason: {reason}");
            }
            println!("Code hashes: {}", args.code_hashes.join(", "));
        }
    }
}
