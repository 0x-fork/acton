use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use tokio::process::Command;
use tracing::info;

const REPOSITORY: &str = "https://github.com/toncenter/ton-indexer.git";
const COMMIT: &str = "eb9fbfa3212a583d3eef672f74b98600dfdd898c";
const BUILD_SCHEMA: &str = "1";
const SWAG_VERSION: &str = "v1.16.3";
const PATCHES: &[(&str, &str)] = &[
    (
        "ton-indexer-cors.patch",
        include_str!("../../docker/ton-indexer-cors.patch"),
    ),
    (
        "ton-indexer-classifier-cpu.patch",
        include_str!("../../docker/ton-indexer-classifier-cpu.patch"),
    ),
    (
        "ton-indexer-v3-catchup.patch",
        include_str!("../../docker/ton-indexer-v3-catchup.patch"),
    ),
    (
        "ton-indexer-localton-scanner.patch",
        include_str!("../../docker/ton-indexer-localton-scanner.patch"),
    ),
];

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// State directory whose tools directory receives sources and build artifacts.
    #[arg(long, default_value = ".localton")]
    state_dir: PathBuf,

    /// Installation directory. Defaults to STATE_DIR/tools/ton-http-api-v3/install.
    #[arg(long)]
    install_dir: Option<PathBuf>,

    /// Component to build. The API also requires the worker's native libraries.
    #[arg(long, value_enum, default_value = "all")]
    component: Component,

    /// Number of parallel native compilation jobs.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=64))]
    jobs: u8,

    /// Source repository override. Also accepts TON_INDEXER_REPOSITORY.
    #[arg(long)]
    repository: Option<String>,

    /// Pinned source commit override (full SHA). Also accepts TON_INDEXER_COMMIT.
    #[arg(long)]
    commit: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Component {
    All,
    Worker,
    Api,
    Classifier,
}

struct Paths {
    root: PathBuf,
    source: PathBuf,
    build: PathBuf,
    install: PathBuf,
}

impl Paths {
    fn new(state_dir: &Path, install_dir: Option<&Path>) -> Result<Self> {
        let root = absolute_path(state_dir)?.join("tools/ton-http-api-v3");
        Ok(Self {
            source: root.join("source"),
            build: root.join("build-worker"),
            install: install_dir
                .map(absolute_path)
                .transpose()?
                .unwrap_or_else(|| root.join("install")),
            root,
        })
    }
}

pub async fn build(args: BuildArgs) -> Result<()> {
    let paths = Paths::new(&args.state_dir, args.install_dir.as_deref())?;
    let repository = args
        .repository
        .or_else(|| nonempty_env("TON_INDEXER_REPOSITORY"))
        .unwrap_or_else(|| REPOSITORY.to_owned());
    let commit = args
        .commit
        .or_else(|| nonempty_env("TON_INDEXER_COMMIT"))
        .unwrap_or_else(|| COMMIT.to_owned());
    ensure!(
        commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "TON Indexer commit must be a full 40-character SHA"
    );
    prepare_source(&paths, &repository, &commit, PATCHES).await?;
    fs::create_dir_all(&paths.install)?;
    fs::copy(paths.source.join("LICENSE"), paths.install.join("LICENSE"))?;

    match args.component {
        Component::All => {
            build_worker(&paths, args.jobs).await?;
            build_api(&paths).await?;
            build_classifier(&paths).await?;
        }
        Component::Worker => build_worker(&paths, args.jobs).await?,
        Component::Api => {
            // Docker's API stage inherits the worker artifacts from its parent.
            if !worker_is_installed(&paths)? {
                build_worker(&paths, args.jobs).await?;
            }
            build_api(&paths).await?;
        }
        Component::Classifier => build_classifier(&paths).await?,
    }

    println!("TON Center API V3 source: {commit}");
    println!("installed: {}", paths.install.display());
    Ok(())
}

