#![cfg(feature = "net")]

//! MetaMask-compatible EVM JSON-RPC facade backed by native stake registry balances.

use crate::net::StakeRegistry;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use once_cell::sync::Lazy;
use rlp::{Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha3::Keccak256;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 10_000;
const NATIVE_DECIMAL_FACTOR: u128 = 1_000_000_000_000_000_000;
const DEFAULT_BLOCK_PERIOD_SECS: u64 = 2;
const DEFAULT_GAS_LIMIT: u64 = 21_000;
const DEFAULT_GAS_PRICE: u64 = 1_000_000_000;

type Blake2b256 = blake2::Blake2b<U32>;

static TX_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    method: String,
    #[serde(default)]
    params: Value,
    id: Option<Value>,
}

#[derive(Debug)]
struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
        }
    }

    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedRawTx {
    hash: String,
    from: String,
    to: String,
    nonce: u64,
    value_wei: u128,
    gas_limit: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    input: String,
    y_parity: u8,
    r: String,
    s: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTxRecord {
    hash: String,
    from: String,
    to: String,
    nonce: u64,
    value_wei: String,
    gas_limit: u64,
    max_fee_per_gas: String,
    max_priority_fee_per_gas: String,
    input: String,
    block_number: u64,
    transaction_index: u64,
    y_parity: u8,
    r: String,
    s: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredReceiptRecord {
    transaction_hash: String,
    from: String,
    to: String,
    block_number: u64,
    transaction_index: u64,
    status: u8,
    gas_used: u64,
    cumulative_gas_used: u64,
    effective_gas_price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EvmRpcState {
    latest_block: u64,
    nonces: HashMap<String, u64>,
    txs: HashMap<String, StoredTxRecord>,
    receipts: HashMap<String, StoredReceiptRecord>,
}

/// EVM JSON-RPC listener configuration.
#[derive(Debug, Clone)]
pub struct EvmRpcConfig {
    /// Socket address where the RPC service listens.
    pub listen: SocketAddr,
    /// EVM chain ID exposed to wallets.
    pub chain_id: u64,
    /// Optional path to the native stake registry for balance lookups/transfers.
    pub stake_registry_path: Option<PathBuf>,
    /// Optional state path used for nonce/receipt persistence.
    pub state_path: Option<PathBuf>,
    /// Max request read timeout.
    pub request_timeout: Duration,
    /// Approximate block cadence in seconds for synthetic block number responses.
    pub block_period_secs: u64,
}

impl EvmRpcConfig {
    /// Build a config using sensible defaults.
    pub fn new(listen: SocketAddr, chain_id: u64, stake_registry_path: Option<PathBuf>) -> Self {
        let state_path = stake_registry_path
            .as_ref()
            .map(|path| path.with_file_name("evm_rpc_state.json"));
        Self {
            listen,
            chain_id,
            stake_registry_path,
            state_path,
            request_timeout: Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS),
            block_period_secs: DEFAULT_BLOCK_PERIOD_SECS,
        }
    }
}

/// Start the EVM RPC service and serve requests until the process exits.
pub async fn run_evm_rpc_server(cfg: EvmRpcConfig) -> io::Result<()> {
    let listener = TcpListener::bind(cfg.listen).await?;
    println!(
        "QSYS|mod=EVMRPC|evt=LISTEN|addr={}|chain_id={}",
        cfg.listen, cfg.chain_id
    );
    loop {
        let (mut stream, _) = listener.accept().await?;
        let cfg = cfg.clone();
        tokio::spawn(async move {
            if let Err(err) = handle_connection(&mut stream, &cfg).await {
                eprintln!("evm rpc connection error: {err}");
            }
        });
    }
}

async fn handle_connection(stream: &mut TcpStream, cfg: &EvmRpcConfig) -> io::Result<()> {
    let req = match read_http_request(
        stream,
        MAX_HEADER_BYTES,
        MAX_BODY_BYTES,
        cfg.request_timeout,
    )
    .await
    {
        Ok(req) => req,
        Err(err) => {
            let body = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {"code": -32700, "message": format!("parse error: {err}")},
            })
            .to_string();
            let resp = build_json_response("400 Bad Request", &body);
            let _ = stream.write_all(&resp).await;
            let _ = stream.shutdown().await;
            return Ok(());
        }
    };

    let method = req.method.to_ascii_uppercase();
    if method == "OPTIONS" {
        let resp = build_preflight_response();
        stream.write_all(&resp).await?;
        stream.shutdown().await?;
        return Ok(());
    }

    if method == "GET" && req.path == "/healthz" {
        let body = json!({
            "status": "ok",
            "service": "julian-evm-rpc",
            "chain_id": cfg.chain_id
        })
        .to_string();
        let resp = build_json_response("200 OK", &body);
        stream.write_all(&resp).await?;
        stream.shutdown().await?;
        return Ok(());
    }

    if method != "POST" {
        let body = json!({
            "jsonrpc": "2.0",
            "id": Value::Null,
            "error": {"code": -32600, "message": "invalid request method"},
        })
        .to_string();
        let resp = build_json_response("405 Method Not Allowed", &body);
        stream.write_all(&resp).await?;
        stream.shutdown().await?;
        return Ok(());
    }

    let _content_type = req.headers.get("content-type").cloned();

    let parsed: JsonRpcRequest = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(err) => {
            let body = json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {"code": -32700, "message": format!("parse error: {err}")},
            })
            .to_string();
            let resp = build_json_response("400 Bad Request", &body);
            stream.write_all(&resp).await?;
            stream.shutdown().await?;
            return Ok(());
        }
    };

    let id = parsed.id.clone().unwrap_or(Value::Null);
    let body = match handle_rpc_method(&parsed, cfg) {
        Ok(result) => json!({"jsonrpc":"2.0","id": id, "result": result}).to_string(),
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": err.code, "message": err.message}
        })
        .to_string(),
    };
    let resp = build_json_response("200 OK", &body);
    stream.write_all(&resp).await?;
    stream.shutdown().await
}

