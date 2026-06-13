use std::{
    env, fs, io,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::http::StatusCode;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::time;
use ton::{
    block_tlb::{CommonMsgInfo, CommonMsgInfoInt, CurrencyCollection, Msg},
    ton_core::{
        cell::TonCell,
        traits::tlb::TLB,
        types::{TonAddress, tlb_core::TLBCoins},
    },
    ton_wallet::{
        Mnemonic, TonWallet, WALLET_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT,
        WALLET_V5R1_ID_DEFAULT_TESTNET, WalletVersion,
    },
};

use crate::config::{Config, TonNetwork};

const TONCENTER_API_KEY_HEADER: &str = "X-API-Key";
const REGISTER_BUNDLE_OPCODE: u32 = 0x5645_5201;
const HASH_BYTES: usize = 32;
const WALLET_MESSAGE_TTL: Duration = Duration::from_mins(10);
const UNDEPLOYED_WALLET_SEQNO_SENTINEL: u32 = 85_143;

#[async_trait]
pub trait RegistryClient: Send + Sync + 'static {
    async fn register_bundle(
        &self,
        request: RegisterBundleRequest,
    ) -> Result<RegistrationReceipt, RegistryError>;
}

pub type SharedRegistryClient = Arc<dyn RegistryClient>;

#[derive(Clone, Debug)]
pub struct RegisterBundleRequest {
    pub code_hash: String,
    pub source_bundle_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistrationReceipt {
    pub master_address: String,
    pub verification_record_address: String,
}

#[derive(Clone, Default)]
pub struct DisabledRegistryClient;

#[async_trait]
impl RegistryClient for DisabledRegistryClient {
    async fn register_bundle(
        &self,
        _request: RegisterBundleRequest,
    ) -> Result<RegistrationReceipt, RegistryError> {
        Err(RegistryError::MissingConfig("registry.master_address"))
    }
}

#[derive(Clone)]
pub struct TonRegistryClient {
    http: Client,
    network: TonNetwork,
    base_url: String,
    api_key: Option<String>,
    master_address: Option<String>,
    register_value_nano: u64,
    confirmation_attempts: usize,
    confirmation_delay: Duration,
    wallet_kind: String,
    wallet_workchain: i32,
    wallet_mnemonic_env: Option<String>,
    wallet_mnemonic_file: Option<PathBuf>,
    wallet_mnemonic: Option<String>,
}

impl TonRegistryClient {
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            http: Client::new(),
            network: config.network(),
            base_url: config.toncenter_base_url().to_owned(),
            api_key: config.toncenter_api_key().map(ToOwned::to_owned),
            master_address: config.registry_master_address().map(ToOwned::to_owned),
            register_value_nano: config.registry_register_value_nano(),
            confirmation_attempts: config.registry_confirmation_attempts(),
            confirmation_delay: config.registry_confirmation_delay(),
            wallet_kind: config.wallet_kind().to_owned(),
            wallet_workchain: config.wallet_workchain(),
            wallet_mnemonic_env: config.wallet_mnemonic_env().map(ToOwned::to_owned),
            wallet_mnemonic_file: config.wallet_mnemonic_file().map(ToOwned::to_owned),
            wallet_mnemonic: config.wallet_mnemonic().map(ToOwned::to_owned),
        }
    }

    fn json_rpc_url(&self) -> String {
        format!("{}/api/v2/jsonRPC", self.base_url.trim_end_matches('/'))
    }

    fn send_boc_url(&self) -> String {
        format!("{}/api/v2/sendBoc", self.base_url.trim_end_matches('/'))
    }

    fn master_address(&self) -> Result<TonAddress, RegistryError> {
        let address = self
            .master_address
            .as_deref()
            .ok_or(RegistryError::MissingConfig("registry.master_address"))?;
        TonAddress::from_str(address).map_err(|err| RegistryError::InvalidAddress {
            field: "registry.master_address",
            value: address.to_owned(),
            message: err.to_string(),
        })
    }

    fn wallet(&self) -> Result<TonWallet, RegistryError> {
        let mnemonic = self.resolve_mnemonic()?;
        let mnemonic =
            Mnemonic::from_str(&mnemonic, None).map_err(|err| RegistryError::Wallet {
                message: err.to_string(),
            })?;
        let wallet_version = parse_wallet_version(&self.wallet_kind)?;
        let wallet_id = wallet_id(wallet_version, self.network);

        TonWallet::new_with_params(
            wallet_version,
            mnemonic
                .to_key_pair()
                .map_err(|err| RegistryError::Wallet {
                    message: err.to_string(),
                })?,
            self.wallet_workchain,
            wallet_id,
        )
        .map_err(|err| RegistryError::Wallet {
            message: err.to_string(),
        })
    }

    fn resolve_mnemonic(&self) -> Result<String, RegistryError> {
        let source_count = [
            self.wallet_mnemonic_env.is_some(),
            self.wallet_mnemonic_file.is_some(),
            self.wallet_mnemonic.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        match source_count {
            0 => Err(RegistryError::MissingConfig(
                "wallet.mnemonic_env, wallet.mnemonic_file, or wallet.mnemonic",
            )),
            1 => {
                if let Some(variable) = &self.wallet_mnemonic_env {
                    return env::var(variable).map_err(|source| RegistryError::MnemonicEnv {
                        variable: variable.clone(),
                        source,
                    });
                }

                if let Some(path) = &self.wallet_mnemonic_file {
                    return fs::read_to_string(path).map_err(|source| {
                        RegistryError::MnemonicFile {
                            path: path.clone(),
                            source,
                        }
                    });
                }

                Ok(self
                    .wallet_mnemonic
                    .clone()
                    .expect("source_count ensures inline mnemonic is present"))
            }
            _ => Err(RegistryError::MultipleMnemonicSources),
        }
    }

    async fn wallet_seqno(&self, wallet: &TonWallet) -> Result<(u32, bool), RegistryError> {
        let address = format_ton_address(&wallet.address, self.network, true);
        let Ok(result) = self.run_get_method(&address, "seqno", Vec::new()).await else {
            return Ok((0, true));
        };

        if result.exit_code == -13 {
            return Ok((0, true));
        }

        if result.exit_code != 0 {
            return Err(RegistryError::GetMethodExit {
                method: "seqno",
                exit_code: result.exit_code,
            });
        }

        let seqno = parse_stack_num(result.stack.first(), "seqno")?;
        let seqno = u32::try_from(seqno).map_err(|_| RegistryError::InvalidStack {
            method: "seqno",
            message: format!("seqno is out of u32 range: {seqno}"),
        })?;

        if seqno == UNDEPLOYED_WALLET_SEQNO_SENTINEL {
            return Ok((0, true));
        }

        Ok((seqno, false))
    }

    async fn run_get_method(
        &self,
        address: &str,
        method: &'static str,
        stack: Vec<Value>,
    ) -> Result<GetMethodResult, RegistryError> {
        let params = json!({
            "address": address,
            "method": method,
            "stack": stack,
        });
        let body = json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "runGetMethod",
            "params": params,
        });

        let mut request = self.http.post(self.json_rpc_url()).json(&body);
        if let Some(api_key) = &self.api_key {
            request = request.header(TONCENTER_API_KEY_HEADER, api_key);
        }

        let response = request.send().await.map_err(RegistryError::Transport)?;
        let status = response.status();
        let text = response.text().await.map_err(RegistryError::Transport)?;

        if !status.is_success() {
            return Err(RegistryError::Api { status, body: text });
        }

        let response =
            serde_json::from_str::<JsonRpcResponse>(&text).map_err(RegistryError::Json)?;
        if let Some(result) = response.result {
            return Ok(result);
        }

        Err(RegistryError::JsonRpc {
            method,
            message: response
                .error
                .map_or_else(|| "missing result".to_owned(), |error| error.to_string()),
        })
    }

    async fn send_boc(&self, boc: &str) -> Result<(), RegistryError> {
        let mut request = self
            .http
            .post(self.send_boc_url())
            .json(&json!({ "boc": boc }));
        if let Some(api_key) = &self.api_key {
            request = request.header(TONCENTER_API_KEY_HEADER, api_key);
        }

        let response = request.send().await.map_err(RegistryError::Transport)?;
        let status = response.status();
        let text = response.text().await.map_err(RegistryError::Transport)?;

        if status.is_success() {
            return Ok(());
        }

        Err(RegistryError::Api { status, body: text })
    }

    async fn verification_record_address(
        &self,
        master_address: &TonAddress,
        code_hash: &str,
    ) -> Result<TonAddress, RegistryError> {
        let result = self
            .run_get_method(
                &format_ton_address(master_address, self.network, true),
                "verificationAddress",
                vec![stack_num_u256(code_hash)],
            )
            .await?;

        if result.exit_code != 0 {
            return Err(RegistryError::GetMethodExit {
                method: "verificationAddress",
                exit_code: result.exit_code,
            });
        }

        let address = parse_stack_address(result.stack.first(), "verificationAddress")?;
        Ok(address)
    }

    async fn has_bundle(
        &self,
        record_address: &TonAddress,
        source_bundle_hash: &str,
    ) -> Result<bool, RegistryError> {
        let result = self
            .run_get_method(
                &format_ton_address(record_address, self.network, true),
                "hasBundle",
                vec![stack_num_u256(source_bundle_hash)],
            )
            .await?;

        if result.exit_code == -13 {
            return Ok(false);
        }

        if result.exit_code != 0 {
            return Err(RegistryError::GetMethodExit {
                method: "hasBundle",
                exit_code: result.exit_code,
            });
        }

        parse_stack_bool(result.stack.first(), "hasBundle")
    }

    async fn wait_for_confirmation(
        &self,
        master_address: &TonAddress,
        code_hash: &str,
        source_bundle_hash: &str,
    ) -> Result<TonAddress, RegistryError> {
        let record_address = self
            .verification_record_address(master_address, code_hash)
            .await?;
        let attempts = self.confirmation_attempts.max(1);

        for attempt in 0..attempts {
            if self.has_bundle(&record_address, source_bundle_hash).await? {
                return Ok(record_address);
            }

            if attempt + 1 < attempts {
                time::sleep(self.confirmation_delay).await;
            }
        }

        Err(RegistryError::ConfirmationTimedOut {
            attempts,
            delay: self.confirmation_delay,
        })
    }

    fn register_bundle_message(
        &self,
        wallet: &TonWallet,
        master_address: &TonAddress,
        code_hash: &str,
        source_bundle_hash: &str,
    ) -> Result<TonCell, RegistryError> {
        let code_hash = parse_hash(code_hash, "code_hash")?;
        let source_bundle_hash = parse_hash(source_bundle_hash, "source_bundle_hash")?;

        let mut body = TonCell::builder();
        body.write_num(&REGISTER_BUNDLE_OPCODE, 32)
            .map_err(cell_error)?;
        body.write_bits(code_hash, HASH_BYTES * 8)
            .map_err(cell_error)?;
        body.write_bits(source_bundle_hash, HASH_BYTES * 8)
            .map_err(cell_error)?;
        let body = body.build().map_err(cell_error)?;

        let info = CommonMsgInfoInt {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: wallet.address.to_msg_address(),
            dst: master_address.to_msg_address(),
            value: CurrencyCollection::new(TLBCoins::new(u128::from(self.register_value_nano))),
            ihr_fee: TLBCoins::ZERO,
            fwd_fee: TLBCoins::ZERO,
            created_lt: 0,
            created_at: 0,
        };

        Msg::new(CommonMsgInfo::Int(info), body)
            .to_cell()
            .map_err(cell_error)
    }
}

