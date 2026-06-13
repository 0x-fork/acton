use std::sync::Arc;

use crate::{
    blockchain::{BlockchainClient, ToncenterClient},
    compilers::{CompilerService, NodeCompilerService},
    config::Config,
    verification::VerificationService,
};

#[derive(Clone)]
pub struct AppState {
    compiler_service: Arc<dyn CompilerService>,
    verification_service: VerificationService,
}

impl AppState {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self::new(
            Arc::new(ToncenterClient::from_config(config)),
            Arc::new(NodeCompilerService::from_config(config)),
        )
    }

    #[must_use]
    pub fn new(
        blockchain_client: Arc<dyn BlockchainClient>,
        compiler_service: Arc<dyn CompilerService>,
    ) -> Self {
        Self {
            compiler_service,
            verification_service: VerificationService::new(blockchain_client),
        }
    }

    #[must_use]
    pub fn compiler_service(&self) -> &dyn CompilerService {
        self.compiler_service.as_ref()
    }

    #[must_use]
    pub const fn verification_service(&self) -> &VerificationService {
        &self.verification_service
    }
}