fn handle_rpc_method(req: &JsonRpcRequest, cfg: &EvmRpcConfig) -> Result<Value, RpcError> {
    match req.method.as_str() {
        "web3_clientVersion" => Ok(Value::String(format!(
            "julian/{}/evm-rpc",
            env!("CARGO_PKG_VERSION")
        ))),
        "net_version" => Ok(Value::String(cfg.chain_id.to_string())),
        "eth_chainId" => Ok(Value::String(to_quantity_u64(cfg.chain_id))),
        "eth_syncing" => Ok(Value::Bool(false)),
        "eth_blockNumber" => Ok(Value::String(to_quantity_u64(current_block_number(
            cfg.block_period_secs,
        )))),
        "eth_gasPrice" => Ok(Value::String(to_quantity_u64(DEFAULT_GAS_PRICE))),
        "eth_maxPriorityFeePerGas" => Ok(Value::String(to_quantity_u64(100_000_000))),
        "eth_feeHistory" => Ok(handle_fee_history(req, cfg)),
        "eth_getBalance" => {
            let address = rpc_param_string(&req.params, 0)
                .ok_or_else(|| RpcError::invalid_params("eth_getBalance expects address param"))?;
            let units = lookup_native_balance(cfg.stake_registry_path.as_deref(), &address)?;
            let wei = u128::from(units).saturating_mul(NATIVE_DECIMAL_FACTOR);
            Ok(Value::String(to_quantity_u128(wei)))
        }
        "eth_getTransactionCount" => {
            let address = rpc_param_string(&req.params, 0).ok_or_else(|| {
                RpcError::invalid_params("eth_getTransactionCount expects address param")
            })?;
            let nonce = load_nonce_for_address(cfg, &address)?;
            Ok(Value::String(to_quantity_u64(nonce)))
        }
        "eth_estimateGas" => Ok(Value::String(to_quantity_u64(DEFAULT_GAS_LIMIT))),
        "eth_getCode" => Ok(Value::String("0x".to_string())),
        "eth_call" => Ok(Value::String("0x".to_string())),
        "eth_accounts" => Ok(Value::Array(Vec::new())),
        "eth_coinbase" => Ok(Value::String(
            "0x0000000000000000000000000000000000000000".to_string(),
        )),
        "eth_getBlockByNumber" => Ok(handle_get_block_by_number(req, cfg)?),
        "eth_getBlockByHash" => Ok(Value::Null),
        "eth_getTransactionByHash" => handle_get_transaction_by_hash(req, cfg),
        "eth_getTransactionReceipt" => handle_get_transaction_receipt(req, cfg),
        "eth_sendRawTransaction" => handle_send_raw_transaction(req, cfg),
        "rpc_modules" => Ok(json!({
            "eth": "1.0",
            "net": "1.0",
            "web3": "1.0"
        })),
        other => Err(RpcError::method_not_found(other)),
    }
}