#[async_trait]
impl RegistryClient for TonRegistryClient {
    async fn register_bundle(
        &self,
        request: RegisterBundleRequest,
    ) -> Result<RegistrationReceipt, RegistryError> {
        let master_address = self.master_address()?;
        let wallet = self.wallet()?;
        let message = self.register_bundle_message(
            &wallet,
            &master_address,
            &request.code_hash,
            &request.source_bundle_hash,
        )?;
        let (seqno, need_state_init) = self.wallet_seqno(&wallet).await?;
        let expire_at = wallet_message_expire_at()?;
        let external = wallet
            .create_ext_in_msg(vec![message], seqno, expire_at, need_state_init)
            .map_err(|err| RegistryError::Wallet {
                message: err.to_string(),
            })?;
        let boc = external.to_boc_base64().map_err(cell_error)?;

        self.send_boc(&boc).await?;
        let record_address = self
            .wait_for_confirmation(
                &master_address,
                &request.code_hash,
                &request.source_bundle_hash,
            )
            .await?;

        Ok(RegistrationReceipt {
            master_address: format_ton_address(&master_address, self.network, true),
            verification_record_address: format_ton_address(&record_address, self.network, true),
        })
    }
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("missing registry configuration: {0}")]
    MissingConfig(&'static str),
    #[error("multiple wallet mnemonic sources configured")]
    MultipleMnemonicSources,
    #[error("failed to read wallet mnemonic environment variable {variable}: {source}")]
    MnemonicEnv {
        variable: String,
        source: env::VarError,
    },
    #[error("failed to read wallet mnemonic file {path}: {source}", path = path.display())]
    MnemonicFile { path: PathBuf, source: io::Error },
    #[error("invalid address in {field}: {value}: {message}")]
    InvalidAddress {
        field: &'static str,
        value: String,
        message: String,
    },
    #[error("invalid hash in {field}: {value}")]
    InvalidHash {
        field: &'static str,
        value: String,
        source: hex::FromHexError,
    },
    #[error("invalid wallet kind: {0}")]
    InvalidWalletKind(String),
    #[error("wallet error: {message}")]
    Wallet { message: String },
    #[error("cell serialization error: {0}")]
    Cell(String),
    #[error("toncenter transport error: {0}")]
    Transport(reqwest::Error),
    #[error("toncenter API error: status={status}, body={body}")]
    Api { status: StatusCode, body: String },
    #[error("toncenter malformed response: {0}")]
    Json(serde_json::Error),
    #[error("toncenter JSON-RPC error in {method}: {message}")]
    JsonRpc {
        method: &'static str,
        message: String,
    },
    #[error("get-method {method} failed with exit code {exit_code}")]
    GetMethodExit {
        method: &'static str,
        exit_code: i32,
    },
    #[error("invalid get-method stack for {method}: {message}")]
    InvalidStack {
        method: &'static str,
        message: String,
    },
    #[error("registration was not confirmed after {attempts} attempts with {delay:?} delay")]
    ConfirmationTimedOut { attempts: usize, delay: Duration },
    #[error("{0}")]
    Operation(String),
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<GetMethodResult>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: Option<i64>,
    message: Option<String>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.code, self.message.as_deref()) {
            (Some(code), Some(message)) => write!(formatter, "code={code}, message={message}"),
            (Some(code), None) => write!(formatter, "code={code}"),
            (None, Some(message)) => formatter.write_str(message),
            (None, None) => formatter.write_str("unknown error"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetMethodResult {
    stack: Vec<Value>,
    exit_code: i32,
}

fn parse_wallet_version(kind: &str) -> Result<WalletVersion, RegistryError> {
    match kind.to_ascii_lowercase().as_str() {
        "v1r1" => Ok(WalletVersion::V1R1),
        "v1r2" => Ok(WalletVersion::V1R2),
        "v1r3" => Ok(WalletVersion::V1R3),
        "v2r1" => Ok(WalletVersion::V2R1),
        "v2r2" => Ok(WalletVersion::V2R2),
        "v3r1" => Ok(WalletVersion::V3R1),
        "v3r2" => Ok(WalletVersion::V3R2),
        "v4r1" => Ok(WalletVersion::V4R1),
        "v4r2" => Ok(WalletVersion::V4R2),
        "v5r1" => Ok(WalletVersion::V5R1),
        "highloadv1r1" => Ok(WalletVersion::HLV1R1),
        "highloadv1r2" => Ok(WalletVersion::HLV1R2),
        "highloadv2" => Ok(WalletVersion::HLV2),
        "highloadv2r1" => Ok(WalletVersion::HLV2R1),
        "highloadv2r2" => Ok(WalletVersion::HLV2R2),
        _ => Err(RegistryError::InvalidWalletKind(kind.to_owned())),
    }
}

const fn wallet_id(wallet: WalletVersion, network: TonNetwork) -> i32 {
    match wallet {
        WalletVersion::V5R1 => {
            if network.uses_testnet_address_format() {
                return WALLET_V5R1_ID_DEFAULT_TESTNET;
            }
            WALLET_V5R1_ID_DEFAULT
        }
        _ => WALLET_ID_DEFAULT,
    }
}

fn wallet_message_expire_at() -> Result<u32, RegistryError> {
    let expire_at = SystemTime::now()
        .checked_add(WALLET_MESSAGE_TTL)
        .ok_or_else(|| RegistryError::Operation("wallet message expiration overflow".to_owned()))?
        .duration_since(UNIX_EPOCH)
        .map_err(|err| RegistryError::Operation(err.to_string()))?
        .as_secs();

    u32::try_from(expire_at).map_err(|_| {
        RegistryError::Operation(format!("wallet expiration is out of range: {expire_at}"))
    })
}

fn format_ton_address(address: &TonAddress, network: TonNetwork, bounceable: bool) -> String {
    address.to_base64(!network.uses_testnet_address_format(), bounceable, true)
}

fn parse_hash(value: &str, field: &'static str) -> Result<[u8; HASH_BYTES], RegistryError> {
    let bytes = hex::decode(value).map_err(|source| RegistryError::InvalidHash {
        field,
        value: value.to_owned(),
        source,
    })?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| RegistryError::InvalidStack {
            method: "registerBundle",
            message: format!("{field} must be {HASH_BYTES} bytes, got {}", bytes.len()),
        })
}

fn stack_num_u256(value: &str) -> Value {
    json!(["num", format!("0x{value}")])
}

fn parse_stack_bool(entry: Option<&Value>, method: &'static str) -> Result<bool, RegistryError> {
    Ok(parse_stack_num(entry, method)? != 0)
}

fn parse_stack_num(entry: Option<&Value>, method: &'static str) -> Result<i128, RegistryError> {
    let (_, value) = stack_type_value(entry, method)?;
    let raw = value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.is_number().then(|| value.to_string()))
        .ok_or_else(|| RegistryError::InvalidStack {
            method,
            message: "number stack value must be a string or number".to_owned(),
        })?;

    parse_int_literal(&raw).map_err(|message| RegistryError::InvalidStack { method, message })
}

