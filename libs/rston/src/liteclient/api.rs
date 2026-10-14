//! Typed queries anchored to an explicit trusted or verified block.

use std::time::Instant;

use super::{LiteClient, LiteClientPool, LiteError, ProofError, proof};
use crate::boc::Boc;
use crate::cell::{Cell, CellBuilder, HashBytes};
use crate::models::vm::Tuple;
use crate::models::{
    Account, BlockId, BlockchainConfig, MsgInfo, OwnedMessage, StdAddr, Transaction,
};
use crate::tl::common::{AccountId, Int256};
use crate::tl::request::{self, Request};
use crate::tl::response;

/// A block accepted as a trust anchor or authenticated by a proof chain.
///
/// The identifier is immutable. It authenticates data at that block, without
/// asserting that the block is recent or that the checkpoint belongs to a
/// particular network. The caller owns checkpoint selection and persistence.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VerifiedBlock(BlockId);

impl VerifiedBlock {
    /// Establish an independently trusted checkpoint without contacting a server.
    ///
    /// This is an explicit trust decision, not cryptographic verification.
    /// Obtain the complete identifier from a trusted source. Never use an
    /// arbitrary server response here to bootstrap a trustless client.
    pub const fn from_trusted(id: BlockId) -> Self {
        Self(id)
    }

    /// Read the complete identifier without allowing its hashes to be modified.
    pub const fn id(&self) -> &BlockId {
        &self.0
    }

    pub(super) const fn verified(id: BlockId) -> Self {
        Self(id)
    }
}

/// Account state and transaction cursor authenticated at a requested block.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Block against which the account's inclusion or absence was checked.
    pub block: BlockId,
    /// Shard block bound to the requested masterchain block, or that block itself.
    pub shard_block: VerifiedBlock,
    /// Decoded state. `None` means authenticated absence from the accounts dictionary.
    pub account: Option<Account>,
    /// Logical time from the authenticated shard-account entry; zero for absence.
    pub last_transaction_lt: u64,
    /// Hash from the authenticated shard-account entry; zero for absence.
    pub last_transaction_hash: HashBytes,
}

/// Server execution output whose source account state has been authenticated.
///
/// The stack and exit code remain server claims: Merkle proofs authenticate
/// input state, not execution of TVM. Recompute locally to authenticate results.
#[derive(Debug, Clone)]
pub struct GetMethodResult {
    /// Block used for the account-state proof.
    pub block: BlockId,
    /// Authenticated shard block containing the account.
    pub shard_block: VerifiedBlock,
    /// Account state hash matched by the server's state proof.
    pub account_state_hash: HashBytes,
    /// Server-reported successful TVM exit code, either zero or one.
    pub exit_code: i32,
    /// Decoded server result in bottom-to-top stack order.
    pub stack: Tuple,
}

/// TVM method selector, including numeric selectors for nonstandard methods.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MethodId(u64);

impl MethodId {
    /// Use the exact LiteAPI method identifier supplied by the caller.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Compute TON's get-method selector from the method name's UTF-8 bytes.
    pub fn from_name(name: &str) -> Self {
        Self(u64::from(crate::crc::crc_16(name.as_bytes())) | 0x10000)
    }

    /// Return the selector encoded in the LiteAPI request.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<&str> for MethodId {
    fn from(name: &str) -> Self {
        Self::from_name(name)
    }
}

impl From<u64> for MethodId {
    fn from(id: u64) -> Self {
        Self::new(id)
    }
}

impl LiteClient {
    /// Authenticate the server's current masterchain head from a checkpoint.
    /// Freshness is a server claim; the caller decides acceptable age and seqno.
    pub async fn get_masterchain_info(
        &mut self,
        checkpoint: &VerifiedBlock,
    ) -> Result<VerifiedBlock, LiteError> {
        let response: response::MasterchainInfo =
            self.query_typed(Request::GetMasterchainInfo).await?;
        let target = proof::block_id(&response.last)?;
        self.verify_proof_chain(checkpoint, &target).await
    }

