// Settlement Module — L3 → zkSync Era (L2) → Ethereum (L1)
//
// Submits proven L3 block proofs to COLDL3Settlement.sol on zkSync Era.
//
//   1. Prover generates SP1 proof for an L3 block
//   2. Node verifies proof and records ProvenBlock
//   3. Settlement module batches proven blocks
//   4. Signs + sends settleBatch(publicValues, proofBytes) to Era
//   5. Contract verifies via SP1VerifierGateway and commits state root
//
// Two modes:
//   - Live: sequencer_key set → signs real txs to Era Sepolia/Mainnet
//   - Mock: no key → logs calldata, returns synthetic tx hash (testnet dev)

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use ethers::prelude::*;
use ethers::types::{TransactionRequest, Bytes, H160};
use ethers::utils::keccak256;
use std::sync::Arc;

// ── Settlement Batch ───────────────────────────────────────────────────────

/// A batch of proven blocks ready for L2 settlement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementBatch {
    /// Sequential batch ID.
    pub batch_id: u64,
    /// Block heights included in this batch.
    pub block_heights: Vec<u64>,
    /// SP1 proof bytes (Groth16/PLONK — on-chain verifiable).
    pub proof_bytes: Vec<u8>,
    /// BlockExecutionClaim public values (152 bytes per block).
    pub public_values: Vec<u8>,
    /// Previous state root (before first block in batch).
    pub prev_state_root: [u8; 32],
    /// New state root (after last block in batch).
    pub new_state_root: [u8; 32],
    /// Settlement status.
    pub status: SettlementStatus,
    /// L2 transaction hash (set after submission).
    pub l2_tx_hash: Option<String>,
    /// Timestamp of batch creation.
    pub created_at: u64,
    /// Timestamp of settlement confirmation.
    pub settled_at: Option<u64>,
}

/// Settlement batch status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettlementStatus {
    Pending,
    Submitted,
    Settled,
    Failed,
}

impl std::fmt::Display for SettlementStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementStatus::Pending => write!(f, "pending"),
            SettlementStatus::Submitted => write!(f, "submitted"),
            SettlementStatus::Settled => write!(f, "settled"),
            SettlementStatus::Failed => write!(f, "failed"),
        }
    }
}

// ── Settlement Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementModuleConfig {
    /// zkSync Era RPC URL.
    pub era_rpc_url: String,
    /// COLDL3Settlement contract address on zkSync Era (hex).
    pub settlement_contract: String,
    /// Sequencer private key for signing settlement txs (hex-encoded, no 0x prefix).
    /// If empty/None, settlement runs in mock mode.
    pub sequencer_key: Option<String>,
    /// Maximum blocks per settlement batch.
    pub max_batch_size: u64,
    /// Settlement interval in seconds.
    pub interval_secs: u64,
    /// Chain ID of the target L2 (Era Sepolia = 300, Era Mainnet = 324).
    pub era_chain_id: u64,
}

impl Default for SettlementModuleConfig {
    fn default() -> Self {
        Self {
            era_rpc_url: "https://sepolia.era.zksync.dev".to_string(),
            settlement_contract: String::new(),
            max_batch_size: 10,
            interval_secs: 30,
            sequencer_key: None,
            era_chain_id: 300, // Era Sepolia
        }
    }
}

// ── Settlement Manager ─────────────────────────────────────────────────────

pub struct SettlementManager {
    config: SettlementModuleConfig,
    batches: std::collections::HashMap<u64, SettlementBatch>,
    next_batch_id: u64,
    last_settled_height: u64,
}

impl SettlementManager {
    pub fn new(config: SettlementModuleConfig) -> Self {
        let mode = if config.sequencer_key.is_some() && !config.settlement_contract.is_empty() {
            "LIVE"
        } else {
            "MOCK"
        };
        info!(
            "Settlement manager initialized [{}] → {} (chain {})",
            mode, config.era_rpc_url, config.era_chain_id,
        );

        Self {
            config,
            batches: std::collections::HashMap::new(),
            next_batch_id: 1,
            last_settled_height: 0,
        }
    }