fn parse_stack_address(
    entry: Option<&Value>,
    method: &'static str,
) -> Result<TonAddress, RegistryError> {
    let (entry_type, value) = stack_type_value(entry, method)?;
    if entry_type != "slice" && entry_type != "cell" {
        return Err(RegistryError::InvalidStack {
            method,
            message: format!("expected slice or cell stack value, got {entry_type}"),
        });
    }

    let boc = stack_boc_base64(value).ok_or_else(|| RegistryError::InvalidStack {
        method,
        message: "address stack value is missing BoC bytes".to_owned(),
    })?;
    let cell = ton_cell_from_boc_base64(&boc)?;

    TonAddress::from_cell(&cell).map_err(|err| RegistryError::InvalidStack {
        method,
        message: err.to_string(),
    })
}

fn stack_type_value<'a>(
    entry: Option<&'a Value>,
    method: &'static str,
) -> Result<(&'a str, &'a Value), RegistryError> {
    let entry = entry.ok_or_else(|| RegistryError::InvalidStack {
        method,
        message: "empty stack".to_owned(),
    })?;

    if let Some(items) = entry.as_array() {
        let [entry_type, value] = items.as_slice() else {
            return Err(RegistryError::InvalidStack {
                method,
                message: "legacy stack entry must contain exactly two items".to_owned(),
            });
        };

        let entry_type = entry_type
            .as_str()
            .ok_or_else(|| RegistryError::InvalidStack {
                method,
                message: "legacy stack entry type must be a string".to_owned(),
            })?;
        return Ok((entry_type, value));
    }

    let entry_type =
        entry
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| RegistryError::InvalidStack {
                method,
                message: "stack entry must contain type".to_owned(),
            })?;
    let value = entry.get("value").unwrap_or(&Value::Null);

    Ok((entry_type, value))
}

