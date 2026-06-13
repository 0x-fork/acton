use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use verifier::compilers::{CompileOutput, CompileRequest, CompilerError, CompilerService};

pub struct MockCompilerService {
    code_hash: String,
    recorded_requests: Arc<Mutex<Vec<CompileRequest>>>,
}

impl MockCompilerService {
    pub fn new(code_hash: &str) -> Self {
        Self {
            code_hash: code_hash.to_owned(),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn recorded_requests(&self) -> Arc<Mutex<Vec<CompileRequest>>> {
        Arc::clone(&self.recorded_requests)
    }
}

#[async_trait]
impl CompilerService for MockCompilerService {
    async fn compile(&self, request: CompileRequest) -> Result<CompileOutput, CompilerError> {
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded compiler requests mutex should not be poisoned");
            recorded_requests.push(request);
        }

        Ok(CompileOutput {
            code_hash: self.code_hash.clone(),
        })
    }
}