async fn prepare_source(
    paths: &Paths,
    repository: &str,
    commit: &str,
    patches: &[(&str, &str)],
) -> Result<()> {
    fs::create_dir_all(&paths.root)?;
    let stamp = paths.root.join(".source-version");
    let expected = serde_json::to_string(&(BUILD_SCHEMA, repository, commit, patches))?;
    if paths.source.exists() {
        ensure!(
            paths.source.join(".git").is_dir(),
            "{} exists but is not a TON Indexer source checkout",
            paths.source.display()
        );
    } else {
        run(
            "initialize TON Indexer source checkout",
            Command::new("git").args(["init", "-q"]).arg(&paths.source),
        )
        .await?;
    }

    let head = Command::new("git")
        .current_dir(&paths.source)
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .context("failed to inspect TON Indexer checkout")?;
    if head.status.success()
        && String::from_utf8_lossy(&head.stdout).trim() == commit
        && fs::read_to_string(&stamp).is_ok_and(|actual| actual == expected)
    {
        return Ok(());
    }

    // Invalidate before updating so an interrupted checkout is retried next time.
    if stamp.exists() {
        fs::remove_file(&stamp)?;
    }
    run(
        "configure TON Indexer source remote",
        Command::new("git").current_dir(&paths.source).args([
            "config",
            "remote.origin.url",
            repository,
        ]),
    )
    .await?;
    run(
        "fetch pinned TON Indexer source",
        Command::new("git")
            .current_dir(&paths.source)
            .args(["fetch", "--depth", "1", "origin", commit]),
    )
    .await?;
    run(
        "check out pinned TON Indexer source",
        Command::new("git")
            .current_dir(&paths.source)
            .args(["checkout", "--detach", "--force", commit]),
    )
    .await?;
    run(
        "update TON Indexer submodules",
        Command::new("git").current_dir(&paths.source).args([
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--depth",
            "1",
        ]),
    )
    .await?;

    let patch_dir = paths.root.join("patches");
    fs::create_dir_all(&patch_dir)?;
    for (name, contents) in patches {
        let patch = patch_dir.join(name);
        fs::write(&patch, contents)?;
        run(
            "apply TON Indexer patch",
            Command::new("git")
                .current_dir(&paths.source)
                .arg("apply")
                .arg(&patch),
        )
        .await?;
    }
    if paths.build.exists() {
        fs::remove_dir_all(&paths.build)?;
    }
    fs::write(stamp, expected)?;
    Ok(())
}

fn worker_is_installed(paths: &Paths) -> Result<bool> {
    let expected = fs::read_to_string(paths.root.join(".source-version"))?;
    if !fs::read_to_string(paths.install.join(".worker-version"))
        .is_ok_and(|actual| actual == expected)
        || !paths.install.join("include/wrapper.h").is_file()
    {
        return Ok(false);
    }
    let libraries = marker_libraries(&paths.install.join("lib"))?;
    Ok(["libton-marker.", "libton-marker-core."]
        .iter()
        .all(|prefix| {
            libraries.iter().any(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(prefix)
            })
        }))
}