fn handle_fee_history(req: &JsonRpcRequest, cfg: &EvmRpcConfig) -> Value {
    let block_count = rpc_param_u64(&req.params, 0).unwrap_or(1).clamp(1, 64) as usize;
    let newest = current_block_number(cfg.block_period_secs);
    let mut base_fee_per_gas = Vec::with_capacity(block_count + 1);
    for _ in 0..=block_count {
        base_fee_per_gas.push(Value::String(to_quantity_u64(DEFAULT_GAS_PRICE)));
    }
    let gas_used_ratio = vec![Value::from(0.0); block_count];
    json!({
        "oldestBlock": to_quantity_u64(newest.saturating_sub(block_count as u64).saturating_add(1)),
        "baseFeePerGas": base_fee_per_gas,
        "gasUsedRatio": gas_used_ratio,
        "reward": []
    })
}

fn handle_get_block_by_number(req: &JsonRpcRequest, cfg: &EvmRpcConfig) -> Result<Value, RpcError> {
    let block = rpc_param_string(&req.params, 0).unwrap_or_else(|| "latest".to_string());
    let include_txs = rpc_param_bool(&req.params, 1).unwrap_or(false);
    let number = parse_block_tag(&block, cfg.block_period_secs);
    let hash = synthetic_block_hash(number);
    let parent_hash = synthetic_block_hash(number.saturating_sub(1));
    let tx_records = state_transactions_for_block(cfg, number)?;
    let txs = if include_txs {
        Value::Array(
            tx_records
                .iter()
                .map(|tx| tx_to_rpc_object(tx, true))
                .collect::<Vec<_>>(),
        )
    } else {
        Value::Array(
            tx_records
                .iter()
                .map(|tx| Value::String(tx.hash.clone()))
                .collect::<Vec<_>>(),
        )
    };

    Ok(json!({
        "number": to_quantity_u64(number),
        "hash": hash,
        "parentHash": parent_hash,
        "nonce": "0x0000000000000000",
        "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
        "logsBloom": "0x0",
        "transactionsRoot": "0x56e81f171bcc55a6ff8345e69d706f96f39c5c6f2a6fbb5f3a8f1c8f7c4b5e71",
        "stateRoot": "0x56e81f171bcc55a6ff8345e69d706f96f39c5c6f2a6fbb5f3a8f1c8f7c4b5e71",
        "receiptsRoot": "0x56e81f171bcc55a6ff8345e69d706f96f39c5c6f2a6fbb5f3a8f1c8f7c4b5e71",
        "miner": "0x0000000000000000000000000000000000000000",
        "difficulty": "0x0",
        "totalDifficulty": "0x0",
        "extraData": "0x6d66656e78",
        "size": "0x0",
        "gasLimit": to_quantity_u64(30_000_000),
        "gasUsed": to_quantity_u64((tx_records.len() as u64).saturating_mul(DEFAULT_GAS_LIMIT)),
        "timestamp": to_quantity_u64(now_secs()),
        "transactions": txs,
        "uncles": [],
        "baseFeePerGas": to_quantity_u64(DEFAULT_GAS_PRICE)
    }))
}

fn handle_get_transaction_by_hash(
    req: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let tx_hash = rpc_param_string(&req.params, 0)
        .ok_or_else(|| RpcError::invalid_params("eth_getTransactionByHash expects tx hash"))?;
    let state = load_state(cfg.state_path.as_deref())?;
    if let Some(tx) = state.txs.get(&tx_hash.to_ascii_lowercase()) {
        return Ok(tx_to_rpc_object(tx, false));
    }
    Ok(Value::Null)
}

fn handle_get_transaction_receipt(
    req: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let tx_hash = rpc_param_string(&req.params, 0)
        .ok_or_else(|| RpcError::invalid_params("eth_getTransactionReceipt expects tx hash"))?;
    let state = load_state(cfg.state_path.as_deref())?;
    if let Some(receipt) = state.receipts.get(&tx_hash.to_ascii_lowercase()) {
        let block_hash = synthetic_block_hash(receipt.block_number);
        return Ok(json!({
            "transactionHash": receipt.transaction_hash,
            "transactionIndex": to_quantity_u64(receipt.transaction_index),
            "blockHash": block_hash,
            "blockNumber": to_quantity_u64(receipt.block_number),
            "from": receipt.from,
            "to": receipt.to,
            "cumulativeGasUsed": to_quantity_u64(receipt.cumulative_gas_used),
            "gasUsed": to_quantity_u64(receipt.gas_used),
            "effectiveGasPrice": receipt.effective_gas_price,
            "contractAddress": Value::Null,
            "logs": [],
            "logsBloom": "0x0",
            "type": "0x2",
            "status": to_quantity_u64(receipt.status as u64)
        }));
    }
    Ok(Value::Null)
}

