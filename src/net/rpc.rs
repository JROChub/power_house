#![cfg(feature = "net")]

//! MetaMask-compatible JSON-RPC backed exclusively by finalized native-chain state.

use crate::net::native_chain::{
    decode_eip1559_transaction, decode_hex_prefixed, normalize_evm_address, to_quantity_u128,
    to_quantity_u64, FinalizedNativeBlock, NativeChainCommand, NativeTransaction,
    SharedNativeChainState, NATIVE_DECIMAL_FACTOR, NATIVE_GAS_LIMIT, NATIVE_GAS_PRICE,
};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, io, net::SocketAddr, str, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Semaphore},
    time,
};

type Blake2b256 = blake2::Blake2b<U32>;

const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_BATCH_REQUESTS: usize = 100;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_CONNECTIONS: usize = 256;
const EMPTY_UNCLES_HASH: &str =
    "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
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
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: message.into(),
        }
    }

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

    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            code: -32004,
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

#[derive(Clone)]
/// Runtime settings and shared finalized state for the wallet JSON-RPC server.
pub struct EvmRpcConfig {
    /// TCP socket on which the HTTP JSON-RPC service listens.
    pub listen: SocketAddr,
    /// Chain identifier returned to wallet clients.
    pub chain_id: u64,
    /// Quorum-finalized native chain state used for all reads.
    pub state: SharedNativeChainState,
    /// Bounded command path used to submit signed transactions to consensus.
    pub command_sender: mpsc::Sender<NativeChainCommand>,
    /// Maximum time allowed for request reads and transaction acceptance.
    pub request_timeout: Duration,
    /// Maximum number of concurrently serviced HTTP connections.
    pub connection_limit: std::sync::Arc<Semaphore>,
}

impl EvmRpcConfig {
    /// Creates an RPC configuration backed by the supplied consensus state and command queue.
    pub fn new(
        listen: SocketAddr,
        chain_id: u64,
        state: SharedNativeChainState,
        command_sender: mpsc::Sender<NativeChainCommand>,
    ) -> Self {
        Self {
            listen,
            chain_id,
            state,
            command_sender,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
            connection_limit: std::sync::Arc::new(Semaphore::new(DEFAULT_MAX_CONNECTIONS)),
        }
    }
}

/// Serves HTTP JSON-RPC until the task is cancelled or the listener fails.
pub async fn run_evm_rpc_server(cfg: EvmRpcConfig) -> io::Result<()> {
    let listener = TcpListener::bind(cfg.listen).await?;
    println!(
        "QSYS|mod=EVMRPC|evt=LISTEN|addr={}|chain_id={}|state=finalized",
        cfg.listen, cfg.chain_id
    );
    loop {
        let (mut stream, _) = listener.accept().await?;
        let permit = cfg
            .connection_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| io::Error::other("RPC connection limiter closed"))?;
        let cfg = cfg.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(err) = handle_connection(&mut stream, &cfg).await {
                eprintln!("evm rpc connection error: {err}");
            }
        });
    }
}