fn stack_boc_base64(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_owned());
    }

    value
        .get("bytes")
        .and_then(Value::as_str)
        .or_else(|| value.get("boc").and_then(Value::as_str))
        .or_else(|| value.get("cell").and_then(Value::as_str))
        .or_else(|| value.get("slice").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn ton_cell_from_boc_base64(value: &str) -> Result<TonCell, RegistryError> {
    let bytes = [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .into_iter()
        .find_map(|engine| engine.decode(value).ok())
        .ok_or_else(|| RegistryError::InvalidStack {
            method: "parseAddress",
            message: "failed to decode BoC base64".to_owned(),
        })?;

    TonCell::from_boc(bytes).map_err(cell_error)
}

fn parse_int_literal(value: &str) -> Result<i128, String> {
    if let Some(hex) = value
        .strip_prefix("-0x")
        .or_else(|| value.strip_prefix("-0X"))
    {
        return i128::from_str_radix(hex, 16)
            .map(|value| -value)
            .map_err(|err| err.to_string());
    }

    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return i128::from_str_radix(hex, 16).map_err(|err| err.to_string());
    }

    value.parse::<i128>().map_err(|err| err.to_string())
}

fn cell_error(error: impl std::fmt::Display) -> RegistryError {
    RegistryError::Cell(error.to_string())
}