fn handle_send_raw_transaction(
    req: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let raw_hex = rpc_param_string(&req.params, 0)
        .ok_or_else(|| RpcError::invalid_params("eth_sendRawTransaction expects hex payload"))?;
    let raw = decode_hex_prefixed(&raw_hex)?;
    let parsed = decode_eip1559_transaction(&raw, cfg.chain_id)?;

    let _guard = TX_WRITE_LOCK
        .lock()
        .map_err(|_| RpcError::internal("rpc state lock poisoned"))?;

    let mut state = load_state(cfg.state_path.as_deref())?;
    if state.txs.contains_key(&parsed.hash) {
        return Ok(Value::String(parsed.hash));
    }

    let expected_nonce = state.nonces.get(&parsed.from).copied().unwrap_or(0);
    if parsed.nonce != expected_nonce {
        return Err(RpcError::internal(format!(
            "nonce mismatch: expected {expected_nonce}, got {}",
            parsed.nonce
        )));
    }

    let transfer_units = wei_to_native_units(parsed.value_wei)?;
    if transfer_units > 0 {
        apply_native_transfer(
            cfg.stake_registry_path.as_deref(),
            &parsed.from,
            &parsed.to,
            transfer_units,
        )?;
    }

    let next_block =
        current_block_number(cfg.block_period_secs).max(state.latest_block.saturating_add(1));
    let tx_index = state
        .txs
        .values()
        .filter(|tx| tx.block_number == next_block)
        .count() as u64;

    let tx_record = StoredTxRecord {
        hash: parsed.hash.clone(),
        from: parsed.from.clone(),
        to: parsed.to.clone(),
        nonce: parsed.nonce,
        value_wei: to_quantity_u128(parsed.value_wei),
        gas_limit: parsed.gas_limit,
        max_fee_per_gas: to_quantity_u128(parsed.max_fee_per_gas),
        max_priority_fee_per_gas: to_quantity_u128(parsed.max_priority_fee_per_gas),
        input: parsed.input,
        block_number: next_block,
        transaction_index: tx_index,
        y_parity: parsed.y_parity,
        r: parsed.r,
        s: parsed.s,
    };

    let receipt_record = StoredReceiptRecord {
        transaction_hash: parsed.hash.clone(),
        from: parsed.from.clone(),
        to: parsed.to,
        block_number: next_block,
        transaction_index: tx_index,
        status: 1,
        gas_used: DEFAULT_GAS_LIMIT,
        cumulative_gas_used: DEFAULT_GAS_LIMIT.saturating_mul(tx_index.saturating_add(1)),
        effective_gas_price: to_quantity_u64(DEFAULT_GAS_PRICE),
    };

    state.latest_block = next_block;
    state
        .nonces
        .insert(parsed.from, expected_nonce.saturating_add(1));
    state.txs.insert(tx_record.hash.clone(), tx_record);
    state
        .receipts
        .insert(receipt_record.transaction_hash.clone(), receipt_record);
    save_state(cfg.state_path.as_deref(), &state)?;

    Ok(Value::String(parsed.hash))
}

fn load_nonce_for_address(cfg: &EvmRpcConfig, address: &str) -> Result<u64, RpcError> {
    let normalized = normalize_evm_address(address)
        .ok_or_else(|| RpcError::invalid_params("invalid address format"))?;
    let state = load_state(cfg.state_path.as_deref())?;
    Ok(state.nonces.get(&normalized).copied().unwrap_or(0))
}

fn load_state(path: Option<&Path>) -> Result<EvmRpcState, RpcError> {
    let Some(path) = path else {
        return Ok(EvmRpcState::default());
    };
    if !path.exists() {
        return Ok(EvmRpcState::default());
    }
    let bytes = std::fs::read(path)
        .map_err(|err| RpcError::internal(format!("failed to read rpc state: {err}")))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| RpcError::internal(format!("failed to decode rpc state: {err}")))
}

fn save_state(path: Option<&Path>, state: &EvmRpcState) -> Result<(), RpcError> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| RpcError::internal(format!("failed to create state dir: {err}")))?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|err| RpcError::internal(format!("failed to encode rpc state: {err}")))?;
    std::fs::write(path, bytes)
        .map_err(|err| RpcError::internal(format!("failed to write rpc state: {err}")))
}