async fn handle_connection(stream: &mut TcpStream, cfg: &EvmRpcConfig) -> io::Result<()> {
    let request = match read_http_request(
        stream,
        MAX_HEADER_BYTES,
        MAX_BODY_BYTES,
        cfg.request_timeout,
    )
    .await
    {
        Ok(request) => request,
        Err(err) => {
            return write_json(
                stream,
                "400 Bad Request",
                &json_rpc_error(Value::Null, -32700, format!("parse error: {err}")),
            )
            .await;
        }
    };

    if request.method.eq_ignore_ascii_case("OPTIONS") {
        stream.write_all(&preflight_response()).await?;
        return stream.shutdown().await;
    }
    if request.method.eq_ignore_ascii_case("GET") && request.path == "/healthz" {
        let state = cfg.state.read().await;
        let body = json!({
            "status": "ok",
            "service": "power-house-finalized-rpc",
            "chain_id": cfg.chain_id,
            "finalized_block": state.latest_number(),
            "finalized_hash": state.latest_hash(),
        });
        return write_json(stream, "200 OK", &body).await;
    }
    if !request.method.eq_ignore_ascii_case("POST") {
        return write_json(
            stream,
            "405 Method Not Allowed",
            &json_rpc_error(Value::Null, -32600, "JSON-RPC requires POST"),
        )
        .await;
    }
    if !request.path.is_empty() && request.path != "/" {
        return write_json(
            stream,
            "404 Not Found",
            &json_rpc_error(Value::Null, -32600, "unknown RPC path"),
        )
        .await;
    }
    if let Some(content_type) = request.headers.get("content-type") {
        if !content_type
            .to_ascii_lowercase()
            .starts_with("application/json")
        {
            return write_json(
                stream,
                "415 Unsupported Media Type",
                &json_rpc_error(Value::Null, -32600, "content-type must be application/json"),
            )
            .await;
        }
    }

    let document: Value = match serde_json::from_slice(&request.body) {
        Ok(document) => document,
        Err(err) => {
            return write_json(
                stream,
                "400 Bad Request",
                &json_rpc_error(Value::Null, -32700, format!("parse error: {err}")),
            )
            .await;
        }
    };
    let response = if let Some(batch) = document.as_array() {
        if batch.is_empty() || batch.len() > MAX_BATCH_REQUESTS {
            Some(json_rpc_error(
                Value::Null,
                -32600,
                format!("batch size must be between 1 and {MAX_BATCH_REQUESTS}"),
            ))
        } else {
            let mut responses = Vec::new();
            for item in batch {
                if let Some(response) = process_request(item.clone(), cfg).await {
                    responses.push(response);
                }
            }
            if responses.is_empty() {
                None
            } else {
                Some(Value::Array(responses))
            }
        }
    } else {
        process_request(document, cfg).await
    };
    match response {
        Some(response) => write_json(stream, "200 OK", &response).await,
        None => write_no_content(stream).await,
    }
}

async fn process_request(document: Value, cfg: &EvmRpcConfig) -> Option<Value> {
    let request: JsonRpcRequest = match serde_json::from_value(document) {
        Ok(request) => request,
        Err(err) => {
            return Some(json_rpc_error(
                Value::Null,
                -32600,
                format!("invalid request: {err}"),
            ));
        }
    };
    let notification = request.id.is_none();
    let id = request.id.clone().unwrap_or(Value::Null);
    let result = if request.jsonrpc.as_deref() != Some("2.0") {
        Err(RpcError::invalid_request("jsonrpc must equal 2.0"))
    } else {
        handle_rpc_method(&request, cfg).await
    };
    if notification {
        return None;
    }
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => json_rpc_error(id, error.code, error.message),
    })
}

