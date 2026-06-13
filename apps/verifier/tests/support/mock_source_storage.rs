use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use verifier::source_storage::{
    SourceBundleManifest, SourceBundleManifestFile, SourceBundleManifestSource, SourceStorage,
    SourceStorageError, SourceStorageProvider, SourceStorageReceipt, StoreSourceBundleRequest,
    StoredSourceBundle, StoredSourceFile,
};

pub struct MockSourceStorage {
    outcome: MockSourceStorageOutcome,
    recorded_requests: Arc<Mutex<Vec<RecordedSourceStorageRequest>>>,
    stored_bundles: Arc<Mutex<Vec<StoredSourceBundle>>>,
}

impl MockSourceStorage {
    pub fn confirmed() -> Self {
        Self {
            outcome: MockSourceStorageOutcome::Confirmed(SourceStorageReceipt {
                provider: SourceStorageProvider::Mock,
                commit: "mock-commit".to_owned(),
                bundle_path: "sources/mock-code-hash/mock-source-bundle-hash".to_owned(),
            }),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            stored_bundles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: MockSourceStorageOutcome::Failed(message.to_owned()),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            stored_bundles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn recorded_requests(&self) -> Arc<Mutex<Vec<RecordedSourceStorageRequest>>> {
        Arc::clone(&self.recorded_requests)
    }
}

#[async_trait]
impl SourceStorage for MockSourceStorage {
    async fn store_bundle(
        &self,
        request: StoreSourceBundleRequest,
    ) -> Result<SourceStorageReceipt, SourceStorageError> {
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded source storage requests mutex should not be poisoned");
            recorded_requests.push(RecordedSourceStorageRequest::from_request(&request));
        }

        match &self.outcome {
            MockSourceStorageOutcome::Confirmed(receipt) => {
                self.stored_bundles
                    .lock()
                    .expect("stored source bundles mutex should not be poisoned")
                    .push(stored_bundle_from_request(&request, receipt));
                Ok(receipt.clone())
            }
            MockSourceStorageOutcome::Failed(message) => {
                Err(SourceStorageError::Operation(message.clone()))
            }
        }
    }

    async fn list_bundles(
        &self,
        code_hash: &str,
    ) -> Result<Vec<StoredSourceBundle>, SourceStorageError> {
        let stored_bundles = self
            .stored_bundles
            .lock()
            .expect("stored source bundles mutex should not be poisoned");
        Ok(stored_bundles
            .iter()
            .filter(|bundle| bundle.manifest.code_hash == code_hash)
            .cloned()
            .collect())
    }
}

#[derive(Clone)]
pub struct RecordedSourceStorageRequest {
    pub code_hash: String,
    pub source_bundle_hash: String,
    pub files: Vec<(String, Vec<u8>)>,
}

impl RecordedSourceStorageRequest {
    fn from_request(request: &StoreSourceBundleRequest) -> Self {
        Self {
            code_hash: request.code_hash.clone(),
            source_bundle_hash: request.source_bundle_hash.clone(),
            files: request
                .files
                .iter()
                .map(|file| (file.path.clone(), file.content.clone()))
                .collect(),
        }
    }
}

enum MockSourceStorageOutcome {
    Confirmed(SourceStorageReceipt),
    Failed(String),
}

fn stored_bundle_from_request(
    request: &StoreSourceBundleRequest,
    receipt: &SourceStorageReceipt,
) -> StoredSourceBundle {
    let mut files = request
        .files
        .iter()
        .map(|file| {
            let sha256 = hex::encode(Sha256::digest(&file.content));
            StoredSourceFile {
                path: file.path.clone(),
                sha256,
                content_base64: STANDARD.encode(&file.content),
                content_text: String::from_utf8(file.content.clone()).ok(),
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let mut manifest_files = files
        .iter()
        .map(|file| SourceBundleManifestFile {
            path: file.path.clone(),
            sha256: file.sha256.clone(),
        })
        .collect::<Vec<_>>();
    manifest_files.sort_by(|left, right| left.path.cmp(&right.path));

    let mut sources = request
        .sources
        .iter()
        .map(|source| SourceBundleManifestSource {
            path: source.path.clone(),
            is_entrypoint: source.is_entrypoint,
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.path.cmp(&right.path));

    StoredSourceBundle {
        commit: Some(receipt.commit.clone()),
        manifest: SourceBundleManifest {
            schema_version: 1,
            address: request.address.clone(),
            code_hash: request.code_hash.clone(),
            source_bundle_hash: request.source_bundle_hash.clone(),
            language: request.language.clone(),
            compiler_version: request.compiler_version.clone(),
            entrypoint: request.entrypoint.clone(),
            compile_params: request.compile_params.clone(),
            bundle_path: receipt.bundle_path.clone(),
            sources,
            files: manifest_files,
        },
        files,
    }
}