fn state_transactions_for_block(
    cfg: &EvmRpcConfig,
    block_number: u64,
) -> Result<Vec<StoredTxRecord>, RpcError> {
    let state = load_state(cfg.state_path.as_deref())?;
    let mut txs = state
        .txs
        .values()
        .filter(|tx| tx.block_number == block_number)
        .cloned()
        .collect::<Vec<_>>();
    txs.sort_by_key(|tx| tx.transaction_index);
    Ok(txs)
}

fn tx_to_rpc_object(tx: &StoredTxRecord, include_block: bool) -> Value {
    let block_hash = synthetic_block_hash(tx.block_number);
    let mut value = json!({
        "hash": tx.hash,
        "nonce": to_quantity_u64(tx.nonce),
        "from": tx.from,
        "to": tx.to,
        "value": tx.value_wei,
        "gas": to_quantity_u64(tx.gas_limit),
        "gasPrice": tx.max_fee_per_gas,
        "maxFeePerGas": tx.max_fee_per_gas,
        "maxPriorityFeePerGas": tx.max_priority_fee_per_gas,
        "input": tx.input,
        "type": "0x2",
        "v": to_quantity_u64(tx.y_parity as u64),
        "r": tx.r,
        "s": tx.s,
        "transactionIndex": to_quantity_u64(tx.transaction_index)
    });

    if include_block {
        value["blockHash"] = Value::String(block_hash);
        value["blockNumber"] = Value::String(to_quantity_u64(tx.block_number));
    } else {
        value["blockHash"] = Value::String(block_hash);
        value["blockNumber"] = Value::String(to_quantity_u64(tx.block_number));
    }
    value
}

fn decode_eip1559_transaction(raw: &[u8], expected_chain_id: u64) -> Result<ParsedRawTx, RpcError> {
    if raw.is_empty() {
        return Err(RpcError::invalid_params("empty raw transaction"));
    }
    if raw[0] != 0x02 {
        return Err(RpcError::invalid_params(
            "only EIP-1559 (type 0x02) transactions are supported",
        ));
    }

    let rlp = Rlp::new(&raw[1..]);
    if !rlp.is_list() {
        return Err(RpcError::invalid_params(
            "typed transaction payload is not an RLP list",
        ));
    }
    let items = rlp
        .item_count()
        .map_err(|err| RpcError::invalid_params(format!("invalid rlp item count: {err}")))?;
    if items != 12 {
        return Err(RpcError::invalid_params(format!(
            "expected 12 rlp fields for type-2 tx, found {items}"
        )));
    }

    let chain_id = rlp_u64(&rlp, 0)?;
    if chain_id != expected_chain_id {
        return Err(RpcError::invalid_params(format!(
            "chainId mismatch: tx={chain_id} rpc={expected_chain_id}"
        )));
    }

    let nonce = rlp_u64(&rlp, 1)?;
    let max_priority_fee_per_gas = rlp_u128(&rlp, 2)?;
    let max_fee_per_gas = rlp_u128(&rlp, 3)?;
    let gas_limit = rlp_u64(&rlp, 4)?;
    let to_bytes = rlp
        .at(5)
        .map_err(|err| RpcError::invalid_params(format!("missing to field: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid to field: {err}")))?;
    if to_bytes.len() != 20 {
        return Err(RpcError::invalid_params(
            "contract creation transactions are not supported",
        ));
    }
    let to = format!("0x{}", hex::encode(to_bytes));

    let value_wei = rlp_u128(&rlp, 6)?;
    let input_bytes = rlp
        .at(7)
        .map_err(|err| RpcError::invalid_params(format!("missing input field: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid input field: {err}")))?;
    if !input_bytes.is_empty() {
        return Err(RpcError::invalid_params(
            "contract call data is not supported for native transfer mode",
        ));
    }
    let input = format!("0x{}", hex::encode(input_bytes));

    let access_list_raw = rlp
        .at(8)
        .map_err(|err| RpcError::invalid_params(format!("missing access list: {err}")))?
        .as_raw()
        .to_vec();
    let y_parity = rlp_u64(&rlp, 9)?;
    if y_parity > 1 {
        return Err(RpcError::invalid_params("invalid y parity in signature"));
    }

    let r_raw = rlp
        .at(10)
        .map_err(|err| RpcError::invalid_params(format!("missing signature r: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid signature r: {err}")))?;
    let s_raw = rlp
        .at(11)
        .map_err(|err| RpcError::invalid_params(format!("missing signature s: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid signature s: {err}")))?;
    let r32 = left_pad_32(r_raw)?;
    let s32 = left_pad_32(s_raw)?;

    let mut sig_stream = RlpStream::new_list(9);
    sig_stream.append(&chain_id);
    sig_stream.append(&nonce);
    sig_stream.append(&max_priority_fee_per_gas);
    sig_stream.append(&max_fee_per_gas);
    sig_stream.append(&gas_limit);
    sig_stream.append(&to_bytes.as_ref());
    sig_stream.append(&value_wei);
    sig_stream.append(&input_bytes.as_ref());
    sig_stream.append_raw(&access_list_raw, 1);

    let encoded_stream = sig_stream.out();
    let mut signing_payload = Vec::with_capacity(1 + encoded_stream.len());
    signing_payload.push(0x02);
    signing_payload.extend_from_slice(encoded_stream.as_ref());
    let sighash = keccak256(&signing_payload);

    let sig = Signature::from_scalars(r32, s32)
        .map_err(|err| RpcError::invalid_params(format!("invalid signature scalars: {err}")))?;
    let recid = RecoveryId::from_byte(y_parity as u8)
        .ok_or_else(|| RpcError::invalid_params("invalid signature recovery id"))?;
    let vk = VerifyingKey::recover_from_prehash(&sighash, &sig, recid)
        .map_err(|err| RpcError::invalid_params(format!("signature recovery failed: {err}")))?;

    let pubkey = vk.to_encoded_point(false);
    let pub_bytes = pubkey.as_bytes();
    if pub_bytes.len() != 65 || pub_bytes[0] != 0x04 {
        return Err(RpcError::internal("unexpected recovered public key format"));
    }
    let from_digest = keccak256(&pub_bytes[1..]);
    let from = format!("0x{}", hex::encode(&from_digest[12..]));

    let tx_hash = format!("0x{}", hex::encode(keccak256(raw)));

    Ok(ParsedRawTx {
        hash: tx_hash,
        from,
        to,
        nonce,
        value_wei,
        gas_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        input,
        y_parity: y_parity as u8,
        r: format!("0x{}", hex::encode(r32)),
        s: format!("0x{}", hex::encode(s32)),
    })
}