async fn handle_rpc_method(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    match request.method.as_str() {
        "web3_clientVersion" => Ok(Value::String(format!(
            "power-house/{}/finalized-native-rpc",
            env!("CARGO_PKG_VERSION")
        ))),
        "net_version" => Ok(Value::String(cfg.chain_id.to_string())),
        "eth_chainId" => Ok(Value::String(to_quantity_u64(cfg.chain_id))),
        "eth_syncing" => Ok(Value::Bool(false)),
        "eth_blockNumber" => {
            let state = cfg.state.read().await;
            Ok(Value::String(to_quantity_u64(state.latest_number())))
        }
        "eth_gasPrice" => Ok(Value::String(to_quantity_u64(NATIVE_GAS_PRICE))),
        "eth_maxPriorityFeePerGas" => Ok(Value::String("0x0".to_string())),
        "eth_feeHistory" => handle_fee_history(request, cfg).await,
        "eth_getBalance" => {
            let address = required_string(&request.params, 0, "address")?;
            let address = normalize_evm_address(&address)
                .ok_or_else(|| RpcError::invalid_params("invalid address format"))?;
            let state = cfg.state.read().await;
            let number = requested_block(&request.params, 1, state.latest_number())?;
            let units = state
                .account_at(&address, number)
                .map_err(RpcError::invalid_params)?
                .balance;
            Ok(Value::String(to_quantity_u128(
                u128::from(units).saturating_mul(NATIVE_DECIMAL_FACTOR),
            )))
        }
        "eth_getTransactionCount" => {
            let address = required_string(&request.params, 0, "address")?;
            let address = normalize_evm_address(&address)
                .ok_or_else(|| RpcError::invalid_params("invalid address format"))?;
            let state = cfg.state.read().await;
            let number = requested_block(&request.params, 1, state.latest_number())?;
            Ok(Value::String(to_quantity_u64(
                state
                    .account_at(&address, number)
                    .map_err(RpcError::invalid_params)?
                    .nonce,
            )))
        }
        "eth_estimateGas" => validate_native_call(&request.params)
            .map(|_| Value::String(to_quantity_u64(NATIVE_GAS_LIMIT))),
        "eth_getCode" => Ok(Value::String("0x".to_string())),
        "eth_call" | "eth_getStorageAt" => Err(RpcError::unsupported(
            "contract execution is not available on the native transfer chain",
        )),
        "eth_accounts" => Ok(Value::Array(Vec::new())),
        "eth_coinbase" => Ok(Value::String(zero_address())),
        "eth_getBlockByNumber" => get_block_by_number(request, cfg).await,
        "eth_getBlockByHash" => get_block_by_hash(request, cfg).await,
        "eth_getBlockTransactionCountByNumber" => {
            block_transaction_count_by_number(request, cfg).await
        }
        "eth_getBlockTransactionCountByHash" => block_transaction_count_by_hash(request, cfg).await,
        "eth_getTransactionByHash" => get_transaction_by_hash(request, cfg).await,
        "eth_getTransactionReceipt" => get_transaction_receipt(request, cfg).await,
        "eth_sendRawTransaction" => send_raw_transaction(request, cfg).await,
        "eth_getLogs" => Ok(Value::Array(Vec::new())),
        "rpc_modules" => Ok(json!({"eth":"1.0","net":"1.0","web3":"1.0"})),
        other => Err(RpcError::method_not_found(other)),
    }
}

async fn handle_fee_history(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let requested = optional_u64(&request.params, 0).unwrap_or(1).clamp(1, 64) as usize;
    let state = cfg.state.read().await;
    let newest = state.latest_number();
    let count = requested.min(newest.saturating_add(1) as usize);
    Ok(json!({
        "oldestBlock": to_quantity_u64(newest.saturating_add(1).saturating_sub(count as u64)),
        "baseFeePerGas": vec![to_quantity_u64(NATIVE_GAS_PRICE); count + 1],
        "gasUsedRatio": vec![0.0; count],
        "reward": []
    }))
}

async fn get_block_by_number(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let tag = required_string(&request.params, 0, "block tag")?;
    let include_transactions = optional_bool(&request.params, 1).unwrap_or(false);
    let state = cfg.state.read().await;
    let number = parse_block_tag(&tag, state.latest_number())?;
    Ok(state
        .block_by_number(number)
        .map(|block| block_to_rpc(block, include_transactions))
        .unwrap_or(Value::Null))
}

async fn get_block_by_hash(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let hash = required_string(&request.params, 0, "block hash")?;
    let include_transactions = optional_bool(&request.params, 1).unwrap_or(false);
    let state = cfg.state.read().await;
    Ok(state
        .block_by_hash(&hash)
        .map(|block| block_to_rpc(block, include_transactions))
        .unwrap_or(Value::Null))
}

