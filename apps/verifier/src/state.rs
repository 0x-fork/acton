use std::sync::Arc;

use crate::{
    blockchain::{BlockchainClient, ToncenterClient},
    compilers::{CompilerService, NodeCompilerService},
    config::Config,
    registry::{RegistryClient, TonRegistryClient},
    source_storage::{GitSourceStorage, SourceStorage},
    verification::VerificationService,
};

#[derive(Clone)]
pub struct AppState {
    compiler_service: Arc<dyn CompilerService>,
    registry_client: Arc<dyn RegistryClient>,
    source_storage: Arc<dyn SourceStorage>,
    verification_service: VerificationService,
}

impl AppState {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self::new(
            Arc::new(ToncenterClient::from_config(config)),
            Arc::new(NodeCompilerService::from_config(config)),
            Arc::new(TonRegistryClient::from_config(config)),
            Arc::new(GitSourceStorage::from_config(config)),
        )
    }

    #[must_use]
    pub fn new(
        blockchain_client: Arc<dyn BlockchainClient>,
        compiler_service: Arc<dyn CompilerService>,
        registry_client: Arc<dyn RegistryClient>,
        source_storage: Arc<dyn SourceStorage>,
    ) -> Self {
        Self {
            compiler_service,
            registry_client,
            source_storage,
            verification_service: VerificationService::new(blockchain_client),
        }
    }

    #[must_use]
    pub fn compiler_service(&self) -> &dyn CompilerService {
        self.compiler_service.as_ref()
    }

    #[must_use]
    pub fn registry_client(&self) -> &dyn RegistryClient {
        self.registry_client.as_ref()
    }

    #[must_use]
    pub fn source_storage(&self) -> &dyn SourceStorage {
        self.source_storage.as_ref()
    }

    #[must_use]
    pub const fn verification_service(&self) -> &VerificationService {
        &self.verification_service
    }
}