    /// Whether real L2 submission is enabled (key + contract configured).
    pub fn is_live(&self) -> bool {
        self.config.sequencer_key.is_some() && !self.config.settlement_contract.is_empty()
    }

    /// Create a settlement batch from proven blocks.
    pub fn create_batch(
        &mut self,
        block_heights: Vec<u64>,
        proof_bytes: Vec<u8>,
        public_values: Vec<u8>,
        prev_state_root: [u8; 32],
        new_state_root: [u8; 32],
    ) -> Result<u64> {
        if block_heights.is_empty() {
            return Err(anyhow!("Cannot create empty settlement batch"));
        }

        let first = *block_heights.first().unwrap();
        if first <= self.last_settled_height && self.last_settled_height > 0 {
            return Err(anyhow!(
                "Block {} already settled (last settled: {})",
                first, self.last_settled_height,
            ));
        }

        let batch_id = self.next_batch_id;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let batch = SettlementBatch {
            batch_id,
            block_heights,
            proof_bytes,
            public_values,
            prev_state_root,
            new_state_root,
            status: SettlementStatus::Pending,
            l2_tx_hash: None,
            created_at: now,
            settled_at: None,
        };

        info!(
            "Settlement batch {} created: blocks {:?}, prev_root: 0x{}..., new_root: 0x{}...",
            batch_id,
            &batch.block_heights,
            hex::encode(&prev_state_root[..4]),
            hex::encode(&new_state_root[..4]),
        );

        self.batches.insert(batch_id, batch);
        self.next_batch_id += 1;

        Ok(batch_id)
    }

