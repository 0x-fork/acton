use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use verifier::registry::{
    RegisterBundleRequest, RegistrationReceipt, RegistryClient, RegistryError,
};

pub struct MockRegistryClient {
    outcome: MockRegistryOutcome,
    recorded_requests: Arc<Mutex<Vec<RegisterBundleRequest>>>,
}

impl MockRegistryClient {
    pub fn confirmed() -> Self {
        Self {
            outcome: MockRegistryOutcome::Confirmed(RegistrationReceipt {
                master_address: "mock-master".to_owned(),
                verification_record_address: "mock-record".to_owned(),
            }),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: MockRegistryOutcome::Failed(message.to_owned()),
            recorded_requests: Arc::new(Mutex::new(Vec::new())),
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
        {
            let mut recorded_requests = self
                .recorded_requests
                .lock()
                .expect("recorded registry requests mutex should not be poisoned");
            recorded_requests.push(request);
        }

        match &self.outcome {
            MockRegistryOutcome::Confirmed(receipt) => Ok(receipt.clone()),
            MockRegistryOutcome::Failed(message) => Err(RegistryError::Operation(message.clone())),
        }
    }
}

enum MockRegistryOutcome {
    Confirmed(RegistrationReceipt),
    Failed(String),
}
