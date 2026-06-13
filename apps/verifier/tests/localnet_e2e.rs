use std::{
    error::Error,
    ffi::OsStr,
    fmt::Write as _,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::Arc,
    thread,
    time::Duration,
};

use axum::http::StatusCode;
use serde::Deserialize;
use verifier::{
    blockchain::{BlockchainClient, ToncenterClient},
    config::Config,
    registry::TonRegistryClient,
};

mod support;

use support::{file_part, owned_text_part, post_verify, response_json, text_part};

const LOCALNET_PORT: &str = "5412";
const LOCALNET_READY_ATTEMPTS: usize = 60;
const COMPILE_PARAMS_TOLK: &str = r#"{"compiler_version":"1.4.1"}"#;
const SOURCES_MAIN: &str = r#"[{"path":"main.tolk","is_entrypoint":true}]"#;
const LOCALNET_DEPLOYER_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

#[tokio::test]
async fn verify_resolves_master_code_hash_from_localnet_backend() -> Result<(), Box<dyn Error>> {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let _wallets = WalletsGuard::install_test_wallet(&project_root)?;

    if localnet_is_running(&project_root)? {
        return Err(format!(
            "localnet is already running on port {LOCALNET_PORT}; stop it before running isolated e2e tests"
        )
        .into());
    }

    let mut localnet = LocalnetGuard::start(&project_root)?;
    localnet.wait_until_ready()?;

    let output = run_command(
        &project_root,
        "acton",
        ["script", "scripts/e2e-localnet.tolk", "--net", "localnet"],
    )?;
    assert_success(
        &output,
        "acton script scripts/e2e-localnet.tolk --net localnet",
    )?;

    let script_stdout = String::from_utf8(output.stdout)?;
    let master_address = find_labeled_value(&script_stdout, "RegistryMaster")?;
    let localnet_base_url = localnet_base_url();
    let localnet_client = ToncenterClient::new(localnet_base_url.clone(), None);
    let onchain_code_hash = localnet_client
        .get_code_hash(&master_address)
        .await?
        .ok_or_else(|| format!("localnet did not return code hash for {master_address}"))?;
    let verifier_config = write_localnet_verifier_config(
        &project_root,
        &localnet_base_url,
        &master_address,
        LOCALNET_DEPLOYER_MNEMONIC,
    )?;
    let verifier_config = Config::load_from_path(verifier_config)?;
    let registry_client = Arc::new(TonRegistryClient::from_config(&verifier_config));

    let response = post_verify(
        support::toncenter_app_state_with_registry(
            &localnet_base_url,
            &onchain_code_hash,
            registry_client,
        ),
        vec![
            owned_text_part("address", master_address.clone()),
            text_part("language", "tolk"),
            text_part("compile_params", COMPILE_PARAMS_TOLK),
            text_part("sources", SOURCES_MAIN),
            file_part("files", "main.tolk", "text/plain", "fun main() {}"),
        ],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerifyResponse>(response).await;
    assert_eq!(body.address.as_deref(), Some(master_address.as_str()));
    assert_eq!(body.code_hash, onchain_code_hash);
    assert_eq!(body.compiled_code_hash, body.code_hash);
    assert_eq!(body.verification_result, "match");
    assert_eq!(body.language, "tolk");
    assert_eq!(
        body.onchain_registration
            .as_ref()
            .map(|registration| registration.status.as_str()),
        Some("confirmed")
    );
    assert_eq!(
        body.onchain_registration
            .as_ref()
            .map(|registration| registration.master_address.as_str()),
        Some(master_address.as_str())
    );
    let source_bundle_hash = body
        .source_bundle_hash
        .as_deref()
        .ok_or("verification response did not include source_bundle_hash")?;
    assert_eq!(source_bundle_hash.len(), 64);
    let verification_record_address = body
        .onchain_registration
        .as_ref()
        .map(|registration| registration.verification_record_address.as_str())
        .ok_or("verification response did not include record address")?;
    assert_eq!(verification_record_address.len(), 48);

    Ok(())
}

struct WalletsGuard {
    wallets_path: PathBuf,
    backup_path: Option<PathBuf>,
}

impl WalletsGuard {
    fn install_test_wallet(project_root: &Path) -> Result<Self, Box<dyn Error>> {
        let build_dir = project_root.join("build");
        fs::create_dir_all(&build_dir)?;

        let wallets_path = project_root.join("wallets.toml");
        let backup_path = wallets_path.exists().then(|| {
            build_dir.join(format!(
                "localnet-e2e.wallets.toml.backup.{}",
                std::process::id()
            ))
        });

        if let Some(path) = &backup_path {
            fs::copy(&wallets_path, path)?;
        }

        fs::copy(
            project_root.join("tests/fixtures/localnet-wallets.toml"),
            &wallets_path,
        )?;

        Ok(Self {
            wallets_path,
            backup_path,
        })
    }
}

impl Drop for WalletsGuard {
    fn drop(&mut self) {
        if let Some(path) = &self.backup_path {
            let _ = fs::rename(path, &self.wallets_path);
            return;
        }

        let _ = fs::remove_file(&self.wallets_path);
    }
}

struct LocalnetGuard {
    child: Child,
    log_path: PathBuf,
    project_root: PathBuf,
}

impl LocalnetGuard {
    fn start(project_root: &Path) -> Result<Self, Box<dyn Error>> {
        let log_path = project_root.join("build/localnet-e2e.log");
        let log = File::create(&log_path)?;
        let child = Command::new("acton")
            .args([
                "localnet",
                "start",
                "--port",
                LOCALNET_PORT,
                "--accounts",
                "deployer",
            ])
            .current_dir(project_root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;

        Ok(Self {
            child,
            log_path,
            project_root: project_root.to_path_buf(),
        })
    }

    fn wait_until_ready(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..LOCALNET_READY_ATTEMPTS {
            if localnet_is_running(&self.project_root)? {
                return Ok(());
            }

            if let Some(status) = self.child.try_wait()? {
                return Err(format!(
                    "localnet exited before becoming ready with status {status}\n{}",
                    self.log_tail()
                )
                .into());
            }

            thread::sleep(Duration::from_secs(1));
        }

        Err(format!(
            "localnet did not become ready within {LOCALNET_READY_ATTEMPTS} seconds\n{}",
            self.log_tail()
        )
        .into())
    }

    fn log_tail(&self) -> String {
        match fs::read_to_string(&self.log_path) {
            Ok(contents) => tail_lines(&contents, 100),
            Err(err) => format!("failed to read localnet log: {err}"),
        }
    }
}

impl Drop for LocalnetGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn localnet_is_running(project_root: &Path) -> Result<bool, Box<dyn Error>> {
    let output = Command::new("acton")
        .args(["localnet", "status", "--port", LOCALNET_PORT, "--json"])
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let status = serde_json::from_slice::<LocalnetStatus>(&output.stdout)?;
    Ok(status.running)
}

fn localnet_base_url() -> String {
    format!("http://127.0.0.1:{LOCALNET_PORT}")
}

fn write_localnet_verifier_config(
    project_root: &Path,
    base_url: &str,
    master_address: &str,
    mnemonic: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = project_root.join("build/localnet-e2e-verifier.toml");
    fs::write(
        &path,
        format!(
            r#"
[network]
name = "localnet"

[toncenter]
base_url = "{base_url}"

[registry]
master_address = "{master_address}"
register_value_nano = 500000000
confirmation_attempts = 30
confirmation_delay_ms = 200

[wallet]
kind = "v5r1"
workchain = 0
mnemonic = "{mnemonic}"
"#
        ),
    )?;

    Ok(path)
}

fn run_command<I, S>(project_root: &Path, program: &str, args: I) -> Result<Output, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Ok(Command::new(program)
        .args(args)
        .current_dir(project_root)
        .output()?)
}

fn find_labeled_value(output: &str, label: &str) -> Result<String, Box<dyn Error>> {
    let prefix = format!("{label}: ");
    output
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .and_then(|value| value.split_whitespace().next())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("missing `{label}: ...` line in script output:\n{output}").into())
}

fn assert_success(output: &Output, command: &str) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        return Ok(());
    }

    let mut message = format!("{command} failed with status {}", output.status);
    append_output(&mut message, "stdout", &output.stdout);
    append_output(&mut message, "stderr", &output.stderr);

    Err(message.into())
}

fn append_output(message: &mut String, name: &str, output: &[u8]) {
    if output.is_empty() {
        return;
    }

    let text = String::from_utf8_lossy(output);
    let _ = write!(message, "\n{name}:\n{text}");
}

fn tail_lines(contents: &str, lines: usize) -> String {
    let mut selected = contents.lines().rev().take(lines).collect::<Vec<_>>();
    selected.reverse();
    selected.join("\n")
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    address: Option<String>,
    code_hash: String,
    compiled_code_hash: String,
    verification_result: String,
    source_bundle_hash: Option<String>,
    onchain_registration: Option<OnchainRegistration>,
    language: String,
}

#[derive(Debug, Deserialize)]
struct OnchainRegistration {
    status: String,
    master_address: String,
    verification_record_address: String,
}

#[derive(Debug, Deserialize)]
struct LocalnetStatus {
    running: bool,
}