fn rlp_u64(rlp: &Rlp<'_>, index: usize) -> Result<u64, RpcError> {
    let bytes = rlp
        .at(index)
        .map_err(|err| RpcError::invalid_params(format!("missing rlp field {index}: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid rlp field {index}: {err}")))?;
    u64_from_be_bytes(bytes)
}

fn rlp_u128(rlp: &Rlp<'_>, index: usize) -> Result<u128, RpcError> {
    let bytes = rlp
        .at(index)
        .map_err(|err| RpcError::invalid_params(format!("missing rlp field {index}: {err}")))?
        .data()
        .map_err(|err| RpcError::invalid_params(format!("invalid rlp field {index}: {err}")))?;
    u128_from_be_bytes(bytes)
}

fn u64_from_be_bytes(bytes: &[u8]) -> Result<u64, RpcError> {
    if bytes.len() > 8 {
        return Err(RpcError::invalid_params("integer overflow (u64)"));
    }
    let mut out = 0u64;
    for byte in bytes {
        out = (out << 8) | (*byte as u64);
    }
    Ok(out)
}

fn u128_from_be_bytes(bytes: &[u8]) -> Result<u128, RpcError> {
    if bytes.len() > 16 {
        return Err(RpcError::invalid_params("integer overflow (u128)"));
    }
    let mut out = 0u128;
    for byte in bytes {
        out = (out << 8) | (*byte as u128);
    }
    Ok(out)
}

fn left_pad_32(bytes: &[u8]) -> Result<[u8; 32], RpcError> {
    if bytes.len() > 32 {
        return Err(RpcError::invalid_params(
            "signature component exceeds 32 bytes",
        ));
    }
    let mut out = [0u8; 32];
    let offset = 32usize.saturating_sub(bytes.len());
    out[offset..].copy_from_slice(bytes);
    Ok(out)
}

