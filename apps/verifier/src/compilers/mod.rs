use std::{path::PathBuf, process::ExitStatus};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::{self, Duration},
};

use crate::config::Config;

#[async_trait]
pub trait CompilerService: Send + Sync + 'static {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError>;
}

pub struct NodeCompilerService {
    node_bin: String,
    worker_path: PathBuf,
    timeout: Duration,
}

impl NodeCompilerService {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            node_bin: config.compiler_node_bin().to_owned(),
            worker_path: config.compiler_worker_path().to_path_buf(),
            timeout: config.compiler_timeout(),
        }
    }
}

#[async_trait]
impl CompilerService for NodeCompilerService {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError> {
        let input = serde_json::to_vec(&request).map_err(CompilerError::SerializeInput)?;
        let mut child = Command::new(&self.node_bin)
            .arg(&self.worker_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(CompilerError::Spawn)?;

        let mut stdin = child.stdin.take().ok_or(CompilerError::MissingStdin)?;
        stdin
            .write_all(&input)
            .await
            .map_err(CompilerError::WriteStdin)?;
        drop(stdin);

        let output = time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| CompilerError::Timeout {
                timeout_ms: self.timeout.as_millis(),
            })?
            .map_err(CompilerError::Wait)?;

        if !output.status.success() {
            return Err(CompilerError::WorkerFailed {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let output = serde_json::from_slice::<WorkerOutput>(&output.stdout)
            .map_err(CompilerError::DeserializeOutput)?;

        match output {
            WorkerOutput::Ok { code_hash } => Ok(CompileOutput { code_hash }),
            WorkerOutput::CompileError { error } => Err(CompilerError::CompileFailed(error)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CompileRequest {
    pub language: String,
    pub compiler_version: String,
    pub root_dir: PathBuf,
    pub entrypoint: String,
    pub sources: Vec<CompileSource>,
}

#[derive(Debug, Serialize)]
pub struct CompileSource {
    pub path: String,
}

pub struct CompileOutput {
    pub code_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WorkerOutput {
    Ok { code_hash: String },
    CompileError { error: String },
}

#[derive(Debug, Error)]
pub enum CompilerError {
    #[error("failed to serialize compiler input: {0}")]
    SerializeInput(serde_json::Error),
    #[error("failed to spawn compiler worker: {0}")]
    Spawn(std::io::Error),
    #[error("compiler worker stdin was not available")]
    MissingStdin,
    #[error("failed to write compiler worker stdin: {0}")]
    WriteStdin(std::io::Error),
    #[error("compiler worker timed out after {timeout_ms} ms")]
    Timeout { timeout_ms: u128 },
    #[error("failed to wait for compiler worker: {0}")]
    Wait(std::io::Error),
    #[error("compiler worker failed with status {status}: {stderr}")]
    WorkerFailed { status: ExitStatus, stderr: String },
    #[error("failed to parse compiler worker output: {0}")]
    DeserializeOutput(serde_json::Error),
    #[error("compile error: {0}")]
    CompileFailed(String),
}