async fn build_worker(paths: &Paths, jobs: u8) -> Result<()> {
    let stamp = paths.install.join(".worker-version");
    if stamp.exists() {
        fs::remove_file(&stamp)?;
    }
    let ccache = env::var_os("CCACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.root.join("ccache"));
    fs::create_dir_all(&ccache)?;
    run(
        "configure TON Indexer worker",
        Command::new("cmake")
            .arg("-S")
            .arg(paths.source.join("ton-index-worker"))
            .arg("-B")
            .arg(&paths.build)
            .args([
                "-G",
                "Ninja",
                "-DCMAKE_BUILD_TYPE=Release",
                "-DCMAKE_C_COMPILER_LAUNCHER=ccache",
                "-DCMAKE_CXX_COMPILER_LAUNCHER=ccache",
                "-DPORTABLE=1",
                "-DTON_ARCH=",
            ])
            .env("CCACHE_DIR", &ccache),
    )
    .await?;
    run(
        "build TON Indexer worker",
        Command::new("cmake")
            .arg("--build")
            .arg(&paths.build)
            .args(["--parallel", &jobs.to_string(), "--target"])
            .args([
                "ton-index-postgres",
                "ton-index-postgres-migrate",
                "ton-smc-scanner",
                "ton-marker",
                "ton-marker-cli",
                "ton-marker-core",
            ])
            .env("CCACHE_DIR", &ccache),
    )
    .await?;

    let bin_dir = paths.install.join("bin");
    fs::create_dir_all(&bin_dir)?;
    for (directory, binary) in [
        ("ton-index-postgres", "ton-index-postgres"),
        ("ton-index-postgres", "ton-index-postgres-migrate"),
        ("ton-smc-scanner", "ton-smc-scanner"),
    ] {
        run(
            "install TON Indexer worker binary",
            Command::new("install")
                .args(["-m", "0755"])
                .arg(paths.build.join(directory).join(binary))
                .arg(bin_dir.join(binary)),
        )
        .await?;
    }
    install_marker_libraries(&paths.build.join("ton-marker"), &paths.install.join("lib")).await?;
    fs::create_dir_all(paths.install.join("include"))?;
    fs::copy(
        paths
            .source
            .join("ton-index-worker/ton-marker/src/wrapper.h"),
        paths.install.join("include/wrapper.h"),
    )?;
    fs::copy(paths.root.join(".source-version"), stamp)?;
    Ok(())
}

async fn build_api(paths: &Paths) -> Result<()> {
    let tools_bin = paths.root.join("bin");
    fs::create_dir_all(&tools_bin)?;
    fs::create_dir_all(paths.install.join("bin"))?;
    let source = paths.source.join("ton-index-go");
    run(
        "install TON Indexer OpenAPI generator",
        Command::new("go")
            .current_dir(&source)
            .args([
                "install",
                &format!("github.com/swaggo/swag/cmd/swag@{SWAG_VERSION}"),
            ])
            .env("GOBIN", &tools_bin),
    )
    .await?;
    run(
        "generate TON Indexer OpenAPI documentation",
        Command::new(tools_bin.join("swag"))
            .current_dir(&source)
            .arg("init"),
    )
    .await?;
    let mut command = Command::new("go");
    command
        .current_dir(source)
        .args(["build", "-trimpath", "-ldflags=-s -w -buildid=", "-o"])
        .arg(paths.install.join("bin/ton-index-go"))
        .arg("./main.go")
        .env("CGO_ENABLED", "1");
    prepend_path(&mut command, "CPATH", &paths.install.join("include"))?;
    prepend_path(&mut command, "LIBRARY_PATH", &paths.install.join("lib"))?;
    prepend_path(&mut command, "LD_LIBRARY_PATH", &paths.install.join("lib"))?;
    run("build TON Indexer API", &mut command).await
}

async fn build_classifier(paths: &Paths) -> Result<()> {
    let venv = paths.install.join("venv");
    run(
        "create TON Indexer classifier environment",
        Command::new("python3").args(["-m", "venv"]).arg(&venv),
    )
    .await?;
    run(
        "install TON Indexer classifier dependencies",
        Command::new(venv.join("bin/python"))
            .args(["-m", "pip", "install", "-r"])
            .arg(paths.source.join("indexer/requirements.txt")),
    )
    .await?;
    let classifier = paths.install.join("classifier");
    fs::create_dir_all(&classifier)?;
    run(
        "install TON Indexer classifier",
        Command::new("cp")
            .arg("-a")
            .arg(paths.source.join("indexer/."))
            .arg(classifier),
    )
    .await
}

fn marker_libraries(directory: &Path) -> Result<Vec<PathBuf>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut libraries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("libton-marker")
        {
            libraries.push(entry.path());
        }
    }
    libraries.sort();
    Ok(libraries)
}

async fn install_marker_libraries(source: &Path, destination: &Path) -> Result<()> {
    let libraries = marker_libraries(source)?;
    ensure!(
        !libraries.is_empty(),
        "TON Indexer marker libraries are missing: {}",
        source.display()
    );
    fs::create_dir_all(destination)?;
    run(
        "install TON Indexer marker libraries",
        Command::new("cp")
            .arg("-a")
            .args(libraries)
            .arg(destination),
    )
    .await
}

