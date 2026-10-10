use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use semver::Version;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tar::Archive;

use super::client::{Asset, Release, ReleaseClient};

#[derive(serde::Serialize)]
pub(super) struct UpdateInfo {
    pub success: bool,
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

pub(super) fn check_update<C: ReleaseClient>(
    client: &C,
    current_version_str: &str,
    current_is_trunk: bool,
) -> Result<UpdateInfo> {
    let release = client.get_release(None, false)?;
    let latest_version = release.tag_name;

    let update_available = if current_is_trunk {
        // don't report anything for trunk release user
        false
    } else {
        let current_v = Version::parse(current_version_str).with_context(|| {
            format!("Current Acton version '{current_version_str}' is not valid semver")
        })?;
        let latest_v_str = latest_version.trim_start_matches('v');
        if let Ok(latest_v) = Version::parse(latest_v_str) {
            latest_v > current_v
        } else {
            false
        }
    };

    Ok(UpdateInfo {
        success: true,
        current_version: current_version_str.to_string(),
        latest_version,
        update_available,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_update<C: ReleaseClient>(
    client: &C,
    current_exe: &Path,
    current_version_str: &str,
    current_is_trunk: bool,
    version: Option<String>,
    trunk: bool,
    stable: bool,
    force: bool,
) -> Result<()> {
    let current_version = Version::parse(current_version_str);
    let use_trunk_release = version.is_none() && !stable && (trunk || current_is_trunk);

    let release = client.get_release(version.as_deref(), use_trunk_release)?;

    let should_install = if version.is_some() || use_trunk_release {
        // An explicit version always wins; otherwise stay on the active trunk channel.
        if version.is_none() && use_trunk_release {
            println!("  {} trunk release", "Installing".green().bold());
        } else {
            println!("  {} {}", "Installing".green().bold(), release.tag_name);
        }
        true
    } else if stable {
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if current_is_trunk {
                // when we are on a trunk build and user provide `--stable`, update to latest stable version
                println!(
                    "  {} stable version {} (current: trunk)",
                    "Installing".green().bold(),
                    target_version
                );
                true
            } else if force {
                print_forced_install_release(
                    &target_version,
                    current_version.as_ref().ok(),
                    current_version_str,
                    true,
                );
                true
            } else if let Ok(current_version) = &current_version
                && &target_version != current_version
            {
                // if we on stable release, install new stable version
                println!(
                    "  {} stable version {} (current: {})",
                    "Installing".green().bold(),
                    target_version,
                    current_version
                );
                true
            } else {
                println!(
                    "  {} Acton is already at the latest stable version ({})",
                    "Up to date".green().bold(),
                    current_version?
                );
                false
            }
        } else {
            println!(
                "    {} Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                "Skipping".yellow().bold(),
                release.tag_name
            );
            return Ok(());
        }
    } else {
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if force {
                print_forced_install_release(
                    &target_version,
                    current_version.as_ref().ok(),
                    current_version_str,
                    false,
                );
                true
            } else {
                let current_version = current_version.with_context(|| {
                    format!("Current Acton version '{current_version_str}' is not valid semver")
                })?;
                if target_version > current_version {
                    println!(
                        "    {} version {} (current: {})",
                        "Updating".green().bold(),
                        target_version,
                        current_version
                    );
                    true
                } else if target_version == current_version {
                    // If versions match, we're up to date
                    println!(
                        "  {} Acton is up to date (version {})",
                        "Up to date".green().bold(),
                        current_version
                    );
                    return Ok(());
                } else {
                    println!(
                        "  {} Acton is up to date (version {})",
                        "Up to date".green().bold(),
                        current_version
                    );
                    return Ok(());
                }
            }
        } else {
            println!(
                "   {} Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                "Skipping".yellow().bold(),
                release.tag_name
            );
            return Ok(());
        }
    };

    if !should_install {
        return Ok(());
    }

    let asset = find_asset(&release)?;
    let checksum_asset = find_checksum_asset(&release, &asset.name)?;
    let tarball_path = client.download_asset(asset)?;
    let checksum_path = client.download_asset(checksum_asset)?;

    let verify_result = verify_sha256(&tarball_path, &checksum_path, &asset.name);
    let _ = fs::remove_file(&checksum_path);
    verify_result?;

    install_binary(&tarball_path, current_exe, current_version_str)?;

    println!("     {} to {}", "Updated".green().bold(), release.tag_name);

    Ok(())
}

/// Restore the previously installed Acton binary from a local backup.
///
/// `install_binary` keeps a copy of every replaced binary next to the current
/// one (named `acton-<version>`), so the rollback works fully offline and
/// never talks to GitHub. It restores the most recently modified backup and
/// does not create or overwrite any backups, which keeps repeated calls
/// idempotent: once the restored version is the current one, further
/// rollbacks are no-ops.
pub(super) fn run_rollback(current_exe: &Path, current_version: &str) -> Result<()> {
    let backup_path = latest_backup_path(current_exe)?;

    if backup_path
        .as_ref()
        .is_some_and(|path| backup_version(path) == current_version)
    {
        println!(
            "  {} Acton is already at version {} (no rollback needed)",
            "Up to date".green().bold(),
            current_version
        );
        return Ok(());
    }

    let backup_path = backup_path.ok_or_else(|| {
        anyhow::anyhow!(
            "No previous Acton version was found next to the current binary — backups are created by `acton up` when it replaces the binary"
        )
    })?;

    let backup_version = backup_version(&backup_path);

    println!(
        "  {} to {} (current: {})",
        "Rolling back".green().bold(),
        backup_version,
        current_version
    );
    println!(
        "       {} {}",
        "Found".green().bold(),
        backup_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );

    swap_binary(&backup_path, current_exe, "restored Acton binary")?;

    println!(
        "     {} to {}",
        "Rolled back".green().bold(),
        backup_version
    );

    Ok(())
}

/// Find the most recently modified `acton-<version>` backup in the directory
/// of the current binary. Mtime is the only available recency signal; file
/// name breaks ties deterministically.
fn latest_backup_path(current_exe: &Path) -> Result<Option<PathBuf>> {
    let bin_dir = current_exe.parent().ok_or_else(|| {
        anyhow::anyhow!("Could not determine the directory of the current Acton binary")
    })?;

    let mut candidates: Vec<(Option<SystemTime>, String, PathBuf)> = Vec::new();

    for entry in fs::read_dir(bin_dir).with_context(|| {
        format!(
            "Failed to read the directory of the current Acton binary at {}",
            bin_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(version) = name.strip_prefix("acton-") else {
            continue;
        };
        if version.is_empty() {
            continue;
        }

        let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok();
        candidates.push((modified, name.to_owned(), path));
    }

    candidates.sort_by(|(mtime_a, name_a, _), (mtime_b, name_b, _)| {
        mtime_a
            .partial_cmp(mtime_b)
            .unwrap_or(Ordering::Equal)
            .then_with(|| name_a.cmp(name_b))
    });

    Ok(candidates.into_iter().next_back().map(|(_, _, path)| path))
}

/// Extract the version from an `acton-<version>` backup file name.
fn backup_version(backup_path: &Path) -> String {
    backup_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.strip_prefix("acton-").unwrap_or(name).to_owned())
        .unwrap_or_default()
}

fn print_forced_install_release(
    target_version: &Version,
    current_version: Option<&Version>,
    current_version_str: &str,
    stable: bool,
) {
    let action = if current_version.is_some_and(|current| current == target_version) {
        "Reinstalling".green().bold()
    } else {
        "Installing".green().bold()
    };
    let current_display =
        current_version.map_or_else(|| current_version_str.to_owned(), ToString::to_string);
    let subject = if stable { "stable version" } else { "version" };

    println!("  {action} {subject} {target_version} (current: {current_display})");
}

fn find_asset(release: &Release) -> Result<&Asset> {
    find_asset_for_target_triple(release, env!("TARGET_TRIPLE"))
}

pub(super) fn find_asset_for_target_triple<'a>(
    release: &'a Release,
    target_triple: &str,
) -> Result<&'a Asset> {
    let expected_name = release_asset_name_for_target_triple(target_triple)?;

    release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(&expected_name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Release {} does not include an archive for target {}. Expected asset: {}",
                release.tag_name,
                target_triple,
                expected_name
            )
        })
}