async fn block_transaction_count_by_number(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let tag = required_string(&request.params, 0, "block tag")?;
    let state = cfg.state.read().await;
    let number = parse_block_tag(&tag, state.latest_number())?;
    Ok(state
        .block_by_number(number)
        .map(|block| Value::String(to_quantity_u64(block.proposal.transactions.len() as u64)))
        .unwrap_or(Value::Null))
}

async fn block_transaction_count_by_hash(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let hash = required_string(&request.params, 0, "block hash")?;
    let state = cfg.state.read().await;
    Ok(state
        .block_by_hash(&hash)
        .map(|block| Value::String(to_quantity_u64(block.proposal.transactions.len() as u64)))
        .unwrap_or(Value::Null))
}

async fn get_transaction_by_hash(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let hash = required_string(&request.params, 0, "transaction hash")?;
    let state = cfg.state.read().await;
    Ok(state
        .transaction(&hash)
        .map(|(block, index, tx)| transaction_to_rpc(tx, block, index))
        .unwrap_or(Value::Null))
}

async fn get_transaction_receipt(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let hash = required_string(&request.params, 0, "transaction hash")?;
    let state = cfg.state.read().await;
    Ok(state
        .transaction(&hash)
        .map(|(block, index, tx)| {
            json!({
                "transactionHash": tx.hash,
                "transactionIndex": to_quantity_u64(index as u64),
                "blockHash": block.proposal.hash,
                "blockNumber": to_quantity_u64(block.proposal.number),
                "from": tx.from,
                "to": tx.to,
                "cumulativeGasUsed": to_quantity_u64(
                    NATIVE_GAS_LIMIT.saturating_mul(index as u64 + 1)
                ),
                "gasUsed": to_quantity_u64(NATIVE_GAS_LIMIT),
                "effectiveGasPrice": to_quantity_u64(NATIVE_GAS_PRICE),
                "contractAddress": Value::Null,
                "logs": [],
                "logsBloom": zero_bloom(),
                "type": "0x2",
                "status": "0x1"
            })
        })
        .unwrap_or(Value::Null))
}

async fn send_raw_transaction(
    request: &JsonRpcRequest,
    cfg: &EvmRpcConfig,
) -> Result<Value, RpcError> {
    let raw_hex = required_string(&request.params, 0, "raw transaction")?;
    let raw = decode_hex_prefixed(&raw_hex).map_err(RpcError::invalid_params)?;
    let transaction =
        decode_eip1559_transaction(&raw, cfg.chain_id).map_err(RpcError::invalid_params)?;
    let hash = transaction.hash.clone();
    let (sender, receiver) = oneshot::channel();
    cfg.command_sender
        .send(NativeChainCommand {
            transaction,
            response: sender,
        })
        .await
        .map_err(|_| RpcError::internal("native chain command queue is unavailable"))?;
    let result = time::timeout(cfg.request_timeout, receiver)
        .await
        .map_err(|_| RpcError::internal("native chain acceptance timed out"))?
        .map_err(|_| RpcError::internal("native chain acceptance channel closed"))?;
    result.map_err(RpcError::internal)?;
    Ok(Value::String(hash))
}