fn decode_hex_prefixed(input: &str) -> Result<Vec<u8>, RpcError> {
    let trimmed = input.trim();
    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .ok_or_else(|| RpcError::invalid_params("hex payload must start with 0x"))?;
    hex::decode(raw).map_err(|err| RpcError::invalid_params(format!("invalid hex payload: {err}")))
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn wei_to_native_units(value_wei: u128) -> Result<u64, RpcError> {
    if value_wei % NATIVE_DECIMAL_FACTOR != 0 {
        return Err(RpcError::invalid_params(
            "value must be a whole native token amount (18-decimal aligned)",
        ));
    }
    let units = value_wei / NATIVE_DECIMAL_FACTOR;
    if units > u64::MAX as u128 {
        return Err(RpcError::invalid_params(
            "value exceeds native balance capacity",
        ));
    }
    Ok(units as u64)
}

fn apply_native_transfer(
    registry_path: Option<&Path>,
    from: &str,
    to: &str,
    amount: u64,
) -> Result<(), RpcError> {
    let path =
        registry_path.ok_or_else(|| RpcError::internal("stake registry path not configured"))?;
    let from_norm = normalize_evm_address(from)
        .ok_or_else(|| RpcError::invalid_params("invalid sender address"))?;
    let to_norm = normalize_evm_address(to)
        .ok_or_else(|| RpcError::invalid_params("invalid recipient address"))?;

    let mut registry = StakeRegistry::load(path)
        .map_err(|err| RpcError::internal(format!("failed to load stake registry: {err}")))?;

    let from_key =
        find_registry_key_for_address(&registry, &from_norm).unwrap_or(from_norm.clone());
    let to_key = find_registry_key_for_address(&registry, &to_norm).unwrap_or(to_norm.clone());

    registry
        .debit_fee(&from_key, amount)
        .map_err(|err| RpcError::internal(format!("insufficient balance for transfer: {err}")))?;
    registry.fund_balance(&to_key, amount);

    registry
        .save(path)
        .map_err(|err| RpcError::internal(format!("failed to persist stake registry: {err}")))
}

fn find_registry_key_for_address(registry: &StakeRegistry, address: &str) -> Option<String> {
    if registry.account(address).is_some() {
        return Some(address.to_string());
    }
    for key in registry.accounts().keys() {
        if let Some(addr) = normalize_evm_address(key) {
            if addr == address {
                return Some(key.clone());
            }
        }
    }
    None
}

fn lookup_native_balance(registry_path: Option<&Path>, address: &str) -> Result<u64, RpcError> {
    let registry_path = match registry_path {
        Some(path) => path,
        None => return Ok(0),
    };
    let query = normalize_evm_address(address)
        .ok_or_else(|| RpcError::invalid_params("invalid address format"))?;
    let registry = StakeRegistry::load(registry_path)
        .map_err(|err| RpcError::internal(format!("registry load failed: {err}")))?;

    if let Some(acct) = registry.account(&query) {
        return Ok(acct.balance);
    }
    if let Some(acct) = registry.account(address) {
        return Ok(acct.balance);
    }

    for (key, account) in registry.accounts() {
        if let Some(candidate) = registry_key_to_evm_address(key) {
            if candidate == query {
                return Ok(account.balance);
            }
        }
    }
    Ok(0)
}

fn registry_key_to_evm_address(key: &str) -> Option<String> {
    if let Some(addr) = normalize_evm_address(key) {
        return Some(addr);
    }
    let pubkey = BASE64.decode(key).ok()?;
    Some(derive_address_from_pubkey(&pubkey))
}

fn derive_address_from_pubkey(pubkey: &[u8]) -> String {
    let mut payload = Vec::with_capacity(32 + pubkey.len());
    payload.extend_from_slice(b"mfenx-migration-address-v1");
    payload.extend_from_slice(pubkey);
    let mut hasher = Blake2b256::new();
    hasher.update(&payload);
    let digest: [u8; 32] = hasher.finalize().into();
    format!("0x{}", hex::encode(&digest[12..]))
}

fn normalize_evm_address(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with("0x") || trimmed.starts_with("0X")) {
        return None;
    }
    if trimmed.len() != 42 {
        return None;
    }
    let raw = &trimmed[2..];
    if !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", raw.to_ascii_lowercase()))
}

fn parse_block_tag(tag: &str, block_period_secs: u64) -> u64 {
    let lower = tag.trim().to_ascii_lowercase();
    if lower == "latest" || lower == "pending" || lower == "safe" || lower == "finalized" {
        return current_block_number(block_period_secs);
    }
    let raw = lower.strip_prefix("0x").unwrap_or(&lower);
    u64::from_str_radix(raw, 16).unwrap_or_else(|_| current_block_number(block_period_secs))
}

