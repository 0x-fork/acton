use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use verifier::registry::{
    RegisterBundleRequest, RegistrationReceipt, RegistryClient, RegistryError,
    SourceBundleStatusReceipt, SourceBundleStatusRequest, VerificationStatusReceipt,
    VerificationStatusRequest,
};

pub struct MockRegistryClient {
    outcome: MockRegistryOutcome,
    recorded_requests: Arc<Mutex<Vec<RegisterBundleRequest>>>,
    registered_bundles: Arc<Mutex<BTreeSet<(String, String)>>>,
}

impl MockRegistryClient {
    pub fn confirmed() -> Self {
        Self {
            outcome: MockRegistryOutcome::Confirmed {
                receipt: RegistrationReceipt {
                    master_address: "mock-master".to_owned(),
                    verification_record_address: "mock-record".to_owned(),
                },
                initially_verified: true,
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            registered_bundles: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn unverified() -> Self {
        Self {
            outcome: MockRegistryOutcome::Confirmed {
                receipt: RegistrationReceipt {
                    master_address: "mock-master".to_owned(),
                    verification_record_address: "mock-record".to_owned(),
                },
                initially_verified: false,
            },
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            registered_bundles: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: MockRegistryOutcome::Failed(message.to_owned()),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
            registered_bundles: Arc::new(Mutex::new(BTreeSet::new())),
        }
    }

    pub fn recorded_requests(&self) -> Arc<Mutex<Vec<RegisterBundleRequest>>> {
        Arc::clone(&self.recorded_requests)
    }
}

#[async_trait]
impl RegistryClient for MockRegistryClient {
    async fn register_bundle(
        &self,
        request: RegisterBundleRequest,
    ) -> Result<RegistrationReceipt, RegistryError> {
        let request_snapshot = request.clone();
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded registry requests mutex should not be poisoned");
            recorded_requests.push(request);
        }

        match &self.outcome {
            MockRegistryOutcome::Confirmed { receipt, .. } => {
                self.registered_bundles
                    .lock()
                    .expect("registered bundles mutex should not be poisoned")
                    .insert((
                        request_snapshot.code_hash,
                        request_snapshot.source_bundle_hash,
                    ));
                Ok(receipt.clone())
            }
            MockRegistryOutcome::Failed(message) => Err(RegistryError::Operation(message.clone())),
        }
    }

    async fn verification_status(
        &self,
        request: VerificationStatusRequest,
    ) -> Result<VerificationStatusReceipt, RegistryError> {
        match &self.outcome {
            MockRegistryOutcome::Confirmed {
                receipt,
                initially_verified,
            } => {
                let has_registered_bundle = {
                    let registered_bundles = self
                        .registered_bundles
                        .lock()
                        .expect("registered bundles mutex should not be poisoned");
                    let has_registered_bundle = registered_bundles
                        .iter()
                        .any(|(code_hash, _)| code_hash == &request.code_hash);
                    drop(registered_bundles);
                    has_registered_bundle
                };
                let verified = *initially_verified || has_registered_bundle;
                Ok(VerificationStatusReceipt {
                    master_address: receipt.master_address.clone(),
                    verification_record_address: receipt.verification_record_address.clone(),
                    verified,
                })
            }
            MockRegistryOutcome::Failed(message) => Err(RegistryError::Operation(message.clone())),
        }
    }

    async fn source_bundle_status(
        &self,
        request: SourceBundleStatusRequest,
    ) -> Result<SourceBundleStatusReceipt, RegistryError> {
        match &self.outcome {
            MockRegistryOutcome::Confirmed {
                initially_verified, ..
            } => {
                let has_registered_bundle = {
                    let registered_bundles = self
                        .registered_bundles
                        .lock()
                        .expect("registered bundles mutex should not be poisoned");
                    let has_registered_bundle = registered_bundles
                        .contains(&(request.code_hash, request.source_bundle_hash));
                    drop(registered_bundles);
                    has_registered_bundle
                };
                let verified = *initially_verified || has_registered_bundle;
                Ok(SourceBundleStatusReceipt { verified })
            }
            MockRegistryOutcome::Failed(message) => Err(RegistryError::Operation(message.clone())),
        }
    }
}

enum MockRegistryOutcome {
    Confirmed {
        receipt: RegistrationReceipt,
        initially_verified: bool,
    },
    Failed(String),
}
