use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode},
};
use clap::Parser;
use expect_test::expect;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{net::TcpListener, task::JoinHandle};
use zip::{ZipWriter, write::SimpleFileOptions};

use super::*;
use crate::binaries::{
    REQUIRED_BINARIES, TonBinaries, install::install_release, release::platform_id,
};
use crate::{
    cli::{BinariesCommand, Cli, Command},
    storage::TON_RELEASE,
};

const TOKEN: &str = "test-private-token";

struct MockGitHub {
    github: GitHub,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl MockGitHub {
    async fn start(
        metadata: Value,
        archive: Vec<u8>,
        token: Option<&str>,
        metadata_status: StatusCode,
        download_status: StatusCode,
        redirect: bool,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let router = Router::new().fallback(move |request: Request<Body>| {
            let metadata = metadata.clone();
            let archive = archive.clone();
            let captured = captured.clone();
            async move {
                let path = request.uri().path();
                let authorized = request
                    .headers()
                    .get(header::AUTHORIZATION)
                    .is_some_and(|value| value == format!("Bearer {TOKEN}").as_str());
                let accept = request
                    .headers()
                    .get(header::ACCEPT)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("<none>");
                captured
                    .lock()
                    .unwrap()
                    .push(format!("{path} accept={accept} authorized={authorized}"));

                let (status, content_type, body) = match path {
                    "/repos/acme/ton/releases/tags/custom%2Fbuild" => (
                        metadata_status,
                        "application/json",
                        serde_json::to_vec(&metadata).unwrap(),
                    ),
                    "/repos/acme/ton/releases/assets/42" if redirect => {
                        return Response::builder()
                            .status(StatusCode::FOUND)
                            .header(
                                header::LOCATION,
                                format!(
                                    "http://localhost:{}/download?signature=private-signature",
                                    address.port()
                                ),
                            )
                            .body(Body::empty())
                            .unwrap();
                    }
                    "/repos/acme/ton/releases/assets/42" | "/download" => {
                        (download_status, "application/octet-stream", archive)
                    }
                    _ => (StatusCode::NOT_FOUND, "text/plain", Vec::new()),
                };
                Response::builder()
                    .status(status)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(body))
                    .unwrap()
            }
        });
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut github = GitHub::new(token).unwrap();
        github.api_base = Url::parse(&format!("http://{address}/")).unwrap();
        Self {
            github,
            requests,
            task,
        }
    }

    fn requests(&self) -> String {
        self.requests.lock().unwrap().join("\n")
    }
}

impl Drop for MockGitHub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn archive(complete: bool) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    for name in REQUIRED_BINARIES {
        if !complete && *name == "fift" {
            continue;
        }
        zip.start_file(*name, options).unwrap();
        zip.write_all(b"test binary").unwrap();
    }
    zip.add_directory("lib/", options).unwrap();
    zip.add_directory("smartcont/", options).unwrap();
    zip.finish().unwrap().into_inner()
}

fn metadata(digest: Option<&str>) -> Value {
    json!({"assets": [{"id": 42, "name": "build.zip", "digest": digest}]})
}

fn source(sha256: Option<String>) -> ReleaseSource {
    ReleaseSource::new(
        "acme/ton".into(),
        "custom/build".into(),
        Some("build.zip".into()),
        sha256,
    )
    .unwrap()
}

#[tokio::test]
async fn private_redirect_installs_reuses_and_repairs_the_cache() {
    let archive = archive(true);
    let digest = hex::encode(Sha256::digest(&archive));
    let server = MockGitHub::start(
        metadata(Some(&format!("sha256:{digest}"))),
        archive,
        Some(TOKEN),
        StatusCode::OK,
        StatusCode::OK,
        true,
    )
    .await;
    let cache = tempfile::tempdir_in("/tmp").unwrap();
    let source = source(None);

    let installed = install_release(&source, &server.github, cache.path())
        .await
        .unwrap();
    TonBinaries {
        root: installed.clone(),
    }
    .validate()
    .unwrap();
    let reused = install_release(&source, &server.github, cache.path())
        .await
        .unwrap();
    fs::remove_file(installed.join("fift")).unwrap();
    let repaired = install_release(&source, &server.github, cache.path())
        .await
        .unwrap();
    let marker = fs::read_to_string(installed.join(".complete")).unwrap();

    expect![[r#"
        reused=true repaired=true marker=true fift=true
        /repos/acme/ton/releases/tags/custom%2Fbuild accept=application/vnd.github+json authorized=true
        /repos/acme/ton/releases/assets/42 accept=application/octet-stream authorized=true
        /download accept=application/octet-stream authorized=false
        /repos/acme/ton/releases/tags/custom%2Fbuild accept=application/vnd.github+json authorized=true
        /repos/acme/ton/releases/tags/custom%2Fbuild accept=application/vnd.github+json authorized=true
        /repos/acme/ton/releases/assets/42 accept=application/octet-stream authorized=true
        /download accept=application/octet-stream authorized=false"#]].assert_eq(&format!(
        "reused={} repaired={} marker={} fift={}\n{}",
        installed == reused, installed == repaired, marker == format!("custom/build\n{digest}\n"),
        installed.join("fift").is_file(), server.requests(),
    ));
}

