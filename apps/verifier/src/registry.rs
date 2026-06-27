use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    bundle_validation::{StoredBundleValidationError, validate_stored_bundle},
    registry_index::{SharedVerificationIndex, VerificationIndexError},
    source_storage::{
        SharedSourceStorage, SourceStorageError, SourceStorageReceipt, StoreSourceBundleRequest,
        StoredSourceBundle,
    },
};

#[async_trait]
pub trait VerificationRegistry: Send + Sync + 'static {
    async fn ensure_current(&self) -> Result<(), RegistryError>;

    async fn store_verified_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<StoreVerifiedBundleReceipt, RegistryError>;

    async fn status(
        &self,
        request: VerificationStatusRequest,
    ) -> Result<VerificationStatusReceipt, RegistryError>;

    async fn verified_bundles(
        &self,
        request: VerifiedBundlesRequest,
    ) -> Result<VerifiedBundlesReceipt, RegistryError>;
}

pub type SharedVerificationRegistry = Arc<dyn VerificationRegistry>;

#[derive(Clone, Debug)]
pub struct StoreVerifiedBundleReceipt {
    pub storage: SourceStorageReceipt,
}

#[derive(Clone, Debug)]
pub struct VerificationStatusRequest {
    pub code_hash: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedBundlesRequest {
    pub code_hash: String,
}

#[derive(Clone, Debug)]
pub struct VerificationStatusReceipt {
    pub verified: bool,
    pub bundle_count: usize,
}

#[derive(Clone, Debug)]
pub struct VerifiedBundlesReceipt {
    pub bundles: Vec<StoredSourceBundle>,
}

#[derive(Clone)]
pub struct SourceVerificationRegistry {
    source_storage: SharedSourceStorage,
    verification_index: SharedVerificationIndex,
}

impl SourceVerificationRegistry {
    #[must_use]
    pub fn new(
        source_storage: SharedSourceStorage,
        verification_index: SharedVerificationIndex,
    ) -> Self {
        Self {
            source_storage,
            verification_index,
        }
    }

    async fn load_stored_bundle(
        &self,
        code_hash: &str,
        source_bundle_hash: &str,
    ) -> Result<StoredSourceBundle, RegistryError> {
        for bundle in self.source_storage.list_bundles(code_hash).await? {
            validate_stored_bundle(&bundle, code_hash)?;
            if bundle.manifest.source_bundle_hash == source_bundle_hash {
                return Ok(bundle);
            }
        }

        Err(RegistryError::StoredBundleNotFound {
            code_hash: code_hash.to_owned(),
            source_bundle_hash: source_bundle_hash.to_owned(),
        })
    }
}

#[async_trait]
impl VerificationRegistry for SourceVerificationRegistry {
    async fn ensure_current(&self) -> Result<(), RegistryError> {
        self.verification_index
            .ensure_current(self.source_storage.as_ref())
            .await?;
        Ok(())
    }

    async fn store_verified_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<StoreVerifiedBundleReceipt, RegistryError> {
        self.ensure_current().await?;
        let code_hash = request.code_hash.clone();
        let source_bundle_hash = request.source_bundle_hash.clone();
        let storage = self.source_storage.store_bundle(request).await?;
        let bundle = self
            .load_stored_bundle(&code_hash, &source_bundle_hash)
            .await?;
        let current_revision = self.source_storage.current_revision().await?;
        self.verification_index
            .upsert_bundle(&bundle, current_revision.as_deref())
            .await?;

        Ok(StoreVerifiedBundleReceipt { storage })
    }

    async fn status(
        &self,
        request: VerificationStatusRequest,
    ) -> Result<VerificationStatusReceipt, RegistryError> {
        self.ensure_current().await?;
        let status = self.verification_index.status(&request.code_hash).await?;

        Ok(VerificationStatusReceipt {
            verified: status.verified,
            bundle_count: status.bundle_count,
        })
    }

    async fn verified_bundles(
        &self,
        request: VerifiedBundlesRequest,
    ) -> Result<VerifiedBundlesReceipt, RegistryError> {
        self.ensure_current().await?;
        Ok(VerifiedBundlesReceipt {
            bundles: self.verification_index.bundles(&request.code_hash).await?,
        })
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error(transparent)]
    SourceStorage(#[from] SourceStorageError),
    #[error(transparent)]
    VerificationIndex(#[from] VerificationIndexError),
    #[error("stored bundle {source_bundle_hash} for code hash {code_hash} could not be indexed")]
    StoredBundleNotFound {
        code_hash: String,
        source_bundle_hash: String,
    },
    #[error(transparent)]
    BundleValidation(#[from] StoredBundleValidationError),
}