fn prepend_path(command: &mut Command, name: &str, path: &Path) -> Result<()> {
    let mut paths = vec![path.to_owned()];
    if let Some(existing) = env::var_os(name) {
        paths.extend(env::split_paths(&existing));
    }
    command.env(name, env::join_paths(paths)?);
    Ok(())
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

async fn run(description: &str, command: &mut Command) -> Result<()> {
    info!("{description}");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .with_context(|| format!("failed to {description}"))?;
    ensure!(status.success(), "{description} failed with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/value.txt b/value.txt\n--- a/value.txt\n+++ b/value.txt\n@@ -1 +1 @@\n-original\n+patched\n";

    async fn fixture() -> (tempfile::TempDir, Paths, PathBuf, String) {
        let temp = tempfile::tempdir().unwrap();
        let upstream = temp.path().join("upstream");
        fs::create_dir(&upstream).unwrap();
        git(&upstream, &["init", "-q"]).await;
        fs::write(upstream.join("value.txt"), "original\n").unwrap();
        git(&upstream, &["add", "value.txt"]).await;
        git(
            &upstream,
            &[
                "-c",
                "user.name=Localton test",
                "-c",
                "user.email=localton@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .await;
        let commit = git(&upstream, &["rev-parse", "HEAD"]).await;
        let paths = Paths::new(&temp.path().join("state with spaces"), None).unwrap();
        (temp, paths, upstream, commit)
    }

    async fn git(directory: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[tokio::test]
    async fn source_reuses_patches_and_invalidates_native_build_when_they_change() {
        let (_temp, paths, upstream, commit) = fixture().await;
        let repository = upstream.to_str().unwrap();
        let patches = [("change.patch", PATCH)];
        prepare_source(&paths, repository, &commit, &patches)
            .await
            .unwrap();
        fs::create_dir_all(&paths.build).unwrap();
        fs::write(paths.build.join("cached-object"), "compiled").unwrap();
        fs::create_dir_all(paths.install.join("include")).unwrap();
        fs::write(paths.install.join("include/wrapper.h"), "installed").unwrap();
        fs::copy(
            paths.root.join(".source-version"),
            paths.install.join(".worker-version"),
        )
        .unwrap();
        fs::create_dir_all(paths.install.join("lib")).unwrap();
        fs::write(paths.install.join("lib/libton-marker.so"), "shared").unwrap();
        fs::write(paths.install.join("lib/libton-marker-core.a"), "static").unwrap();

        prepare_source(&paths, repository, &commit, &patches)
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(paths.source.join("value.txt")).unwrap(),
            "patched\n"
        );
        assert!(paths.build.join("cached-object").is_file());
        assert!(paths.install.join("include/wrapper.h").is_file());
        assert!(worker_is_installed(&paths).unwrap());

        let changed_patch = PATCH.replace("+patched", "+updated");
        prepare_source(
            &paths,
            repository,
            &commit,
            &[("change.patch", &changed_patch)],
        )
        .await
        .unwrap();
        assert_eq!(
            fs::read_to_string(paths.source.join("value.txt")).unwrap(),
            "updated\n"
        );
        assert!(!paths.build.exists());
        assert!(!worker_is_installed(&paths).unwrap());
        assert_eq!(git(&paths.source, &["rev-parse", "HEAD"]).await, commit);
    }

    #[tokio::test]
    async fn failed_patch_is_retried_from_the_pinned_checkout() {
        let (_temp, paths, upstream, commit) = fixture().await;
        let repository = upstream.to_str().unwrap();
        let bad_patch = PATCH.replace("-original", "-missing");
        let error = prepare_source(
            &paths,
            repository,
            &commit,
            &[("first.patch", PATCH), ("bad.patch", &bad_patch)],
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("apply TON Indexer patch failed"));
        assert!(!paths.root.join(".source-version").exists());

        prepare_source(&paths, repository, &commit, &[("change.patch", PATCH)])
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(paths.source.join("value.txt")).unwrap(),
            "patched\n"
        );
    }

    #[tokio::test]
    async fn source_rejects_an_existing_directory_without_git() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::new(temp.path(), None).unwrap();
        fs::create_dir_all(&paths.source).unwrap();
        fs::write(paths.source.join("keep"), "user data").unwrap();

        let error = prepare_source(&paths, REPOSITORY, COMMIT, PATCHES)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("is not a TON Indexer source checkout")
        );
        assert_eq!(
            fs::read_to_string(paths.source.join("keep")).unwrap(),
            "user data"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn marker_install_preserves_shared_library_symlinks_and_static_archives() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("build with spaces");
        let destination = temp.path().join("install/lib");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("libton-marker.so.1"), "shared library").unwrap();
        fs::write(source.join("libton-marker-core.a"), "static library").unwrap();
        fs::write(source.join("CMakeCache.txt"), "build only").unwrap();
        std::os::unix::fs::symlink("libton-marker.so.1", source.join("libton-marker.so")).unwrap();

        install_marker_libraries(&source, &destination)
            .await
            .unwrap();

        assert_eq!(
            fs::read_link(destination.join("libton-marker.so")).unwrap(),
            Path::new("libton-marker.so.1")
        );
        assert_eq!(
            fs::read_to_string(destination.join("libton-marker.so")).unwrap(),
            "shared library"
        );
        assert!(destination.join("libton-marker-core.a").is_file());
        assert!(!destination.join("CMakeCache.txt").exists());
    }
}