fn block_to_rpc(block: &FinalizedNativeBlock, include_transactions: bool) -> Value {
    let transactions = if include_transactions {
        Value::Array(
            block
                .proposal
                .transactions
                .iter()
                .enumerate()
                .map(|(index, tx)| transaction_to_rpc(tx, block, index))
                .collect(),
        )
    } else {
        Value::Array(
            block
                .proposal
                .transactions
                .iter()
                .map(|tx| Value::String(tx.hash.clone()))
                .collect(),
        )
    };
    let gas_used = NATIVE_GAS_LIMIT.saturating_mul(block.proposal.transactions.len() as u64);
    json!({
        "number": to_quantity_u64(block.proposal.number),
        "hash": block.proposal.hash,
        "parentHash": block.proposal.parent_hash,
        "nonce": "0x0000000000000000",
        "sha3Uncles": EMPTY_UNCLES_HASH,
        "logsBloom": zero_bloom(),
        "transactionsRoot": content_hash(
            b"mfenx-native-transactions-root-v1",
            &block.proposal.transactions
        ),
        "stateRoot": block.proposal.state_root,
        "receiptsRoot": content_hash(
            b"mfenx-native-receipts-root-v1",
            &block.proposal.transactions.iter().map(|tx| &tx.hash).collect::<Vec<_>>()
        ),
        "miner": zero_address(),
        "difficulty": "0x0",
        "totalDifficulty": "0x0",
        "extraData": format!("0x{}", hex::encode("mfenx-finalized-native-v1")),
        "size": to_quantity_u64(
            serde_json::to_vec(block).map(|bytes| bytes.len() as u64).unwrap_or_default()
        ),
        "gasLimit": to_quantity_u64(30_000_000),
        "gasUsed": to_quantity_u64(gas_used),
        "timestamp": to_quantity_u64(block.proposal.timestamp),
        "transactions": transactions,
        "uncles": [],
        "baseFeePerGas": to_quantity_u64(NATIVE_GAS_PRICE)
    })
}

fn transaction_to_rpc(tx: &NativeTransaction, block: &FinalizedNativeBlock, index: usize) -> Value {
    json!({
        "hash": tx.hash,
        "nonce": to_quantity_u64(tx.nonce),
        "blockHash": block.proposal.hash,
        "blockNumber": to_quantity_u64(block.proposal.number),
        "transactionIndex": to_quantity_u64(index as u64),
        "from": tx.from,
        "to": tx.to,
        "value": tx.value_wei,
        "gas": to_quantity_u64(tx.gas_limit),
        "gasPrice": to_quantity_u64(NATIVE_GAS_PRICE),
        "maxFeePerGas": tx.max_fee_per_gas,
        "maxPriorityFeePerGas": tx.max_priority_fee_per_gas,
        "input": tx.input,
        "type": "0x2",
        "v": to_quantity_u64(tx.y_parity as u64),
        "r": tx.r,
        "s": tx.s,
        "chainId": to_quantity_u64(block.proposal.chain_id)
    })
}

fn validate_native_call(params: &Value) -> Result<(), RpcError> {
    let request = params
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_object)
        .ok_or_else(|| RpcError::invalid_params("eth_estimateGas expects a transaction object"))?;
    if request.get("to").and_then(Value::as_str).is_none() {
        return Err(RpcError::unsupported("contract creation is not supported"));
    }
    if request
        .get("data")
        .or_else(|| request.get("input"))
        .and_then(Value::as_str)
        .map(|data| data != "0x" && data != "0x0")
        .unwrap_or(false)
    {
        return Err(RpcError::unsupported("contract calldata is not supported"));
    }
    Ok(())
}

fn parse_block_tag(tag: &str, latest: u64) -> Result<u64, RpcError> {
    match tag.to_ascii_lowercase().as_str() {
        "latest" | "finalized" | "safe" | "pending" => Ok(latest),
        "earliest" => Ok(0),
        value => {
            let raw = value
                .strip_prefix("0x")
                .ok_or_else(|| RpcError::invalid_params("block tag must be canonical hex"))?;
            u64::from_str_radix(raw, 16)
                .map_err(|_| RpcError::invalid_params("invalid block number"))
        }
    }
}

fn requested_block(params: &Value, index: usize, latest: u64) -> Result<u64, RpcError> {
    let tag = params
        .as_array()
        .and_then(|items| items.get(index))
        .and_then(Value::as_str)
        .unwrap_or("latest");
    parse_block_tag(tag, latest)
}

fn required_string(params: &Value, index: usize, label: &str) -> Result<String, RpcError> {
    params
        .as_array()
        .and_then(|items| items.get(index))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| RpcError::invalid_params(format!("missing {label} parameter")))
}

fn optional_bool(params: &Value, index: usize) -> Option<bool> {
    params.as_array()?.get(index)?.as_bool()
}