    /// Decode and authenticate account state or absence at this exact block.
    /// Anycast addresses are rejected; proofs must identify the exact account.
    pub async fn get_account(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<AccountInfo, LiteError> {
        let account = account_id(address)?;
        let response: response::AccountState = self
            .query_typed(Request::GetAccountState(request::GetAccountState {
                id: proof::wire_id(block.id()),
                account,
            }))
            .await?;
        Ok(response.verify(block, address)?)
    }

    /// Authenticate configuration at a masterchain block. An empty list requests
    /// the complete config; otherwise only requested parameters are returned.
    /// Missing requested parameters and pruned requested data are errors.
    pub async fn get_config(
        &mut self,
        block: &VerifiedBlock,
        parameters: &[i32],
    ) -> Result<BlockchainConfig, LiteError> {
        proof::require_masterchain(block.id())?;
        let id = proof::wire_id(block.id());
        let query = if parameters.is_empty() {
            Request::GetConfigAll(request::GetConfigAll {
                mode: (),
                id,
                with_state_root: None,
                with_libraries: None,
                with_state_extra_root: None,
                with_shard_hashes: None,
                with_validator_set: None,
                with_special_smc: None,
                with_accounts_root: None,
                with_prev_blocks: None,
                with_workchain_info: None,
                with_capabilities: None,
                extract_from_key_block: None,
            })
        } else {
            Request::GetConfigParams(request::GetConfigParams {
                mode: (),
                id,
                param_list: parameters.to_vec(),
                with_state_root: None,
                with_libraries: None,
                with_state_extra_root: None,
                with_shard_hashes: None,
                with_validator_set: None,
                with_special_smc: None,
                with_accounts_root: None,
                with_prev_blocks: None,
                with_workchain_info: None,
                with_capabilities: None,
                extract_from_key_block: None,
            })
        };
        let response: response::ConfigInfo = self.query_typed(query).await?;
        Ok(response.verify(block, parameters)?)
    }

    /// Authenticate one transaction's address, logical time, and inclusion.
    /// `block` must identify its containing shard block, not a masterchain head.
    pub async fn get_transaction(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        lt: u64,
    ) -> Result<Transaction, LiteError> {
        let account = account_id(address)?;
        if !block.id().shard.contains_address(address) {
            return Err(
                ProofError::Invalid("transaction address is outside the requested shard").into(),
            );
        }
        let response: response::TransactionInfo = self
            .query_typed(Request::GetOneTransaction(request::GetOneTransaction {
                id: proof::wire_id(block.id()),
                account,
                lt,
            }))
            .await?;
        Ok(response.verify(block, address, lt)?)
    }

    /// Retrieve a partial masterchain proof without authenticating its links.
    /// Use [`response::PartialBlockProof::verify`] or [`Self::verify_proof_chain`]
    /// before promoting its destination to a verified block.
    pub async fn get_block_proof(
        &mut self,
        known: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<response::PartialBlockProof, LiteError> {
        proof::require_masterchain(known.id())?;
        proof::require_masterchain(target)?;
        let response: response::PartialBlockProof = self
            .query_typed(Request::GetBlockProof(request::GetBlockProof {
                mode: (),
                known_block: proof::wire_id(known.id()),
                target_block: Some(proof::wire_id(target)),
                allow_weak_target: None,
                base_block_from_request: None,
            }))
            .await?;
        if proof::block_id(&response.from)? != *known.id() {
            return Err(ProofError::Invalid("proof starts from a different block").into());
        }
        Ok(response)
    }

    /// Authenticate a masterchain block through forward and backward proof links.
    ///
    /// The request deadline covers the whole operation, including all queries.
    /// Synchronous proof batches finish before an expired deadline is reported.
    /// At most 64 partial responses and 1024 links are accepted. The checkpoint
    /// is never changed on failure or cancellation; persist the returned block
    /// yourself. Requests are not retried and no first-response trust is used.
    pub async fn verify_proof_chain(
        &mut self,
        checkpoint: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<VerifiedBlock, LiteError> {
        proof::require_masterchain(checkpoint.id())?;
        proof::require_masterchain(target)?;
        let started = Instant::now();
        let timeout = self.request_timeout;
        log::debug!(
            "operation=verify_proof_chain node={} target={target} outcome=started",
            self.target
        );

        let result = tokio::time::timeout(timeout, async {
            let mut current = *checkpoint;
            let mut links = 0usize;
            for _ in 0..64 {
                if current.id() == target {
                    return Ok(current);
                }
                let part = self.get_block_proof(&current, target).await?;
                links += part.steps.len();
                if links > 1024 {
                    return Err(ProofError::Invalid("proof chain exceeds 1024 links").into());
                }
                current = part.verify(&current, target)?;
                if started.elapsed() >= timeout {
                    return Err(LiteError::Timeout {
                        operation: "verify_proof_chain",
                        timeout,
                    });
                }
                log::debug!(
                    "operation=verify_proof_chain node={} target={target} seqno={} links={links} duration_ms={} outcome=progress",
                    self.target,
                    current.id().seqno,
                    started.elapsed().as_millis(),
                );
            }
            Err(LiteError::from(ProofError::Invalid(
                "proof chain exceeds 64 responses",
            )))
        })
        .await
        .map_err(|_| LiteError::Timeout {
            operation: "verify_proof_chain",
            timeout,
        })
        .and_then(|result| result);

        log::debug!(
            "operation=verify_proof_chain node={} target={target} duration_ms={} outcome={}",
            self.target,
            started.elapsed().as_millis(),
            if result.is_ok() { "success" } else { "failure" }
        );
        result
    }

    /// Execute a get method remotely, authenticating its source account state.
    /// The returned stack and exit code remain unverified server computation.
    /// Arguments and results use bottom-to-top stack order. Nonzero/non-one
    /// exit codes are returned as [`LiteError::ContractExecution`].
    pub async fn run_get_method(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        method: impl Into<MethodId>,
        arguments: &Tuple,
    ) -> Result<GetMethodResult, LiteError> {
        let account = account_id(address)?;
        let params = CellBuilder::build_from(arguments).map_err(ProofError::from)?;
        let response: response::RunMethodResult = self
            .query_typed(Request::RunSmcMethod(request::RunSmcMethod {
                mode: 7,
                id: proof::wire_id(block.id()),
                account,
                method_id: method.into().value(),
                params: Boc::encode(params),
            }))
            .await
            .map_err(|error| match error {
                LiteError::ServerError(error) if error.code == -256 => {
                    LiteError::ContractExecution { exit_code: -256 }
                }
                error => error,
            })?;
        Ok(response.verify(block, address)?)
    }

    /// Submit one external incoming message, returning its representation hash
    /// after the server acknowledges receipt. This proves neither propagation
    /// nor on-chain inclusion. Cancellation and timeout leave acceptance unknown.
    /// The exact message is sent once, with no automatic retry or re-signing.
    pub async fn send_message(&mut self, message: &Cell) -> Result<HashBytes, LiteError> {
        let decoded = message.parse::<OwnedMessage>().map_err(ProofError::from)?;
        if !matches!(decoded.info, MsgInfo::ExtIn(_)) {
            return Err(
                ProofError::Invalid("only external incoming messages can be submitted").into(),
            );
        }
        let response: response::SendMsgStatus = self
            .query_typed(Request::SendMessage(request::SendMessage {
                body: Boc::encode(message),
            }))
            .await?;
        if response.status != 1 {
            return Err(LiteError::MessageRejected {
                status: response.status,
            });
        }
        Ok(*message.repr_hash())
    }
}

impl LiteClientPool {
    /// Authenticate the selected server's head from an explicit checkpoint.
    /// Connection setup is bounded separately from proof-chain verification.
    pub async fn get_masterchain_info(
        &mut self,
        checkpoint: &VerifiedBlock,
    ) -> Result<VerifiedBlock, LiteError> {
        self.next_client()
            .await?
            .get_masterchain_info(checkpoint)
            .await
    }

    /// Authenticate account state at an exact block through the next reachable server.
    pub async fn get_account(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<AccountInfo, LiteError> {
        self.next_client().await?.get_account(block, address).await
    }

    /// Authenticate requested masterchain configuration through the next reachable server.
    pub async fn get_config(
        &mut self,
        block: &VerifiedBlock,
        parameters: &[i32],
    ) -> Result<BlockchainConfig, LiteError> {
        self.next_client()
            .await?
            .get_config(block, parameters)
            .await
    }

    /// Authenticate transaction inclusion in an exact shard block.
    pub async fn get_transaction(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        lt: u64,
    ) -> Result<Transaction, LiteError> {
        self.next_client()
            .await?
            .get_transaction(block, address, lt)
            .await
    }

    /// Fetch unverified proof links; receiving them does not authenticate a block.
    pub async fn get_block_proof(
        &mut self,
        known: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<response::PartialBlockProof, LiteError> {
        self.next_client()
            .await?
            .get_block_proof(known, target)
            .await
    }

    /// Authenticate all links through one selected connection, without retries.
    /// The limits and timeout of [`LiteClient::verify_proof_chain`] apply.
    pub async fn verify_proof_chain(
        &mut self,
        checkpoint: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<VerifiedBlock, LiteError> {
        self.next_client()
            .await?
            .verify_proof_chain(checkpoint, target)
            .await
    }

    /// Authenticate source state for a remote get-method result; execution remains a server claim.
    pub async fn run_get_method(
        &mut self,
        block: &VerifiedBlock,
        address: &StdAddr,
        method: impl Into<MethodId>,
        arguments: &Tuple,
    ) -> Result<GetMethodResult, LiteError> {
        self.next_client()
            .await?
            .run_get_method(block, address, method, arguments)
            .await
    }

    /// Submit once through the next reachable server. Receipt does not confirm inclusion.
    pub async fn send_message(&mut self, message: &Cell) -> Result<HashBytes, LiteError> {
        self.next_client().await?.send_message(message).await
    }
}

fn account_id(address: &StdAddr) -> Result<AccountId, ProofError> {
    if address.anycast.is_some() {
        return Err(ProofError::Invalid(
            "anycast addresses are not supported by account proofs",
        ));
    }
    Ok(AccountId {
        workchain: i32::from(address.workchain),
        id: Int256(address.address.0),
    })
}
