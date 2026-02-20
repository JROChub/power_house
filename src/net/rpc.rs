#![cfg(feature = "net")]

//! Minimal MetaMask-compatible EVM JSON-RPC facade backed by native stake registry balances.

use crate::net::StakeRegistry;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time;

const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 512 * 1024;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 10_000;
const NATIVE_DECIMAL_FACTOR: u128 = 1_000_000_000_000_000_000;
const DEFAULT_BLOCK_PERIOD_SECS: u64 = 2;

type Blake2b256 = blake2::Blake2b<U32>;

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

/// EVM JSON-RPC listener configuration.
#[derive(Debug, Clone)]
pub struct EvmRpcConfig {
    /// Socket address where the RPC service listens.
    pub listen: SocketAddr,
    /// EVM chain ID exposed to wallets.
    pub chain_id: u64,
    /// Optional path to the native stake registry for balance lookups.
    pub stake_registry_path: Option<PathBuf>,
    /// Max request read timeout.
    pub request_timeout: Duration,
    /// Approximate block cadence in seconds for synthetic block number responses.
    pub block_period_secs: u64,
}

impl EvmRpcConfig {
    /// Build a config using sensible defaults.
    pub fn new(listen: SocketAddr, chain_id: u64, stake_registry_path: Option<PathBuf>) -> Self {
        Self {
            listen,
            chain_id,
            stake_registry_path,
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

    // touch headers so the parser keeps ownership meaningful (auth may be layered later)
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
        "eth_gasPrice" => Ok(Value::String(to_quantity_u64(1_000_000_000))),
        "eth_maxPriorityFeePerGas" => Ok(Value::String(to_quantity_u64(100_000_000))),
        "eth_feeHistory" => Ok(handle_fee_history(req, cfg)),
        "eth_getBalance" => {
            let address = rpc_param_string(&req.params, 0)
                .ok_or_else(|| RpcError::invalid_params("eth_getBalance expects address param"))?;
            let units = lookup_native_balance(cfg.stake_registry_path.as_deref(), &address)?;
            let wei = u128::from(units).saturating_mul(NATIVE_DECIMAL_FACTOR);
            Ok(Value::String(to_quantity_u128(wei)))
        }
        "eth_getTransactionCount" => Ok(Value::String("0x0".to_string())),
        "eth_estimateGas" => Ok(Value::String("0x5208".to_string())),
        "eth_getCode" => Ok(Value::String("0x".to_string())),
        "eth_call" => Ok(Value::String("0x".to_string())),
        "eth_accounts" => Ok(Value::Array(Vec::new())),
        "eth_coinbase" => Ok(Value::String(
            "0x0000000000000000000000000000000000000000".to_string(),
        )),
        "eth_getBlockByNumber" => Ok(handle_get_block_by_number(req, cfg)),
        "eth_getBlockByHash" => Ok(Value::Null),
        "eth_getTransactionByHash" => Ok(Value::Null),
        "eth_getTransactionReceipt" => Ok(Value::Null),
        "eth_sendRawTransaction" => Err(RpcError::internal(
            "native tx execution via raw EVM payload is not enabled yet",
        )),
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
        base_fee_per_gas.push(Value::String("0x3b9aca00".to_string()));
    }
    let gas_used_ratio = vec![Value::from(0.0); block_count];
    json!({
        "oldestBlock": to_quantity_u64(newest.saturating_sub(block_count as u64).saturating_add(1)),
        "baseFeePerGas": base_fee_per_gas,
        "gasUsedRatio": gas_used_ratio,
        "reward": []
    })
}

fn handle_get_block_by_number(req: &JsonRpcRequest, cfg: &EvmRpcConfig) -> Value {
    let block = rpc_param_string(&req.params, 0).unwrap_or_else(|| "latest".to_string());
    let include_txs = rpc_param_bool(&req.params, 1).unwrap_or(false);
    let number = parse_block_tag(&block, cfg.block_period_secs);
    let hash = synthetic_block_hash(number);
    let parent_hash = synthetic_block_hash(number.saturating_sub(1));
    let txs = if include_txs {
        Value::Array(Vec::new())
    } else {
        Value::Array(Vec::new())
    };
    json!({
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
        "gasLimit": "0x1c9c380",
        "gasUsed": "0x0",
        "timestamp": to_quantity_u64(now_secs()),
        "transactions": txs,
        "uncles": [],
        "baseFeePerGas": "0x3b9aca00"
    })
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
        derive_address_from_pubkey, lookup_native_balance, normalize_evm_address, to_quantity_u128,
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
}