fn optional_u64(params: &Value, index: usize) -> Option<u64> {
    let value = params.as_array()?.get(index)?;
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    let text = value.as_str()?;
    if let Some(hex) = text.strip_prefix("0x") {
        return u64::from_str_radix(hex, 16).ok();
    }
    text.parse().ok()
}

fn content_hash<T: serde::Serialize>(domain: &[u8], value: &T) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(domain);
    if let Ok(bytes) = serde_json::to_vec(value) {
        hasher.update(bytes);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    format!("0x{}", hex::encode(digest))
}

fn zero_address() -> String {
    format!("0x{}", "00".repeat(20))
}

fn zero_bloom() -> String {
    format!("0x{}", "00".repeat(256))
}

fn json_rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message.into()}
    })
}

async fn write_json(stream: &mut TcpStream, status: &str, body: &Value) -> io::Result<()> {
    let encoded = serde_json::to_vec(body)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: POST, OPTIONS, GET\r\n\
         Access-Control-Allow-Headers: content-type\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        encoded.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(&encoded).await?;
    stream.shutdown().await
}

async fn write_no_content(stream: &mut TcpStream) -> io::Result<()> {
    stream
        .write_all(
            b"HTTP/1.1 204 No Content\r\n\
              Access-Control-Allow-Origin: *\r\n\
              Cache-Control: no-store\r\n\
              Content-Length: 0\r\n\
              Connection: close\r\n\r\n",
        )
        .await?;
    stream.shutdown().await
}

fn preflight_response() -> Vec<u8> {
    b"HTTP/1.1 204 No Content\r\n\
      Access-Control-Allow-Origin: *\r\n\
      Access-Control-Allow-Methods: POST, OPTIONS, GET\r\n\
      Access-Control-Allow-Headers: content-type\r\n\
      Cache-Control: no-store\r\n\
      Content-Length: 0\r\n\
      Connection: close\r\n\r\n"
        .to_vec()
}

