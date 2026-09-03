use std::{env, process::Command};

use chrono::Utc;

fn main() {
    println!("cargo:rerun-if-env-changed=LOCALTON_GIT_HASH");

    let package_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION must be set");
    let git_hash = git_hash();
    let build_date = Utc::now().format("%Y-%m-%d");

    println!("cargo:rustc-env=LOCALTON_LONG_VERSION={package_version} ({git_hash} {build_date})");
}

fn git_hash() -> String {
    if let Ok(hash) = env::var("LOCALTON_GIT_HASH")
        && !hash.trim().is_empty()
    {
        return hash.trim().chars().take(9).collect();
    }

    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_owned())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
