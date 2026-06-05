#![cfg(feature = "net")]
#![allow(missing_docs)]

//! Quorum-finalized native transfers exposed through the wallet RPC adapter.

use crate::net::{
    decode_public_key_base64, encode_public_key_base64, encode_signature_base64,
    verify_signature_base64, StakeRegistry,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use ed25519_dalek::{Signer, SigningKey};
use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use rlp::{Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{oneshot, RwLock};

type Blake2b256 = blake2::Blake2b<U32>;

pub const NATIVE_CHAIN_TOPIC: &str = "mfenx/powerhouse/native-chain/v1";
pub const NATIVE_DECIMAL_FACTOR: u128 = 1_000_000_000_000_000_000;
pub const NATIVE_GAS_LIMIT: u64 = 21_000;
pub const NATIVE_GAS_PRICE: u64 = 0;
const STATE_SCHEMA: &str = "mfenx.powerhouse.native-chain-state.v1";
const MESSAGE_SCHEMA: &str = "mfenx.powerhouse.native-chain-message.v1";
const MAX_BLOCK_TRANSACTIONS: usize = 256;
const MAX_FUTURE_SECONDS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTransaction {
    pub hash: String,
    pub raw: String,
    pub from: String,
    pub to: String,
    pub nonce: u64,
    pub value_wei: String,
    pub value_units: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub input: String,
    pub y_parity: u8,
    pub r: String,
    pub s: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NativeAccount {
    pub balance: u64,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBlockProposal {
    pub chain_id: u64,
    pub number: u64,
    pub parent_hash: String,
    pub timestamp: u64,
    pub proposer: String,
    pub transactions: Vec<NativeTransaction>,
    pub state_root: String,
    pub hash: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeBlockVote {
    pub block_hash: String,
    pub block_number: u64,
    pub validator: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedNativeBlock {
    pub proposal: NativeBlockProposal,
    pub votes: Vec<NativeBlockVote>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChainTip {
    pub height: u64,
    pub hash: String,
    pub observed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChainSyncRequest {
    pub from_height: u64,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChainSyncResponse {
    pub request_id: String,
    pub block: FinalizedNativeBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChainState {
    pub schema: String,
    pub chain_id: u64,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub genesis_accounts: BTreeMap<String, NativeAccount>,
    pub accounts: BTreeMap<String, NativeAccount>,
    pub blocks: Vec<FinalizedNativeBlock>,
    #[serde(default)]
    pub votes_cast: BTreeMap<u64, String>,
}

pub type SharedNativeChainState = Arc<RwLock<NativeChainState>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum NativeChainMessagePayload {
    Transaction(NativeTransaction),
    Proposal(NativeBlockProposal),
    Vote(NativeBlockVote),
    Finalized(FinalizedNativeBlock),
    Tip(NativeChainTip),
    SyncRequest(NativeChainSyncRequest),
    SyncResponse(NativeChainSyncResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChainMessage {
    pub schema: String,
    pub payload: NativeChainMessagePayload,
}

impl NativeChainMessage {
    pub fn new(payload: NativeChainMessagePayload) -> Self {
        Self {
            schema: MESSAGE_SCHEMA.to_string(),
            payload,
        }
    }

    pub fn validate_schema(&self) -> Result<(), String> {
        if self.schema != MESSAGE_SCHEMA {
            return Err(format!("unsupported native-chain schema: {}", self.schema));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct NativeChainCommand {
    pub transaction: NativeTransaction,
    pub response: oneshot::Sender<Result<String, String>>,
}

pub struct NativeChainRuntime {
    pub state: SharedNativeChainState,
    state_path: PathBuf,
    validators: Vec<String>,
    quorum: usize,
    local_validator: String,
    pending: BTreeMap<String, NativeTransaction>,
    proposals: BTreeMap<String, NativeBlockProposal>,
    votes: BTreeMap<String, BTreeMap<String, NativeBlockVote>>,
    orphan_votes: BTreeMap<String, BTreeMap<String, NativeBlockVote>>,
    voted_heights: BTreeMap<u64, String>,
}

impl NativeChainRuntime {
    pub async fn new(
        state: SharedNativeChainState,
        state_path: PathBuf,
        mut validators: Vec<String>,
        quorum: usize,
        signing: &SigningKey,
    ) -> Result<Self, String> {
        validators.sort();
        validators.dedup();
        if validators.is_empty() {
            return Err("native chain requires at least one validator".to_string());
        }
        if quorum == 0 || quorum > validators.len() {
            return Err(format!(
                "native chain quorum {quorum} is invalid for {} validators",
                validators.len()
            ));
        }
        if quorum.saturating_mul(2) <= validators.len() {
            return Err(format!(
                "native chain quorum {quorum} must be a strict majority of {} validators",
                validators.len()
            ));
        }
        let voted_heights = {
            let current = state.read().await;
            current.validate()?;
            if current.validators != validators || current.quorum != quorum {
                return Err(
                    "native chain validator configuration does not match persisted genesis"
                        .to_string(),
                );
            }
            current.votes_cast.clone()
        };
        Ok(Self {
            state,
            state_path,
            validators,
            quorum,
            local_validator: encode_public_key_base64(&signing.verifying_key()),
            pending: BTreeMap::new(),
            proposals: BTreeMap::new(),
            votes: BTreeMap::new(),
            orphan_votes: BTreeMap::new(),
            voted_heights,
        })
    }

    pub async fn accept_transaction(&mut self, tx: NativeTransaction) -> Result<bool, String> {
        if self.pending.contains_key(&tx.hash) {
            return Ok(false);
        }
        let state = self.state.read().await;
        if state.transaction(&tx.hash).is_some() {
            return Ok(false);
        }
        validate_transaction(&state, &tx)?;
        let mut expected = state.account(&tx.from).nonce;
        let pending_nonces = self
            .pending
            .values()
            .filter(|item| item.from == tx.from)
            .map(|item| item.nonce)
            .collect::<BTreeSet<_>>();
        while pending_nonces.contains(&expected) {
            expected = expected.saturating_add(1);
        }
        if tx.nonce != expected {
            return Err(format!(
                "nonce mismatch: expected {expected}, received {}",
                tx.nonce
            ));
        }
        drop(state);
        self.pending.insert(tx.hash.clone(), tx);
        Ok(true)
    }

    pub async fn propose(
        &mut self,
        signing: &SigningKey,
    ) -> Result<Option<NativeBlockProposal>, String> {
        if self.pending.is_empty() {
            return Ok(None);
        }
        let state = self.state.read().await;
        let number = state.latest_number().saturating_add(1);
        if expected_leader(&self.validators, number) != self.local_validator {
            return Ok(None);
        }
        if self
            .proposals
            .values()
            .any(|proposal| proposal.number == number)
        {
            return Ok(None);
        }

        let mut transactions = Vec::new();
        let mut working = state.accounts.clone();
        for tx in self.pending.values() {
            if transactions.len() >= MAX_BLOCK_TRANSACTIONS {
                break;
            }
            if apply_transaction_to_accounts(state.chain_id, &mut working, tx).is_ok() {
                transactions.push(tx.clone());
            }
        }
        if transactions.is_empty() {
            return Ok(None);
        }
        let timestamp = now_secs().max(state.latest_timestamp().saturating_add(1));
        let mut proposal = NativeBlockProposal {
            chain_id: state.chain_id,
            number,
            parent_hash: state.latest_hash().to_string(),
            timestamp,
            proposer: self.local_validator.clone(),
            transactions,
            state_root: accounts_root(&working),
            hash: String::new(),
            signature: String::new(),
        };
        proposal.hash = block_hash(&proposal);
        proposal.signature = sign_block_hash(signing, &proposal.hash);
        self.proposals
            .insert(proposal.hash.clone(), proposal.clone());
        Ok(Some(proposal))
    }

    pub async fn handle_message(
        &mut self,
        message: NativeChainMessage,
        signing: &SigningKey,
    ) -> Result<Vec<NativeChainMessage>, String> {
        message.validate_schema()?;
        match message.payload {
            NativeChainMessagePayload::Transaction(tx) => {
                self.accept_transaction(tx).await?;
                Ok(Vec::new())
            }
            NativeChainMessagePayload::Proposal(proposal) => {
                self.handle_proposal(proposal, signing).await
            }
            NativeChainMessagePayload::Vote(vote) => self.handle_vote(vote).await,
            NativeChainMessagePayload::Finalized(block) => {
                self.finalize(block).await?;
                Ok(Vec::new())
            }
            NativeChainMessagePayload::Tip(tip) => self.handle_tip(tip).await,
            NativeChainMessagePayload::SyncRequest(request) => {
                self.handle_sync_request(request).await
            }
            NativeChainMessagePayload::SyncResponse(response) => {
                self.handle_sync_response(response).await
            }
        }
    }

    pub async fn tip_message(&self) -> NativeChainMessage {
        let state = self.state.read().await;
        NativeChainMessage::new(NativeChainMessagePayload::Tip(NativeChainTip {
            height: state.latest_number(),
            hash: state.latest_hash().to_string(),
            observed_at: now_secs(),
        }))
    }

    async fn handle_tip(&self, tip: NativeChainTip) -> Result<Vec<NativeChainMessage>, String> {
        if !is_hash(&tip.hash) {
            return Err("native chain tip contains an invalid hash".to_string());
        }
        let state = self.state.read().await;
        if tip.height == state.latest_number() && tip.hash != state.latest_hash() {
            return Err("native chain tip conflicts at the finalized height".to_string());
        }
        if tip.height <= state.latest_number() {
            return Ok(Vec::new());
        }
        Ok(vec![
            self.sync_request(state.latest_number().saturating_add(1))
        ])
    }

    async fn handle_sync_request(
        &self,
        request: NativeChainSyncRequest,
    ) -> Result<Vec<NativeChainMessage>, String> {
        if request.request_id.len() > 256 {
            return Err("native chain sync request ID is too long".to_string());
        }
        let state = self.state.read().await;
        let Some(block) = state.block_by_number(request.from_height) else {
            return Ok(Vec::new());
        };
        Ok(vec![NativeChainMessage::new(
            NativeChainMessagePayload::SyncResponse(NativeChainSyncResponse {
                request_id: request.request_id,
                block: block.clone(),
            }),
        )])
    }

    async fn handle_sync_response(
        &mut self,
        response: NativeChainSyncResponse,
    ) -> Result<Vec<NativeChainMessage>, String> {
        if response.request_id.len() > 256 {
            return Err("native chain sync response ID is too long".to_string());
        }
        let number = response.block.proposal.number;
        self.finalize(response.block).await?;
        let latest = self.state.read().await.latest_number();
        if number < latest {
            return Ok(Vec::new());
        }
        Ok(vec![self.sync_request(latest.saturating_add(1))])
    }

    fn sync_request(&self, from_height: u64) -> NativeChainMessage {
        NativeChainMessage::new(NativeChainMessagePayload::SyncRequest(
            NativeChainSyncRequest {
                from_height,
                request_id: format!("{}:{from_height}:{}", self.local_validator, now_nanos()),
            },
        ))
    }

    async fn handle_proposal(
        &mut self,
        proposal: NativeBlockProposal,
        signing: &SigningKey,
    ) -> Result<Vec<NativeChainMessage>, String> {
        {
            let state = self.state.read().await;
            if proposal.number <= state.latest_number() {
                if state
                    .block_by_number(proposal.number)
                    .map(|block| block.proposal.hash == proposal.hash)
                    .unwrap_or(false)
                {
                    return Ok(Vec::new());
                }
                return Err("conflicting stale proposal".to_string());
            }
            validate_proposal(&state, &proposal, &self.validators)?;
        }
        if let Some(previous) = self.voted_heights.get(&proposal.number) {
            if previous != &proposal.hash {
                return Err(format!(
                    "refusing conflicting proposal at height {}",
                    proposal.number
                ));
            }
            return Ok(Vec::new());
        }
        self.proposals
            .insert(proposal.hash.clone(), proposal.clone());
        if let Some(orphaned) = self.orphan_votes.remove(&proposal.hash) {
            let votes = self.votes.entry(proposal.hash.clone()).or_default();
            for vote in orphaned.into_values() {
                validate_vote(&vote, &proposal, &self.validators)?;
                votes.insert(vote.validator.clone(), vote);
            }
        }
        if !self.validators.contains(&self.local_validator) {
            return Ok(Vec::new());
        }
        {
            let mut state = self.state.write().await;
            if let Some(previous) = state.votes_cast.get(&proposal.number) {
                if previous != &proposal.hash {
                    return Err(format!(
                        "persistent vote lock rejects conflicting proposal at height {}",
                        proposal.number
                    ));
                }
                return Ok(Vec::new());
            }
            state
                .votes_cast
                .insert(proposal.number, proposal.hash.clone());
            save_state_atomic(&self.state_path, &state)?;
        }
        let vote = NativeBlockVote {
            block_hash: proposal.hash.clone(),
            block_number: proposal.number,
            validator: self.local_validator.clone(),
            signature: sign_vote(signing, &proposal.hash, proposal.number),
        };
        self.voted_heights
            .insert(proposal.number, proposal.hash.clone());
        self.votes
            .entry(proposal.hash)
            .or_default()
            .insert(vote.validator.clone(), vote.clone());
        Ok(vec![NativeChainMessage::new(
            NativeChainMessagePayload::Vote(vote),
        )])
    }

    async fn handle_vote(
        &mut self,
        vote: NativeBlockVote,
    ) -> Result<Vec<NativeChainMessage>, String> {
        let proposal = self.proposals.get(&vote.block_hash).cloned();
        let Some(proposal) = proposal else {
            {
                let state = self.state.read().await;
                if vote.block_number <= state.latest_number() {
                    return Ok(Vec::new());
                }
            }
            validate_vote_envelope(&vote, &self.validators)?;
            self.orphan_votes
                .entry(vote.block_hash.clone())
                .or_default()
                .insert(vote.validator.clone(), vote);
            return Ok(Vec::new());
        };
        validate_vote(&vote, &proposal, &self.validators)?;
        let votes = self.votes.entry(vote.block_hash.clone()).or_default();
        votes.insert(vote.validator.clone(), vote);
        if votes.len() < self.quorum {
            return Ok(Vec::new());
        }
        let finalized = FinalizedNativeBlock {
            proposal,
            votes: votes.values().cloned().collect(),
        };
        self.finalize(finalized.clone()).await?;
        Ok(vec![NativeChainMessage::new(
            NativeChainMessagePayload::Finalized(finalized),
        )])
    }

    async fn finalize(&mut self, block: FinalizedNativeBlock) -> Result<(), String> {
        let mut state = self.state.write().await;
        if block.proposal.number <= state.latest_number() {
            if state
                .blocks
                .iter()
                .any(|existing| existing.proposal.hash == block.proposal.hash)
            {
                return Ok(());
            }
            return Err("conflicting finalized block below current height".to_string());
        }
        validate_finalized(&state, &block, &self.validators, self.quorum)?;
        let mut next_accounts = state.accounts.clone();
        for tx in &block.proposal.transactions {
            apply_transaction_to_accounts(state.chain_id, &mut next_accounts, tx)?;
        }
        state.accounts = next_accounts;
        state.blocks.push(block.clone());
        state
            .votes_cast
            .retain(|number, _| *number > block.proposal.number);
        save_state_atomic(&self.state_path, &state)?;
        drop(state);

        for tx in &block.proposal.transactions {
            self.pending.remove(&tx.hash);
        }
        let height = block.proposal.number;
        self.proposals
            .retain(|_, proposal| proposal.number > height);
        self.votes
            .retain(|hash, _| self.proposals.contains_key(hash));
        self.orphan_votes
            .retain(|_, votes| votes.values().any(|vote| vote.block_number > height));
        self.voted_heights.retain(|number, _| *number > height);
        Ok(())
    }
}

impl NativeChainState {
    pub fn load_or_initialize(
        path: &Path,
        chain_id: u64,
        registry_path: Option<&Path>,
        mut validators: Vec<String>,
        quorum: usize,
    ) -> Result<Self, String> {
        validators.sort();
        validators.dedup();
        if validators.is_empty()
            || quorum == 0
            || quorum > validators.len()
            || quorum.saturating_mul(2) <= validators.len()
        {
            return Err("native chain requires a strict-majority validator quorum".to_string());
        }
        if path.exists() {
            let bytes = fs::read(path).map_err(|err| err.to_string())?;
            let state: Self = serde_json::from_slice(&bytes).map_err(|err| err.to_string())?;
            state.validate()?;
            if state.chain_id != chain_id {
                return Err(format!(
                    "native chain ID mismatch: state={} configured={chain_id}",
                    state.chain_id
                ));
            }
            if state.validators != validators || state.quorum != quorum {
                return Err(
                    "configured validator set or quorum differs from native-chain genesis"
                        .to_string(),
                );
            }
            return Ok(state);
        }

        let mut accounts = BTreeMap::new();
        if let Some(registry_path) = registry_path {
            let registry = StakeRegistry::load(registry_path)?;
            for (key, account) in registry.accounts() {
                if let Some(address) = registry_key_to_evm_address(key) {
                    let entry = accounts
                        .entry(address)
                        .or_insert_with(NativeAccount::default);
                    entry.balance = entry.balance.saturating_add(account.balance);
                }
            }
        }
        let genesis = genesis_block(chain_id, &accounts, &validators, quorum);
        let state = Self {
            schema: STATE_SCHEMA.to_string(),
            chain_id,
            validators,
            quorum,
            genesis_accounts: accounts.clone(),
            accounts,
            blocks: vec![genesis],
            votes_cast: BTreeMap::new(),
        };
        save_state_atomic(path, &state)?;
        Ok(state)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != STATE_SCHEMA {
            return Err(format!(
                "unsupported native chain state schema: {}",
                self.schema
            ));
        }
        if self.blocks.is_empty() {
            return Err("native chain state has no genesis block".to_string());
        }
        if self.blocks[0].proposal.number != 0 {
            return Err("native chain genesis block number must be zero".to_string());
        }
        if self.validators.is_empty()
            || self.quorum == 0
            || self.quorum > self.validators.len()
            || self.quorum.saturating_mul(2) <= self.validators.len()
        {
            return Err("native chain validator quorum is invalid".to_string());
        }
        if self.blocks[0]
            != genesis_block(
                self.chain_id,
                &self.genesis_accounts,
                &self.validators,
                self.quorum,
            )
        {
            return Err("native chain genesis commitment is corrupt".to_string());
        }
        let mut replay = Self {
            schema: self.schema.clone(),
            chain_id: self.chain_id,
            validators: self.validators.clone(),
            quorum: self.quorum,
            genesis_accounts: self.genesis_accounts.clone(),
            accounts: self.genesis_accounts.clone(),
            blocks: vec![self.blocks[0].clone()],
            votes_cast: BTreeMap::new(),
        };
        for block in self.blocks.iter().skip(1) {
            validate_finalized(&replay, block, &self.validators, self.quorum)?;
            for tx in &block.proposal.transactions {
                apply_transaction_to_accounts(self.chain_id, &mut replay.accounts, tx)?;
            }
            replay.blocks.push(block.clone());
        }
        if replay.accounts != self.accounts {
            return Err("native chain account state does not match replayed blocks".to_string());
        }
        Ok(())
    }

    pub fn account(&self, address: &str) -> NativeAccount {
        normalize_evm_address(address)
            .and_then(|normalized| self.accounts.get(&normalized).cloned())
            .unwrap_or_default()
    }

    pub fn account_at(&self, address: &str, number: u64) -> Result<NativeAccount, String> {
        if number > self.latest_number() {
            return Err(format!("block {number} has not been finalized"));
        }
        let normalized =
            normalize_evm_address(address).ok_or_else(|| "invalid address format".to_string())?;
        let mut accounts = self.genesis_accounts.clone();
        for block in self.blocks.iter().skip(1).take(number as usize) {
            for tx in &block.proposal.transactions {
                apply_transaction_to_accounts(self.chain_id, &mut accounts, tx)?;
            }
        }
        Ok(accounts.get(&normalized).cloned().unwrap_or_default())
    }

    pub fn latest_block(&self) -> &FinalizedNativeBlock {
        self.blocks.last().expect("validated state has genesis")
    }

    pub fn latest_number(&self) -> u64 {
        self.latest_block().proposal.number
    }

    pub fn latest_hash(&self) -> &str {
        &self.latest_block().proposal.hash
    }

    pub fn latest_timestamp(&self) -> u64 {
        self.latest_block().proposal.timestamp
    }

    pub fn block_by_number(&self, number: u64) -> Option<&FinalizedNativeBlock> {
        self.blocks.get(number as usize)
    }

    pub fn block_by_hash(&self, hash: &str) -> Option<&FinalizedNativeBlock> {
        self.blocks
            .iter()
            .find(|block| block.proposal.hash.eq_ignore_ascii_case(hash))
    }

    pub fn transaction(
        &self,
        hash: &str,
    ) -> Option<(&FinalizedNativeBlock, usize, &NativeTransaction)> {
        self.blocks.iter().find_map(|block| {
            block
                .proposal
                .transactions
                .iter()
                .enumerate()
                .find(|(_, tx)| tx.hash.eq_ignore_ascii_case(hash))
                .map(|(index, tx)| (block, index, tx))
        })
    }
}

pub fn decode_eip1559_transaction(
    raw: &[u8],
    expected_chain_id: u64,
) -> Result<NativeTransaction, String> {
    if raw.first().copied() != Some(0x02) {
        return Err("only EIP-1559 type 0x02 transactions are supported".to_string());
    }
    let rlp = Rlp::new(&raw[1..]);
    if !rlp.is_list() || rlp.item_count().map_err(|err| err.to_string())? != 12 {
        return Err("invalid EIP-1559 RLP payload".to_string());
    }
    let chain_id = rlp_u64(&rlp, 0)?;
    if chain_id != expected_chain_id {
        return Err(format!(
            "chainId mismatch: transaction={chain_id} configured={expected_chain_id}"
        ));
    }
    let nonce = rlp_u64(&rlp, 1)?;
    let max_priority_fee_per_gas = rlp_u128(&rlp, 2)?;
    let max_fee_per_gas = rlp_u128(&rlp, 3)?;
    let gas_limit = rlp_u64(&rlp, 4)?;
    if gas_limit < NATIVE_GAS_LIMIT {
        return Err(format!("gas limit must be at least {NATIVE_GAS_LIMIT}"));
    }
    let to_bytes = rlp_data(&rlp, 5)?;
    if to_bytes.len() != 20 {
        return Err("contract creation is not supported".to_string());
    }
    let value_wei = rlp_u128(&rlp, 6)?;
    let value_units = wei_to_native_units(value_wei)?;
    let input_bytes = rlp_data(&rlp, 7)?;
    if !input_bytes.is_empty() {
        return Err("contract calldata is not supported".to_string());
    }
    let access_list_item = rlp.at(8).map_err(|err| err.to_string())?;
    if !access_list_item.is_list() {
        return Err("EIP-1559 access list must be an RLP list".to_string());
    }
    let access_list = access_list_item.as_raw().to_vec();
    let y_parity = rlp_u64(&rlp, 9)?;
    if y_parity > 1 {
        return Err("invalid signature recovery parity".to_string());
    }
    let r32 = left_pad_32(rlp_data(&rlp, 10)?)?;
    let s32 = left_pad_32(rlp_data(&rlp, 11)?)?;

    let mut stream = RlpStream::new_list(9);
    stream.append(&chain_id);
    stream.append(&nonce);
    stream.append(&max_priority_fee_per_gas);
    stream.append(&max_fee_per_gas);
    stream.append(&gas_limit);
    stream.append(&to_bytes);
    stream.append(&value_wei);
    stream.append(&input_bytes);
    stream.append_raw(&access_list, 1);
    let encoded = stream.out();
    let mut signing_payload = Vec::with_capacity(encoded.len() + 1);
    signing_payload.push(0x02);
    signing_payload.extend_from_slice(encoded.as_ref());
    let sighash = keccak256(&signing_payload);
    let signature = Signature::from_scalars(r32, s32).map_err(|err| err.to_string())?;
    if signature.normalize_s().is_some() {
        return Err("signature must use canonical low-S form".to_string());
    }
    let recovery_id =
        RecoveryId::from_byte(y_parity as u8).ok_or_else(|| "invalid recovery ID".to_string())?;
    let verifying = VerifyingKey::recover_from_prehash(&sighash, &signature, recovery_id)
        .map_err(|err| format!("signature recovery failed: {err}"))?;
    let encoded_public = verifying.to_encoded_point(false);
    let public = encoded_public.as_bytes();
    if public.len() != 65 {
        return Err("invalid recovered public key".to_string());
    }
    let address_hash = keccak256(&public[1..]);

    Ok(NativeTransaction {
        hash: format!("0x{}", hex::encode(keccak256(raw))),
        raw: format!("0x{}", hex::encode(raw)),
        from: format!("0x{}", hex::encode(&address_hash[12..])),
        to: format!("0x{}", hex::encode(to_bytes)),
        nonce,
        value_wei: to_quantity_u128(value_wei),
        value_units,
        gas_limit,
        max_fee_per_gas: to_quantity_u128(max_fee_per_gas),
        max_priority_fee_per_gas: to_quantity_u128(max_priority_fee_per_gas),
        input: "0x".to_string(),
        y_parity: y_parity as u8,
        r: format!("0x{}", hex::encode(r32)),
        s: format!("0x{}", hex::encode(s32)),
    })
}

pub fn decode_hex_prefixed(input: &str) -> Result<Vec<u8>, String> {
    let raw = input
        .trim()
        .strip_prefix("0x")
        .or_else(|| input.trim().strip_prefix("0X"))
        .ok_or_else(|| "hex payload must start with 0x".to_string())?;
    hex::decode(raw).map_err(|err| format!("invalid hex payload: {err}"))
}

pub fn normalize_evm_address(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))?;
    if raw.len() != 40 || !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", raw.to_ascii_lowercase()))
}

pub fn to_quantity_u64(value: u64) -> String {
    format!("0x{value:x}")
}

pub fn to_quantity_u128(value: u128) -> String {
    format!("0x{value:x}")
}

fn validate_transaction(state: &NativeChainState, tx: &NativeTransaction) -> Result<(), String> {
    let raw = decode_hex_prefixed(&tx.raw)?;
    let decoded = decode_eip1559_transaction(&raw, state.chain_id)?;
    if &decoded != tx {
        return Err("transaction fields do not match signed raw payload".to_string());
    }
    let sender = state.account(&tx.from);
    if sender.nonce != tx.nonce {
        return Err(format!(
            "nonce mismatch: expected {}, received {}",
            sender.nonce, tx.nonce
        ));
    }
    if sender.balance < tx.value_units {
        return Err("insufficient native balance".to_string());
    }
    Ok(())
}

fn apply_transaction_to_accounts(
    chain_id: u64,
    accounts: &mut BTreeMap<String, NativeAccount>,
    tx: &NativeTransaction,
) -> Result<(), String> {
    let raw = decode_hex_prefixed(&tx.raw)?;
    if decode_eip1559_transaction(&raw, chain_id)? != *tx {
        return Err("transaction fields do not match signed raw payload".to_string());
    }
    let from = normalize_evm_address(&tx.from).ok_or_else(|| "invalid sender".to_string())?;
    let to = normalize_evm_address(&tx.to).ok_or_else(|| "invalid recipient".to_string())?;
    let sender = accounts.entry(from.clone()).or_default();
    if sender.nonce != tx.nonce {
        return Err(format!(
            "nonce mismatch: expected {}, received {}",
            sender.nonce, tx.nonce
        ));
    }
    if sender.balance < tx.value_units {
        return Err("insufficient native balance".to_string());
    }
    sender.balance -= tx.value_units;
    sender.nonce = sender.nonce.saturating_add(1);
    let recipient = accounts.entry(to).or_default();
    recipient.balance = recipient.balance.saturating_add(tx.value_units);
    Ok(())
}

fn validate_proposal(
    state: &NativeChainState,
    proposal: &NativeBlockProposal,
    validators: &[String],
) -> Result<(), String> {
    if proposal.chain_id != state.chain_id
        || proposal.number != state.latest_number().saturating_add(1)
        || proposal.parent_hash != state.latest_hash()
    {
        return Err("proposal does not extend the finalized tip".to_string());
    }
    if proposal.transactions.is_empty() || proposal.transactions.len() > MAX_BLOCK_TRANSACTIONS {
        return Err("proposal transaction count is invalid".to_string());
    }
    if proposal.timestamp <= state.latest_timestamp()
        || proposal.timestamp > now_secs().saturating_add(MAX_FUTURE_SECONDS)
    {
        return Err("proposal timestamp is outside the accepted range".to_string());
    }
    if expected_leader(validators, proposal.number) != proposal.proposer {
        return Err("proposal was not signed by the expected leader".to_string());
    }
    if block_hash(proposal) != proposal.hash {
        return Err("proposal hash mismatch".to_string());
    }
    verify_signature_base64(
        &proposal.proposer,
        block_signing_payload(&proposal.hash).as_bytes(),
        &proposal.signature,
    )
    .map_err(|err| format!("invalid proposer signature: {err}"))?;

    let mut accounts = state.accounts.clone();
    let mut hashes = BTreeSet::new();
    for tx in &proposal.transactions {
        if !hashes.insert(tx.hash.clone()) {
            return Err("proposal contains duplicate transactions".to_string());
        }
        apply_transaction_to_accounts(state.chain_id, &mut accounts, tx)?;
    }
    if accounts_root(&accounts) != proposal.state_root {
        return Err("proposal state root does not match transaction execution".to_string());
    }
    Ok(())
}

fn validate_vote(
    vote: &NativeBlockVote,
    proposal: &NativeBlockProposal,
    validators: &[String],
) -> Result<(), String> {
    if vote.block_hash != proposal.hash || vote.block_number != proposal.number {
        return Err("vote does not match proposal".to_string());
    }
    validate_vote_envelope(vote, validators)
}

fn validate_vote_envelope(vote: &NativeBlockVote, validators: &[String]) -> Result<(), String> {
    if !validators.contains(&vote.validator) {
        return Err("vote signer is not a validator".to_string());
    }
    if !is_hash(&vote.block_hash) {
        return Err("vote contains an invalid block hash".to_string());
    }
    verify_signature_base64(
        &vote.validator,
        vote_signing_payload(&vote.block_hash, vote.block_number).as_bytes(),
        &vote.signature,
    )
    .map_err(|err| format!("invalid block vote: {err}"))
}

fn is_hash(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("0x")
        && value[2..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_finalized(
    state: &NativeChainState,
    block: &FinalizedNativeBlock,
    validators: &[String],
    quorum: usize,
) -> Result<(), String> {
    validate_proposal(state, &block.proposal, validators)?;
    let mut signers = BTreeSet::new();
    for vote in &block.votes {
        validate_vote(vote, &block.proposal, validators)?;
        signers.insert(vote.validator.clone());
    }
    if signers.len() < quorum {
        return Err(format!(
            "quorum certificate has {} distinct votes; {quorum} required",
            signers.len()
        ));
    }
    Ok(())
}

fn genesis_block(
    chain_id: u64,
    accounts: &BTreeMap<String, NativeAccount>,
    validators: &[String],
    quorum: usize,
) -> FinalizedNativeBlock {
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-native-genesis-v1");
    hasher.update(chain_id.to_be_bytes());
    hasher.update(accounts_root(accounts).as_bytes());
    hasher.update(serde_json::to_vec(validators).expect("validators serialize"));
    hasher.update(quorum.to_be_bytes());
    let hash: [u8; 32] = hasher.finalize().into();
    FinalizedNativeBlock {
        proposal: NativeBlockProposal {
            chain_id,
            number: 0,
            parent_hash: format!("0x{}", "00".repeat(32)),
            timestamp: 0,
            proposer: "genesis".to_string(),
            transactions: Vec::new(),
            state_root: accounts_root(accounts),
            hash: format!("0x{}", hex::encode(hash)),
            signature: String::new(),
        },
        votes: Vec::new(),
    }
}

fn block_hash(proposal: &NativeBlockProposal) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-native-block-v1");
    hasher.update(proposal.chain_id.to_be_bytes());
    hasher.update(proposal.number.to_be_bytes());
    hasher.update(proposal.parent_hash.as_bytes());
    hasher.update(proposal.timestamp.to_be_bytes());
    hasher.update(proposal.proposer.as_bytes());
    hasher.update(proposal.state_root.as_bytes());
    for tx in &proposal.transactions {
        hasher.update(tx.hash.as_bytes());
    }
    let digest: [u8; 32] = hasher.finalize().into();
    format!("0x{}", hex::encode(digest))
}

fn accounts_root(accounts: &BTreeMap<String, NativeAccount>) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-native-state-root-v1");
    hasher.update(serde_json::to_vec(accounts).expect("native accounts serialize"));
    let digest: [u8; 32] = hasher.finalize().into();
    format!("0x{}", hex::encode(digest))
}

fn expected_leader(validators: &[String], number: u64) -> String {
    validators[((number.saturating_sub(1)) as usize) % validators.len()].clone()
}

fn sign_block_hash(signing: &SigningKey, hash: &str) -> String {
    encode_signature_base64(&signing.sign(block_signing_payload(hash).as_bytes()))
}

fn sign_vote(signing: &SigningKey, hash: &str, number: u64) -> String {
    encode_signature_base64(&signing.sign(vote_signing_payload(hash, number).as_bytes()))
}

fn block_signing_payload(hash: &str) -> String {
    format!("mfenx-native-block-signature-v1:{hash}")
}

fn vote_signing_payload(hash: &str, number: u64) -> String {
    format!("mfenx-native-block-vote-v1:{number}:{hash}")
}

fn save_state_atomic(path: &Path, state: &NativeChainState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(state).map_err(|err| err.to_string())?;
    let temp = path.with_extension(format!("tmp-{}-{}", std::process::id(), now_nanos()));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|err| err.to_string())?;
        file.write_all(&bytes).map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
        fs::rename(&temp, path).map_err(|err| err.to_string())?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn registry_key_to_evm_address(key: &str) -> Option<String> {
    if let Some(address) = normalize_evm_address(key) {
        return Some(address);
    }
    let public = BASE64.decode(key).ok()?;
    let _ = decode_public_key_base64(key).ok()?;
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-migration-address-v1");
    hasher.update(public);
    let digest: [u8; 32] = hasher.finalize().into();
    Some(format!("0x{}", hex::encode(&digest[12..])))
}

fn wei_to_native_units(value: u128) -> Result<u64, String> {
    if !value.is_multiple_of(NATIVE_DECIMAL_FACTOR) {
        return Err("value must be aligned to whole native tokens".to_string());
    }
    u64::try_from(value / NATIVE_DECIMAL_FACTOR)
        .map_err(|_| "native transfer value exceeds u64 capacity".to_string())
}

fn rlp_data<'a>(rlp: &'a Rlp<'a>, index: usize) -> Result<&'a [u8], String> {
    rlp.at(index)
        .map_err(|err| err.to_string())?
        .data()
        .map_err(|err| err.to_string())
}

fn rlp_u64(rlp: &Rlp<'_>, index: usize) -> Result<u64, String> {
    let bytes = rlp_data(rlp, index)?;
    if bytes.len() > 8 {
        return Err("RLP integer exceeds u64".to_string());
    }
    Ok(bytes
        .iter()
        .fold(0u64, |value, byte| (value << 8) | u64::from(*byte)))
}

fn rlp_u128(rlp: &Rlp<'_>, index: usize) -> Result<u128, String> {
    let bytes = rlp_data(rlp, index)?;
    if bytes.len() > 16 {
        return Err("RLP integer exceeds u128".to_string());
    }
    Ok(bytes
        .iter()
        .fold(0u128, |value, byte| (value << 8) | u128::from(*byte)))
}

fn left_pad_32(bytes: &[u8]) -> Result<[u8; 32], String> {
    if bytes.len() > 32 {
        return Err("signature scalar exceeds 32 bytes".to_string());
    }
    let mut output = [0u8; 32];
    output[32 - bytes.len()..].copy_from_slice(bytes);
    Ok(output)
}

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
pub(crate) fn signed_test_transfer(
    secret: [u8; 32],
    chain_id: u64,
    nonce: u64,
    to: [u8; 20],
    units: u64,
) -> NativeTransaction {
    use k256::ecdsa::SigningKey as SecpSigningKey;

    let signing = SecpSigningKey::from_bytes((&secret).into()).unwrap();
    let value = u128::from(units) * NATIVE_DECIMAL_FACTOR;
    let mut unsigned = RlpStream::new_list(9);
    unsigned.append(&chain_id);
    unsigned.append(&nonce);
    unsigned.append(&100_000_000u64);
    unsigned.append(&NATIVE_GAS_PRICE);
    unsigned.append(&NATIVE_GAS_LIMIT);
    unsigned.append(&to.as_slice());
    unsigned.append(&value);
    unsigned.append(&&[][..]);
    unsigned.begin_list(0);
    let encoded = unsigned.out();
    let mut payload = vec![0x02];
    payload.extend_from_slice(&encoded);
    let digest = keccak256(&payload);
    let (signature, recovery) = signing.sign_prehash_recoverable(&digest).unwrap();

    let mut signed = RlpStream::new_list(12);
    signed.append(&chain_id);
    signed.append(&nonce);
    signed.append(&100_000_000u64);
    signed.append(&NATIVE_GAS_PRICE);
    signed.append(&NATIVE_GAS_LIMIT);
    signed.append(&to.as_slice());
    signed.append(&value);
    signed.append(&&[][..]);
    signed.begin_list(0);
    signed.append(&u8::from(recovery));
    signed.append(&signature.r().to_bytes().as_slice());
    signed.append(&signature.s().to_bytes().as_slice());
    let mut raw = vec![0x02];
    raw.extend_from_slice(&signed.out());
    decode_eip1559_transaction(&raw, chain_id).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{load_or_derive_keypair, Ed25519KeySource};

    fn validator(seed: &str) -> crate::net::KeyMaterial {
        load_or_derive_keypair(&Ed25519KeySource::Seed(seed.to_string())).unwrap()
    }

    #[tokio::test]
    async fn replicas_apply_identical_quorum_finalized_block() {
        let chain_id = 177155;
        let tx = signed_test_transfer([7u8; 32], chain_id, 0, [9u8; 20], 2);
        let validators = [validator("a"), validator("b"), validator("c")];
        let validator_ids = validators
            .iter()
            .map(|key| encode_public_key_base64(&key.verifying))
            .collect::<Vec<_>>();
        let mut accounts = BTreeMap::new();
        accounts.insert(
            tx.from.clone(),
            NativeAccount {
                balance: 5,
                nonce: 0,
            },
        );
        let base = NativeChainState {
            schema: STATE_SCHEMA.to_string(),
            chain_id,
            validators: validator_ids.clone(),
            quorum: 2,
            genesis_accounts: accounts.clone(),
            accounts: accounts.clone(),
            blocks: vec![genesis_block(chain_id, &accounts, &validator_ids, 2)],
            votes_cast: BTreeMap::new(),
        };
        let root = std::env::temp_dir().join(format!("native_chain_test_{}", now_nanos()));
        fs::create_dir_all(&root).unwrap();
        let mut runtimes = Vec::new();
        for (index, key) in validators.iter().enumerate() {
            let state = Arc::new(RwLock::new(base.clone()));
            runtimes.push(
                NativeChainRuntime::new(
                    state,
                    root.join(format!("state-{index}.json")),
                    validator_ids.clone(),
                    2,
                    &key.signing,
                )
                .await
                .unwrap(),
            );
        }
        for runtime in &mut runtimes {
            runtime.accept_transaction(tx.clone()).await.unwrap();
        }
        let proposal = runtimes[0]
            .propose(&validators[0].signing)
            .await
            .unwrap()
            .unwrap();
        let mut votes = Vec::new();
        for index in 0..2 {
            let messages = runtimes[index]
                .handle_message(
                    NativeChainMessage::new(NativeChainMessagePayload::Proposal(proposal.clone())),
                    &validators[index].signing,
                )
                .await
                .unwrap();
            if let NativeChainMessagePayload::Vote(vote) = messages[0].payload.clone() {
                votes.push(vote);
            }
        }
        let finalized = FinalizedNativeBlock { proposal, votes };
        for runtime in runtimes.iter_mut().take(2) {
            runtime
                .handle_message(
                    NativeChainMessage::new(NativeChainMessagePayload::Finalized(
                        finalized.clone(),
                    )),
                    &validators[0].signing,
                )
                .await
                .unwrap();
        }
        let tip = runtimes[0].tip_message().await;
        let requests = runtimes[2]
            .handle_message(tip, &validators[2].signing)
            .await
            .unwrap();
        let mut responses = Vec::new();
        for request in requests {
            responses.extend(
                runtimes[0]
                    .handle_message(request, &validators[0].signing)
                    .await
                    .unwrap(),
            );
        }
        for response in responses {
            runtimes[2]
                .handle_message(response, &validators[2].signing)
                .await
                .unwrap();
        }
        let hashes = futures::future::join_all(
            runtimes
                .iter()
                .map(|runtime| async { runtime.state.read().await.latest_hash().to_string() }),
        )
        .await;
        assert!(hashes.windows(2).all(|pair| pair[0] == pair[1]));
        for runtime in &runtimes {
            let state = runtime.state.read().await;
            assert_eq!(state.account(&tx.from).balance, 3);
            assert_eq!(state.account(&tx.to).balance, 2);
            assert_eq!(state.account(&tx.from).nonce, 1);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