pub(super) fn release_asset_name_for_target_triple(target_triple: &str) -> Result<String> {
    if target_triple.trim().is_empty() {
        bail!("Target triple is empty");
    }

    Ok(format!("acton-{target_triple}.tar.gz"))
}

fn find_checksum_asset<'a>(release: &'a Release, archive_name: &str) -> Result<&'a Asset> {
    let checksum_name = format!("{archive_name}.sha256");

    release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(&checksum_name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Release {} is missing the checksum file {}. The release may be incomplete.",
                release.tag_name,
                checksum_name
            )
        })
}

fn verify_sha256(tarball_path: &Path, checksum_path: &Path, archive_name: &str) -> Result<()> {
    let expected = read_expected_sha256(checksum_path, archive_name)?;
    let actual = compute_sha256(tarball_path)?;

    if actual != expected {
        bail!(
            "Downloaded archive {archive_name} failed SHA256 verification. Expected {expected}, got {actual}."
        );
    }

    println!("    {} {archive_name}.sha256", "Verified".green().bold());

    Ok(())
}

fn read_expected_sha256(checksum_path: &Path, archive_name: &str) -> Result<String> {
    let contents = fs::read_to_string(checksum_path).with_context(|| {
        format!("Failed to read the downloaded checksum file for {archive_name}")
    })?;

    let line = contents
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("The downloaded checksum file for {archive_name} is empty")
        })?;

    let mut parts = line.split_whitespace();
    let checksum = parts.next().ok_or_else(|| {
        anyhow::anyhow!("The downloaded checksum file for {archive_name} has an invalid format")
    })?;

    if checksum.len() != 64 || !checksum.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("The downloaded checksum file for {archive_name} contains an invalid SHA256 digest");
    }

    if let Some(reported_name) = parts.next() {
        let reported_name = reported_name.trim_start_matches('*');
        if reported_name != archive_name {
            bail!(
                "The downloaded checksum file references '{reported_name}', but the archive is '{archive_name}'"
            );
        }
    }

    Ok(checksum.to_ascii_lowercase())
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open file {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn install_binary(tarball_path: &Path, current_exe: &Path, current_version: &str) -> Result<()> {
    let tar_gz = File::open(tarball_path).with_context(|| {
        format!(
            "Failed to open the downloaded release archive {}",
            tarball_path.display()
        )
    })?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    let mut temp_bin_path = None;

    let temp_dir = tempfile::tempdir()
        .context("Failed to prepare a temporary directory for extracting the new Acton binary")?;
    archive.unpack(&temp_dir).context(
        "Failed to extract the downloaded release archive. The archive may be corrupted.",
    )?;

    for entry in walkdir::WalkDir::new(&temp_dir) {
        let entry = entry.context("Failed while scanning the extracted release archive")?;
        if entry.file_type().is_file() && entry.file_name() == "acton" {
            temp_bin_path = Some(entry.path().to_owned());
            break;
        }
    }

    let new_bin_path = temp_bin_path.ok_or_else(|| {
        anyhow::anyhow!("The downloaded release archive does not contain an `acton` binary")
    })?;

    let bin_dir = current_exe.parent().ok_or_else(|| {
        anyhow::anyhow!("Could not determine the directory of the current Acton binary")
    })?;

    let backup_name = format!("acton-{current_version}");
    let backup_path = bin_dir.join(&backup_name);

    // 1. Keep a backup of the current binary so `acton up --rollback` can restore it offline
    fs::copy(current_exe, &backup_path).with_context(|| {
        format!(
            "Failed to create a backup of the current Acton binary at {}",
            backup_path.display()
        )
    })?;

    // 2. Atomically replace the current binary with the downloaded one
    swap_binary(&new_bin_path, current_exe, "new Acton binary")?;

    let _ = fs::remove_file(tarball_path);

    Ok(())
}

/// Atomically replace `current_exe` with the binary at `source`.
///
/// The replacement is staged in a temporary file inside the binary's own
/// directory so the final rename stays on a single filesystem and is atomic.
/// `subject` names the source binary in failure messages to make them
/// specific to the caller (new release vs restored backup).
fn swap_binary(source: &Path, current_exe: &Path, subject: &str) -> Result<()> {
    let bin_dir = current_exe.parent().ok_or_else(|| {
        anyhow::anyhow!("Could not determine the directory of the current Acton binary")
    })?;

    let temp_file = tempfile::NamedTempFile::new_in(bin_dir)
        .with_context(|| format!("Failed to create a temporary file for the {subject}"))?;

    fs::copy(source, temp_file.path())
        .with_context(|| format!("Failed to copy the {subject} into the temporary file"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(temp_file.path())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(temp_file.path(), perms)?;
    }

    temp_file.persist(current_exe).context(
        "Failed to replace the current Acton binary. Try re-running with sufficient permissions.",
    )?;

    Ok(())
}