    /// Submit a pending batch to COLDL3Settlement on zkSync Era.
    ///
    /// In live mode: signs and sends a real transaction via ethers.
    /// In mock mode: generates a synthetic tx hash (for testnet without Era deployment).
    pub async fn submit_batch(&mut self, batch_id: u64) -> Result<String> {
        let batch = self.batches.get(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Pending {
            return Err(anyhow!(
                "Batch {} is not pending (status: {})",
                batch_id, batch.status,
            ));
        }

        // Build calldata
        let selector = settle_batch_selector();
        let calldata = encode_settle_batch_calldata(
            &selector,
            &batch.public_values,
            &batch.proof_bytes,
        );

        debug!(
            "Settlement calldata: {} bytes (selector: 0x{})",
            calldata.len(),
            hex::encode(&selector),
        );

        let tx_hash = if self.is_live() {
            self.submit_live(batch_id, calldata).await?
        } else {
            self.submit_mock(batch_id, &calldata)
        };

        // Update batch status
        let batch = self.batches.get_mut(&batch_id).unwrap();
        batch.status = SettlementStatus::Submitted;
        batch.l2_tx_hash = Some(tx_hash.clone());

        Ok(tx_hash)
    }

    /// Live submission: sign + send transaction to Era via ethers.
    async fn submit_live(&self, batch_id: u64, calldata: Vec<u8>) -> Result<String> {
        let key_hex = self.config.sequencer_key.as_ref()
            .ok_or_else(|| anyhow!("sequencer_key required for live settlement"))?;

        let contract_addr: H160 = self.config.settlement_contract.parse()
            .map_err(|e| anyhow!("invalid settlement_contract address: {}", e))?;

        // Connect to Era
        let provider = Provider::<Http>::try_from(&self.config.era_rpc_url)
            .map_err(|e| anyhow!("Era RPC connection failed: {}", e))?;

        // Build signer
        let wallet: LocalWallet = key_hex.parse::<LocalWallet>()
            .map_err(|e| anyhow!("invalid sequencer_key: {}", e))?
            .with_chain_id(self.config.era_chain_id);

        let client = SignerMiddleware::new(provider, wallet);

        // Build transaction
        let tx = TransactionRequest::new()
            .to(contract_addr)
            .data(Bytes::from(calldata));

        info!(
            "Submitting batch {} to Era (contract: {}, chain: {})",
            batch_id, self.config.settlement_contract, self.config.era_chain_id,
        );

        // Send and wait for hash (not receipt — that happens in confirm step)
        let pending_tx = client.send_transaction(tx, None).await
            .map_err(|e| {
                error!("Era tx submission failed for batch {}: {}", batch_id, e);
                anyhow!("Era submission failed: {}", e)
            })?;

        let tx_hash = format!("0x{}", hex::encode(pending_tx.tx_hash().as_bytes()));

        info!(
            "Settlement batch {} submitted to Era: {}",
            batch_id, tx_hash,
        );

        Ok(tx_hash)
    }

    /// Mock submission: log calldata, return synthetic tx hash.
    fn submit_mock(&self, batch_id: u64, calldata: &[u8]) -> String {
        let hash = format!(
            "0x{:064x}",
            batch_id as u128 * 1_000_000 + self.batches.get(&batch_id)
                .and_then(|b| b.block_heights.last().copied())
                .unwrap_or(0) as u128
        );

        info!(
            "Settlement batch {} submitted [MOCK] (calldata: {} bytes, hash: {})",
            batch_id, calldata.len(), hash,
        );

        hash
    }

    /// Poll Era for transaction receipt and confirm the batch.
    /// Returns true if confirmed, false if still pending.
    pub async fn poll_confirmation(&mut self, batch_id: u64) -> Result<bool> {
        let batch = self.batches.get(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Submitted {
            return Ok(false);
        }

        let tx_hash_str = match &batch.l2_tx_hash {
            Some(h) => h.clone(),
            None => return Ok(false),
        };

        // In mock mode, auto-confirm after submission
        if !self.is_live() {
            self.confirm_batch(batch_id)?;
            return Ok(true);
        }

        // In live mode, check Era for receipt
        let provider = Provider::<Http>::try_from(&self.config.era_rpc_url)
            .map_err(|e| anyhow!("Era RPC connection: {}", e))?;

        let tx_hash: H256 = tx_hash_str.parse()
            .map_err(|e| anyhow!("invalid tx hash: {}", e))?;

        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(receipt)) => {
                let success = receipt.status
                    .map(|s| s == U64::from(1))
                    .unwrap_or(false);

                if success {
                    info!(
                        "Settlement batch {} confirmed on Era (block: {:?})",
                        batch_id, receipt.block_number,
                    );
                    self.confirm_batch(batch_id)?;
                    Ok(true)
                } else {
                    error!(
                        "Settlement batch {} REVERTED on Era (tx: {})",
                        batch_id, tx_hash_str,
                    );
                    if let Some(b) = self.batches.get_mut(&batch_id) {
                        b.status = SettlementStatus::Failed;
                    }
                    Ok(false)
                }
            }
            Ok(None) => {
                debug!("Batch {} still pending on Era", batch_id);
                Ok(false)
            }
            Err(e) => {
                warn!("Failed to poll Era for batch {}: {}", batch_id, e);
                Ok(false)
            }
        }
    }

