use async_trait::async_trait;
use verifier::compilers::{CompileOutput, CompileRequest, CompilerError, CompilerService};

pub struct MockCompilerService {
    code_hash: String,
}

impl MockCompilerService {
    pub fn new(code_hash: &str) -> Self {
        Self {
            code_hash: code_hash.to_owned(),
        }
    }
}

#[async_trait]
impl CompilerService for MockCompilerService {
    async fn compile(&self, _request: CompileRequest) -> Result<CompileOutput, CompilerError> {
        Ok(CompileOutput {
            code_hash: self.code_hash.clone(),
        })
    }
}