async fn read_http_request(
    stream: &mut TcpStream,
    max_header_bytes: usize,
    max_body_bytes: usize,
    timeout: Duration,
) -> io::Result<HttpRequest> {
    let mut buffer = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 1024];
        let read = time::timeout(timeout, stream.read(&mut chunk))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete HTTP request",
            ));
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.len() > max_header_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP headers exceed limit",
            ));
        }
        if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let header_text = str::from_utf8(&buffer[..header_end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP headers"))?;
    let mut lines = header_text.split("\r\n").filter(|line| !line.is_empty());
    let mut request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?
        .split_whitespace();
    let method = request_line.next().unwrap_or_default().to_string();
    let path = request_line.next().unwrap_or_default().to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_default();
    if content_length > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request body exceeds limit",
        ));
    }
    let mut body = buffer[header_end..].to_vec();
    while body.len() < content_length {
        let mut chunk = vec![0u8; (content_length - body.len()).min(8192)];
        let read = time::timeout(timeout, stream.read(&mut chunk))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete request body",
            ));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{
        encode_public_key_base64, load_or_derive_keypair,
        native_chain::{
            normalize_evm_address, signed_test_transfer, NativeChainMessage,
            NativeChainMessagePayload, NativeChainRuntime, NativeChainState,
        },
        Ed25519KeySource, StakeRegistry,
    };
    use std::{fs, net::TcpListener as StdTcpListener, sync::Arc};
    use tokio::sync::RwLock;

    #[test]
    fn block_tags_are_strict() {
        assert_eq!(parse_block_tag("latest", 7).unwrap(), 7);
        assert_eq!(parse_block_tag("0x2a", 7).unwrap(), 42);
        assert!(parse_block_tag("42", 7).is_err());
    }

    #[test]
    fn native_call_rejects_calldata() {
        let params = json!([{"to":"0x0000000000000000000000000000000000000001","data":"0x12"}]);
        assert!(validate_native_call(&params).is_err());
    }

    #[test]
    fn address_normalization_is_available_to_rpc() {
        assert_eq!(
            normalize_evm_address("0xABCDEFabcdefABCDEFabcdefABCDEFabcdefABCD"),
            Some("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn raw_transaction_reaches_finalized_receipt_over_http() {
        let root = std::env::temp_dir().join(format!(
            "powerhouse_rpc_http_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let chain_id = 177155;
        let transaction = signed_test_transfer([7u8; 32], chain_id, 0, [9u8; 20], 2);
        let registry_path = root.join("stake_registry.json");
        let mut registry = StakeRegistry::default();
        registry.fund_balance(&transaction.from, 5);
        registry.save(&registry_path).unwrap();

        let validator =
            load_or_derive_keypair(&Ed25519KeySource::Seed("rpc-http".to_string())).unwrap();
        let validators = vec![encode_public_key_base64(&validator.verifying)];
        let state_path = root.join("native_chain_state.json");
        let state = NativeChainState::load_or_initialize(
            &state_path,
            chain_id,
            Some(&registry_path),
            validators.clone(),
            1,
        )
        .unwrap();
        let shared = Arc::new(RwLock::new(state));
        let mut runtime = NativeChainRuntime::new(
            shared.clone(),
            state_path,
            validators,
            1,
            &validator.signing,
        )
        .await
        .unwrap();
        let (sender, mut receiver) = mpsc::channel(8);

        let reserved = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let listen = reserved.local_addr().unwrap();
        drop(reserved);
        let server = tokio::spawn(run_evm_rpc_server(EvmRpcConfig::new(
            listen, chain_id, shared, sender,
        )));
        let consensus = tokio::spawn(async move {
            let command = receiver.recv().await.unwrap();
            let hash = command.transaction.hash.clone();
            runtime
                .accept_transaction(command.transaction)
                .await
                .unwrap();
            command.response.send(Ok(hash)).unwrap();
            let proposal = runtime.propose(&validator.signing).await.unwrap().unwrap();
            let vote_messages = runtime
                .handle_message(
                    NativeChainMessage::new(NativeChainMessagePayload::Proposal(proposal)),
                    &validator.signing,
                )
                .await
                .unwrap();
            for vote in vote_messages {
                runtime
                    .handle_message(vote, &validator.signing)
                    .await
                    .unwrap();
            }
        });

        let client = reqwest::Client::new();
        let url = format!("http://{listen}");
        for _ in 0..50 {
            if client.get(format!("{url}/healthz")).send().await.is_ok() {
                break;
            }
            time::sleep(Duration::from_millis(20)).await;
        }
        let response: Value = client
            .post(&url)
            .json(&json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"eth_sendRawTransaction",
                "params":[transaction.raw]
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(response["result"], transaction.hash);
        consensus.await.unwrap();

        let receipt: Value = client
            .post(&url)
            .json(&json!({
                "jsonrpc":"2.0",
                "id":2,
                "method":"eth_getTransactionReceipt",
                "params":[transaction.hash]
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(receipt["result"]["blockNumber"], "0x1");
        assert_eq!(receipt["result"]["status"], "0x1");

        let genesis_balance: Value = client
            .post(&url)
            .json(&json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"eth_getBalance",
                "params":[transaction.from, "earliest"]
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let finalized_balance: Value = client
            .post(&url)
            .json(&json!({
                "jsonrpc":"2.0",
                "id":4,
                "method":"eth_getBalance",
                "params":[transaction.from, "latest"]
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            genesis_balance["result"],
            to_quantity_u128(5 * NATIVE_DECIMAL_FACTOR)
        );
        assert_eq!(
            finalized_balance["result"],
            to_quantity_u128(3 * NATIVE_DECIMAL_FACTOR)
        );
        server.abort();
        let _ = fs::remove_dir_all(root);
    }
}
