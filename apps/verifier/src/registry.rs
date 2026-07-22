use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    bundle_validation::{StoredBundleValidationError, validate_stored_bundle},
    registry_index::{
        IndexedAbiContract, IndexedAbiContractsQuery, IndexedVerifiedBundleSummary,
        SharedVerificationIndex, VerificationIndexError,
    },
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

    async fn verified_bundle(
        &self,
        request: VerifiedBundleRequest,
    ) -> Result<VerifiedBundleReceipt, RegistryError>;

    async fn last_verified(
        &self,
        request: LastVerifiedRequest,
    ) -> Result<LastVerifiedReceipt, RegistryError>;

    async fn abi_contracts(
        &self,
        request: AbiContractsRequest,
    ) -> Result<AbiContractsReceipt, RegistryError>;
}

pub type SharedVerificationRegistry = Arc<dyn VerificationRegistry>;

#[derive(Clone, Debug)]
pub struct StoreVerifiedBundleReceipt {
    pub storage: SourceStorageReceipt,
    pub bundle: StoredSourceBundle,
}

#[derive(Clone, Debug)]
pub struct VerificationStatusRequest {
    pub code_hash: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedBundleRequest {
    pub code_hash: String,
}

#[derive(Clone, Debug)]
pub struct LastVerifiedRequest {
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct AbiContractsRequest {
    pub code_hash: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct VerificationStatusReceipt {
    pub verified: bool,
}

#[derive(Clone, Debug)]
pub struct VerifiedBundleReceipt {
    pub bundle: Option<StoredSourceBundle>,
}

#[derive(Clone, Debug)]
pub struct LastVerifiedReceipt {
    pub items: Vec<IndexedVerifiedBundleSummary>,
    pub total: usize,
}

#[derive(Clone, Debug)]
pub struct AbiContractsReceipt {
    pub items: Vec<IndexedAbiContract>,
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
    ) -> Result<StoredSourceBundle, RegistryError> {
        let bundle = self
            .source_storage
            .load_bundle(code_hash)
            .await?
            .ok_or_else(|| RegistryError::StoredBundleNotFound {
                code_hash: code_hash.to_owned(),
            })?;
        validate_stored_bundle(&bundle, code_hash)?;
        Ok(bundle)
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
        let storage = self.source_storage.store_bundle(request).await?;
        let bundle = self.load_stored_bundle(&code_hash).await?;
        let current_revision = self.source_storage.current_revision().await?;
        self.verification_index
            .upsert_bundle(&bundle, current_revision.as_deref())
            .await?;

        Ok(StoreVerifiedBundleReceipt { storage, bundle })
    }

    async fn status(
        &self,
        request: VerificationStatusRequest,
    ) -> Result<VerificationStatusReceipt, RegistryError> {
        self.ensure_current().await?;
        let status = self.verification_index.status(&request.code_hash).await?;

        Ok(VerificationStatusReceipt {
            verified: status.verified,
        })
    }

    async fn verified_bundle(
        &self,
        request: VerifiedBundleRequest,
    ) -> Result<VerifiedBundleReceipt, RegistryError> {
        self.ensure_current().await?;
        Ok(VerifiedBundleReceipt {
            bundle: self.verification_index.bundle(&request.code_hash).await?,
        })
    }

    async fn last_verified(
        &self,
        request: LastVerifiedRequest,
    ) -> Result<LastVerifiedReceipt, RegistryError> {
        self.ensure_current().await?;
        let page = self
            .verification_index
            .last_verified(request.limit, request.offset)
            .await?;

        Ok(LastVerifiedReceipt {
            items: page.items,
            total: page.total,
        })
    }

    async fn abi_contracts(
        &self,
        request: AbiContractsRequest,
    ) -> Result<AbiContractsReceipt, RegistryError> {
        self.ensure_current().await?;
        let page = self
            .verification_index
            .abi_contracts(IndexedAbiContractsQuery {
                code_hash: request.code_hash,
                limit: request.limit,
                offset: request.offset,
            })
            .await?;

        Ok(AbiContractsReceipt { items: page.items })
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error(transparent)]
    SourceStorage(#[from] SourceStorageError),
    #[error(transparent)]
    VerificationIndex(#[from] VerificationIndexError),
    #[error("stored bundle for code hash {code_hash} could not be indexed")]
    StoredBundleNotFound { code_hash: String },
    #[error(transparent)]
    BundleValidation(#[from] StoredBundleValidationError),
}