    /// Confirm a submitted batch.
    pub fn confirm_batch(&mut self, batch_id: u64) -> Result<()> {
        let batch = self.batches.get_mut(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Submitted {
            return Err(anyhow!(
                "Batch {} is not submitted (status: {})",
                batch_id, batch.status,
            ));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        batch.status = SettlementStatus::Settled;
        batch.settled_at = Some(now);

        if let Some(&last) = batch.block_heights.last() {
            self.last_settled_height = last;
        }

        info!(
            "Settlement batch {} confirmed (blocks: {:?})",
            batch_id, batch.block_heights,
        );

        Ok(())
    }

    /// Synchronous mock submission — for use in the batch submitter loop
    /// where we hold a std::sync::Mutex and can't await.
    /// In mock mode, auto-confirms immediately. Returns tx hash.
    pub fn submit_and_confirm_mock(&mut self, batch_id: u64) -> Result<String> {
        let batch = self.batches.get(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Pending {
            return Err(anyhow!(
                "Batch {} is not pending (status: {})",
                batch_id, batch.status,
            ));
        }

        let selector = settle_batch_selector();
        let calldata = encode_settle_batch_calldata(
            &selector, &batch.public_values, &batch.proof_bytes,
        );
        let tx_hash = self.submit_mock(batch_id, &calldata);

        // Update to Submitted
        let batch = self.batches.get_mut(&batch_id).unwrap();
        batch.status = SettlementStatus::Submitted;
        batch.l2_tx_hash = Some(tx_hash.clone());

        // Auto-confirm in mock mode
        self.confirm_batch(batch_id)?;

        Ok(tx_hash)
    }

    /// Get the config needed for live submission (clone-friendly for async use).
    pub fn live_submission_data(&self, batch_id: u64) -> Result<LiveSubmissionData> {
        let batch = self.batches.get(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Pending {
            return Err(anyhow!(
                "Batch {} is not pending (status: {})",
                batch_id, batch.status,
            ));
        }

        let selector = settle_batch_selector();
        let calldata = encode_settle_batch_calldata(
            &selector, &batch.public_values, &batch.proof_bytes,
        );

        Ok(LiveSubmissionData {
            batch_id,
            calldata,
            era_rpc_url: self.config.era_rpc_url.clone(),
            settlement_contract: self.config.settlement_contract.clone(),
            sequencer_key: self.config.sequencer_key.clone().unwrap_or_default(),
            era_chain_id: self.config.era_chain_id,
        })
    }

    /// Mark a batch as submitted with a tx hash (called after live async submission).
    pub fn mark_submitted(&mut self, batch_id: u64, tx_hash: String) -> Result<()> {
        let batch = self.batches.get_mut(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;
        batch.status = SettlementStatus::Submitted;
        batch.l2_tx_hash = Some(tx_hash);
        Ok(())
    }

    /// Mark a batch as failed.
    pub fn mark_failed(&mut self, batch_id: u64) -> Result<()> {
        let batch = self.batches.get_mut(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;
        batch.status = SettlementStatus::Failed;
        Ok(())
    }

    /// Get a batch by ID (for persistence after status changes).
    pub fn get_batch(&self, batch_id: u64) -> Option<&SettlementBatch> {
        self.batches.get(&batch_id)
    }

    /// Get pending batch IDs (for the submission loop).
    pub fn pending_batch_ids(&self) -> Vec<u64> {
        self.batches.values()
            .filter(|b| b.status == SettlementStatus::Pending)
            .map(|b| b.batch_id)
            .collect()
    }

    /// Get submitted (awaiting confirmation) batch IDs.
    pub fn submitted_batch_ids(&self) -> Vec<u64> {
        self.batches.values()
            .filter(|b| b.status == SettlementStatus::Submitted)
            .map(|b| b.batch_id)
            .collect()
    }

    pub fn batches_by_status(&self, status: SettlementStatus) -> Vec<&SettlementBatch> {
        self.batches.values()
            .filter(|b| b.status == status)
            .collect()
    }

    pub fn summary(&self) -> SettlementSummary {
        SettlementSummary {
            total_batches: self.batches.len() as u64,
            pending: self.batches.values().filter(|b| b.status == SettlementStatus::Pending).count() as u64,
            submitted: self.batches.values().filter(|b| b.status == SettlementStatus::Submitted).count() as u64,
            settled: self.batches.values().filter(|b| b.status == SettlementStatus::Settled).count() as u64,
            failed: self.batches.values().filter(|b| b.status == SettlementStatus::Failed).count() as u64,
            last_settled_height: self.last_settled_height,
            settlement_contract: self.config.settlement_contract.clone(),
            era_rpc_url: self.config.era_rpc_url.clone(),
            mode: if self.is_live() { "live".to_string() } else { "mock".to_string() },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementSummary {
    pub total_batches: u64,
    pub pending: u64,
    pub submitted: u64,
    pub settled: u64,
    pub failed: u64,
    pub last_settled_height: u64,
    pub settlement_contract: String,
    pub era_rpc_url: String,
    pub mode: String,
}

// ── ABI Encoding ─────────────────────────────────────────────────────────────

/// Compute keccak256 selector for settleBatch(bytes,bytes).
fn settle_batch_selector() -> [u8; 4] {
    let hash = keccak256(b"settleBatch(bytes,bytes)");
    [hash[0], hash[1], hash[2], hash[3]]
}

/// Encode calldata for settleBatch(bytes publicValues, bytes proofBytes).
fn encode_settle_batch_calldata(
    selector: &[u8; 4],
    public_values: &[u8],
    proof_bytes: &[u8],
) -> Vec<u8> {
    let pv_padded = pad_to_32(public_values.len());
    let pb_padded = pad_to_32(proof_bytes.len());

    let offset_pv: u64 = 64;
    let offset_pb: u64 = offset_pv + 32 + pv_padded as u64;

    let mut calldata = Vec::new();

    calldata.extend_from_slice(selector);

    calldata.extend_from_slice(&encode_u256(offset_pv as u128));
    calldata.extend_from_slice(&encode_u256(offset_pb as u128));

    calldata.extend_from_slice(&encode_u256(public_values.len() as u128));
    calldata.extend_from_slice(public_values);
    let pad_pv = pv_padded - public_values.len();
    calldata.extend(std::iter::repeat(0u8).take(pad_pv));

    calldata.extend_from_slice(&encode_u256(proof_bytes.len() as u128));
    calldata.extend_from_slice(proof_bytes);
    let pad_pb = pb_padded - proof_bytes.len();
    calldata.extend(std::iter::repeat(0u8).take(pad_pb));

    calldata
}

fn encode_u256(value: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..32].copy_from_slice(&value.to_be_bytes());
    word
}

fn pad_to_32(len: usize) -> usize {
    (len + 31) & !31
}

// ── Live submission data (for async submission outside the Mutex) ────────────

/// Data extracted from SettlementManager for async live submission
/// without holding the Mutex across await points.
#[derive(Debug, Clone)]
pub struct LiveSubmissionData {
    pub batch_id: u64,
    pub calldata: Vec<u8>,
    pub era_rpc_url: String,
    pub settlement_contract: String,
    pub sequencer_key: String,
    pub era_chain_id: u64,
}

/// Submit a settlement batch to Era without holding a lock.
/// Called with data extracted via `SettlementManager::live_submission_data()`.
pub async fn submit_live_async(data: &LiveSubmissionData) -> Result<String> {
    let contract_addr: H160 = data.settlement_contract.parse()
        .map_err(|e| anyhow!("invalid settlement_contract address: {}", e))?;

    let provider = Provider::<Http>::try_from(&data.era_rpc_url)
        .map_err(|e| anyhow!("Era RPC connection failed: {}", e))?;

    let wallet: LocalWallet = data.sequencer_key.parse::<LocalWallet>()
        .map_err(|e| anyhow!("invalid sequencer_key: {}", e))?
        .with_chain_id(data.era_chain_id);

    let client = SignerMiddleware::new(provider, wallet);

    let tx = TransactionRequest::new()
        .to(contract_addr)
        .data(Bytes::from(data.calldata.clone()));

    info!(
        "Submitting batch {} to Era (contract: {}, chain: {})",
        data.batch_id, data.settlement_contract, data.era_chain_id,
    );

    let pending_tx = client.send_transaction(tx, None).await
        .map_err(|e| {
            error!("Era tx submission failed for batch {}: {}", data.batch_id, e);
            anyhow!("Era submission failed: {}", e)
        })?;

    let tx_hash = format!("0x{}", hex::encode(pending_tx.tx_hash().as_bytes()));

    info!(
        "Settlement batch {} submitted to Era: {}",
        data.batch_id, tx_hash,
    );

    Ok(tx_hash)
}

/// Poll Era for a transaction receipt (async, no lock held).
pub async fn poll_receipt_async(
    era_rpc_url: &str,
    tx_hash_str: &str,
) -> Result<Option<bool>> {
    let provider = Provider::<Http>::try_from(era_rpc_url)
        .map_err(|e| anyhow!("Era RPC connection: {}", e))?;

    let tx_hash: H256 = tx_hash_str.parse()
        .map_err(|e| anyhow!("invalid tx hash: {}", e))?;

    match provider.get_transaction_receipt(tx_hash).await {
        Ok(Some(receipt)) => {
            let success = receipt.status
                .map(|s| s == U64::from(1))
                .unwrap_or(false);
            Ok(Some(success))
        }
        Ok(None) => Ok(None), // Still pending
        Err(e) => {
            warn!("Failed to poll Era for tx {}: {}", tx_hash_str, e);
            Ok(None)
        }
    }
}