#[tokio::test]
async fn public_release_with_explicit_checksum_can_be_reused_offline() {
    let archive = archive(true);
    let source = source(Some(hex::encode(Sha256::digest(&archive))));
    let server = MockGitHub::start(
        metadata(None),
        archive,
        None,
        StatusCode::OK,
        StatusCode::OK,
        false,
    )
    .await;
    let cache = tempfile::tempdir_in("/tmp").unwrap();

    let installed = install_release(&source, &server.github, cache.path())
        .await
        .unwrap();
    server.task.abort();
    let reused = install_release(&source, &server.github, cache.path())
        .await
        .unwrap();

    expect![[r#"
        reused=true
        /repos/acme/ton/releases/tags/custom%2Fbuild accept=application/vnd.github+json authorized=false
        /repos/acme/ton/releases/assets/42 accept=application/octet-stream authorized=false"#]].assert_eq(&format!(
        "reused={}\n{}", installed == reused, server.requests(),
    ));
}

#[tokio::test]
async fn failed_downloads_never_publish_an_installation_or_expose_credentials() {
    let mut outcomes = Vec::new();
    for case in [
        "unauthorized",
        "missing asset",
        "missing digest",
        "invalid digest",
        "checksum mismatch",
        "incomplete archive",
        "download failure",
    ] {
        let archive = archive(case != "incomplete archive");
        let actual_digest = hex::encode(Sha256::digest(&archive));
        let digest = if case == "checksum mismatch" {
            "0".repeat(64)
        } else {
            actual_digest.clone()
        };
        let mut metadata = metadata(Some(&format!("sha256:{digest}")));
        match case {
            "missing asset" => metadata["assets"] = json!([]),
            "missing digest" => metadata["assets"][0]["digest"] = Value::Null,
            "invalid digest" => metadata["assets"][0]["digest"] = json!("sha256:invalid"),
            _ => {}
        }
        let metadata_status = if case == "unauthorized" {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::OK
        };
        let download_status = if case == "download failure" {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::OK
        };
        let server = MockGitHub::start(
            metadata,
            archive,
            Some(TOKEN),
            metadata_status,
            download_status,
            true,
        )
        .await;
        let cache = tempfile::tempdir_in("/tmp").unwrap();
        let source = source(None);

        let error = install_release(&source, &server.github, cache.path())
            .await
            .unwrap_err();
        let message = format!("{error:#}");
        let release_dir = source.cache_dir(cache.path(), &digest);
        let platform = platform_id();
        let clean = !release_dir.join(&platform).exists()
            && !release_dir.join(format!(".{platform}.zip.part")).exists()
            && !release_dir.join(format!(".{platform}.installing")).exists();
        let redacted = !message.contains(TOKEN) && !message.contains("private-signature");
        let cause = format!("{}", error.root_cause())
            .replace(&actual_digest, "<actual-sha256>")
            .replace(&digest, "<expected-sha256>")
            .replace(&release_dir.display().to_string(), "<release-cache>")
            .replace(&platform, "<platform>");
        outcomes.push(format!(
            "{case}: clean={clean} redacted={redacted}\n  {cause}"
        ));
    }
    expect![[r#"
        unauthorized: clean=true redacted=true
          HTTP status client error (401 Unauthorized)
        missing asset: clean=true redacted=true
          GitHub release acme/ton@custom/build has no asset build.zip
        missing digest: clean=true redacted=true
          GitHub asset has no SHA-256 digest; supply --sha256 with the expected checksum
        invalid digest: clean=true redacted=true
          SHA-256 must contain exactly 64 hexadecimal digits
        checksum mismatch: clean=true redacted=true
          TON archive checksum mismatch: expected <expected-sha256>, got <actual-sha256>
        incomplete archive: clean=true redacted=true
          required TON binary is missing: <release-cache>/.<platform>.installing/fift
        download failure: clean=true redacted=true
          HTTP status client error (403 Forbidden)"#]]
    .assert_eq(&outcomes.join("\n"));
}

#[tokio::test]
async fn concurrent_installs_download_the_archive_once() {
    let archive = archive(true);
    let digest = hex::encode(Sha256::digest(&archive));
    let server = MockGitHub::start(
        metadata(None),
        archive,
        Some(TOKEN),
        StatusCode::OK,
        StatusCode::OK,
        false,
    )
    .await;
    let cache = tempfile::tempdir_in("/tmp").unwrap();
    let source = source(Some(digest));
    let (first, second) = tokio::join!(
        install_release(&source, &server.github, cache.path()),
        install_release(&source, &server.github, cache.path()),
    );

    expect![[r#"
        same=true
        /repos/acme/ton/releases/tags/custom%2Fbuild accept=application/vnd.github+json authorized=true
        /repos/acme/ton/releases/assets/42 accept=application/octet-stream authorized=true"#]].assert_eq(&format!(
        "same={}\n{}", first.unwrap() == second.unwrap(), server.requests(),
    ));
}

#[test]
fn cache_separates_repositories_tags_assets_and_checksums() {
    let root = Path::new("cache");
    let digest = "a".repeat(64);
    let source = ReleaseSource::new(
        "acme/ton".into(),
        "custom".into(),
        Some("build.zip".into()),
        Some(digest.clone()),
    )
    .unwrap();
    let baseline = source.cache_dir(root, &digest);
    let mut outcomes = Vec::new();
    for (label, repository, tag, asset, checksum) in [
        (
            "repository",
            "other/ton",
            "custom",
            "build.zip",
            digest.clone(),
        ),
        ("tag", "acme/ton", "other", "build.zip", digest.clone()),
        ("asset", "acme/ton", "custom", "other.zip", digest.clone()),
        (
            "checksum",
            "acme/ton",
            "custom",
            "build.zip",
            "b".repeat(64),
        ),
        ("case", "ACME/TON", "custom", "build.zip", digest.clone()),
    ] {
        let source = ReleaseSource::new(
            repository.into(),
            tag.into(),
            Some(asset.into()),
            Some(checksum.clone()),
        )
        .unwrap();
        outcomes.push(format!(
            "{label}: same={}",
            source.cache_dir(root, &checksum) == baseline
        ));
    }
    let pinned = ReleaseSource::pinned().unwrap();
    outcomes.push(format!(
        "official: preserved={}",
        pinned.cache_dir(root, pinned.sha256.as_deref().unwrap())
            == root.join("ton").join(TON_RELEASE)
    ));

    expect_test::expect![[r#"
        repository: same=false
        tag: same=false
        asset: same=false
        checksum: same=false
        case: same=true
        official: preserved=true"#]]
    .assert_eq(&outcomes.join("\n"));
}

#[test]
fn invalid_sources_are_rejected_before_installation() {
    let mut outcomes = Vec::new();
    for (repository, tag, asset, checksum) in [
        ("https://github.com/acme/ton", "custom", "build.zip", None),
        ("../ton", "custom", "build.zip", None),
        ("acme/ton", "..", "build.zip", None),
        ("acme/ton", "tag\nmarker", "build.zip", None),
        ("acme/ton", "custom", "../build.zip", None),
        ("acme/ton", "custom", "build.tar.gz", None),
        ("acme/ton", "custom", "build.zip", Some("invalid")),
    ] {
        let result = ReleaseSource::new(
            repository.into(),
            tag.into(),
            Some(asset.into()),
            checksum.map(str::to_owned),
        );
        outcomes.push(result.err().unwrap().to_string());
    }
    expect_test::expect![[r#"
        repository must use OWNER/REPO format
        repository must use OWNER/REPO format
        release must be a nonempty GitHub tag
        release must be a nonempty GitHub tag
        asset must be a ZIP filename
        asset must be a ZIP filename
        SHA-256 must contain exactly 64 hexadecimal digits"#]]
    .assert_eq(&outcomes.join("\n"));
}

#[test]
fn custom_binary_install_options_do_not_expose_the_token() {
    let cli = Cli::try_parse_from([
        "localton",
        "binaries",
        "install",
        "--repository",
        "my-org/ton",
        "--release",
        "custom/build",
        "--asset",
        "build.zip",
        "--sha256",
        &"a".repeat(64),
        "--github-token",
        "private-test-token",
    ])
    .unwrap();

    expect_test::expect![[r#"
        Cli { command: Binaries { command: Install(BinariesInstallArgs { repository: "my-org/ton", release: "custom/build", asset: Some("build.zip"), sha256: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), .. }) } }"#]]
        .assert_eq(&format!("{cli:?}"));
    let Command::Binaries {
        command: BinariesCommand::Install(args),
    } = cli.command
    else {
        panic!("expected binary installation");
    };
    assert_eq!(args.github_token.as_deref(), Some("private-test-token"));
}