fn synthetic_block_hash(number: u64) -> String {
    let mut payload = b"mfenx-native-block-v1:".to_vec();
    payload.extend_from_slice(&number.to_be_bytes());
    let mut hasher = Blake2b256::new();
    hasher.update(&payload);
    let digest: [u8; 32] = hasher.finalize().into();
    format!("0x{}", hex::encode(digest))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn current_block_number(block_period_secs: u64) -> u64 {
    let period = block_period_secs.max(1);
    now_secs() / period
}

fn to_quantity_u64(value: u64) -> String {
    format!("0x{value:x}")
}

fn to_quantity_u128(value: u128) -> String {
    format!("0x{value:x}")
}

fn rpc_param_string(params: &Value, index: usize) -> Option<String> {
    params
        .as_array()
        .and_then(|arr| arr.get(index))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn rpc_param_bool(params: &Value, index: usize) -> Option<bool> {
    params
        .as_array()
        .and_then(|arr| arr.get(index))
        .and_then(Value::as_bool)
}

fn rpc_param_u64(params: &Value, index: usize) -> Option<u64> {
    let value = params.as_array()?.get(index)?;
    if let Some(v) = value.as_u64() {
        return Some(v);
    }
    let s = value.as_str()?;
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    s.parse::<u64>().ok()
}

async fn read_http_request(
    stream: &mut TcpStream,
    max_header_bytes: usize,
    max_body_bytes: usize,
    timeout: Duration,
) -> io::Result<HttpRequest> {
    let mut buf = Vec::new();
    let mut header_end = None;
    loop {
        let mut tmp = [0u8; 1024];
        let n = time::timeout(timeout, stream.read(&mut tmp))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > max_header_bytes && header_end.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "header too large",
            ));
        }
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = Some(pos + 4);
            break;
        }
    }

    let end = header_end
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "malformed request"))?;
    let header_str = str::from_utf8(&buf[..end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header"))?;
    let mut lines = header_str.split("\r\n").filter(|line| !line.is_empty());
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_len: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if content_len > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "content-length exceeds limit",
        ));
    }

    let mut body = if end < buf.len() {
        buf[end..].to_vec()
    } else {
        Vec::new()
    };
    while body.len() < content_len {
        let remaining = content_len - body.len();
        let mut tmp = vec![0u8; remaining.min(8192)];
        let n = time::timeout(timeout, stream.read(&mut tmp))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    if body.len() < content_len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete request body",
        ));
    }

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn build_json_response(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: POST, OPTIONS, GET\r\n\
         Access-Control-Allow-Headers: content-type\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

fn build_preflight_response() -> Vec<u8> {
    b"HTTP/1.1 204 No Content\r\n\
      Access-Control-Allow-Origin: *\r\n\
      Access-Control-Allow-Methods: POST, OPTIONS, GET\r\n\
      Access-Control-Allow-Headers: content-type\r\n\
      Content-Length: 0\r\n\
      Connection: close\r\n\r\n"
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::{
        decode_hex_prefixed, derive_address_from_pubkey, lookup_native_balance,
        normalize_evm_address, to_quantity_u128, wei_to_native_units,
    };
    use crate::net::StakeRegistry;
    use std::fs;

    #[test]
    fn normalize_hex_address() {
        let addr = normalize_evm_address("0xAbCdEfabcdefABCDefAbcdefABcdefabCDefAb12");
        assert_eq!(
            addr.as_deref(),
            Some("0xabcdefabcdefabcdefabcdefabcdefabcdefab12")
        );
    }

    #[test]
    fn derive_address_is_stable() {
        let pubkey = vec![7u8; 32];
        let addr = derive_address_from_pubkey(&pubkey);
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42);
    }

    #[test]
    fn quantity_encoding_u128() {
        assert_eq!(to_quantity_u128(0), "0x0");
        assert_eq!(to_quantity_u128(15), "0xf");
    }

    #[test]
    fn lookup_balance_reads_direct_evm_key() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rpc_registry_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));

        let mut reg = StakeRegistry::default();
        reg.fund_balance("0xabcdefabcdefabcdefabcdefabcdefabcdefab12", 77);
        reg.save(&path).expect("save registry");

        let got = lookup_native_balance(Some(&path), "0xABCDefabCDefabCDefABcDEFAbCdEfABcDeFAb12")
            .expect("lookup");
        assert_eq!(got, 77);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn wei_alignment_enforced() {
        assert!(wei_to_native_units(1).is_err());
        assert_eq!(wei_to_native_units(1_000_000_000_000_000_000).unwrap(), 1);
    }

    #[test]
    fn decode_hex_requires_prefix() {
        assert!(decode_hex_prefixed("abcd").is_err());
        assert_eq!(decode_hex_prefixed("0x").unwrap(), Vec::<u8>::new());
    }
}
