// Settlement Module — L3 → zkSync Era (L2) → Ethereum (L1)
//
// This module handles submitting proven L3 block proofs to the COLDL3Settlement
// contract on zkSync Era. The settlement flow:
//
//   1. Prover generates SP1 proof for an L3 block
//   2. Node verifies proof and records ProvenBlock
//   3. Settlement module batches proven blocks
//   4. Submits (publicValues, proofBytes) to COLDL3Settlement.settleBatch()
//   5. Contract verifies via SP1VerifierGateway and commits state root
//
// Once settled on Era, the L3 state inherits Ethereum's finality guarantees.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

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
    /// For batch settlement, this is the claim of the last block in the batch
    /// (which proves the cumulative state transition).
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
    /// Batch assembled, awaiting submission.
    Pending,
    /// Submitted to L2, awaiting confirmation.
    Submitted,
    /// Confirmed on L2 (settled).
    Settled,
    /// Submission failed (will retry).
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

/// Configuration for the settlement module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementModuleConfig {
    /// zkSync Era RPC URL.
    pub era_rpc_url: String,
    /// COLDL3Settlement contract address on zkSync Era.
    pub settlement_contract: String,
    /// Sequencer private key for signing settlement txs (hex-encoded).
    /// In production, use a KMS signer.
    pub sequencer_key: Option<String>,
    /// Maximum blocks per settlement batch.
    pub max_batch_size: u64,
    /// Settlement interval in seconds (how often to check for batchable blocks).
    pub interval_secs: u64,
}

impl Default for SettlementModuleConfig {
    fn default() -> Self {
        Self {
            era_rpc_url: "https://mainnet.era.zksync.io".to_string(),
            settlement_contract: String::new(),
            max_batch_size: 10,
            interval_secs: 30,
            sequencer_key: None,
        }
    }
}

// ── Settlement Manager ─────────────────────────────────────────────────────

/// Manages the settlement pipeline: batching proven blocks and submitting to L2.
pub struct SettlementManager {
    config: SettlementModuleConfig,
    /// All settlement batches (batch_id → batch).
    batches: std::collections::HashMap<u64, SettlementBatch>,
    /// Next batch ID.
    next_batch_id: u64,
    /// Last settled block height.
    last_settled_height: u64,
}

impl SettlementManager {
    pub fn new(config: SettlementModuleConfig) -> Self {
        Self {
            config,
            batches: std::collections::HashMap::new(),
            next_batch_id: 1,
            last_settled_height: 0,
        }
    }

    /// Create a settlement batch from proven blocks.
    ///
    /// Takes proven block data and assembles it into a batch for L2 submission.
    /// The batch contains the SP1 proof and public values for the last block,
    /// which proves the cumulative state transition from prev_state_root
    /// to new_state_root.
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

        // Verify continuity: first block must be after last settled
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

    /// Submit a pending batch to the COLDL3Settlement contract on zkSync Era.
    ///
    /// This constructs the calldata for `settleBatch(publicValues, proofBytes)`
    /// and submits it as a transaction to zkSync Era.
    ///
    /// Returns the L2 transaction hash on success.
    pub async fn submit_batch(&mut self, batch_id: u64) -> Result<String> {
        let batch = self.batches.get_mut(&batch_id)
            .ok_or_else(|| anyhow!("Batch {} not found", batch_id))?;

        if batch.status != SettlementStatus::Pending {
            return Err(anyhow!(
                "Batch {} is not pending (status: {})",
                batch_id, batch.status,
            ));
        }

        if self.config.settlement_contract.is_empty() {
            return Err(anyhow!(
                "Settlement contract address not configured. \
                 Set settlement_contract in config to enable L2 settlement."
            ));
        }

        info!(
            "Submitting settlement batch {} to Era ({} blocks, {} proof bytes)",
            batch_id,
            batch.block_heights.len(),
            batch.proof_bytes.len(),
        );

        // Encode calldata: settleBatch(bytes publicValues, bytes proofBytes)
        // Function selector: keccak256("settleBatch(bytes,bytes)")[0..4]
        let selector = settlement_batch_selector();
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

        // Submit to zkSync Era
        // In production: sign with sequencer key and send via Era RPC.
        // For now: record the calldata and mark as submitted.
        //
        // TODO(settlement): Integrate with ethers/alloy for actual Era submission:
        //   let provider = Provider::<Http>::try_from(&self.config.era_rpc_url)?;
        //   let wallet = LocalWallet::from_str(&self.config.sequencer_key.unwrap())?;
        //   let tx = TransactionRequest::new()
        //       .to(self.config.settlement_contract.parse()?)
        //       .data(calldata);
        //   let pending = wallet.sign_transaction(&tx).await?;
        //   let receipt = provider.send_raw_transaction(pending).await?;

        let mock_tx_hash = format!(
            "0x{:064x}",
            batch_id as u128 * 1_000_000 + batch.block_heights.last().unwrap_or(&0)
        );

        batch.status = SettlementStatus::Submitted;
        batch.l2_tx_hash = Some(mock_tx_hash.clone());

        info!(
            "Settlement batch {} submitted (mock tx: {})",
            batch_id, mock_tx_hash,
        );

        Ok(mock_tx_hash)
    }

    /// Confirm a submitted batch (called after L2 tx confirmation).
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

        // Update last settled height
        if let Some(&last) = batch.block_heights.last() {
            self.last_settled_height = last;
        }

        info!(
            "Settlement batch {} confirmed on L2 (blocks: {:?})",
            batch_id, batch.block_heights,
        );

        Ok(())
    }

    /// Get all batches with a given status.
    pub fn batches_by_status(&self, status: SettlementStatus) -> Vec<&SettlementBatch> {
        self.batches.values()
            .filter(|b| b.status == status)
            .collect()
    }

    /// Get settlement summary.
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
        }
    }
}

/// Settlement pipeline summary for the /settlement RPC endpoint.
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
}

// ── ABI Encoding Helpers ───────────────────────────────────────────────────
//
// Minimal ABI encoding for calling COLDL3Settlement.settleBatch(bytes, bytes).
// No ethers dependency required — we just need to encode two dynamic bytes.

/// Function selector for settleBatch(bytes,bytes).
/// keccak256("settleBatch(bytes,bytes)") = first 4 bytes.
fn settlement_batch_selector() -> [u8; 4] {
    use sha2::{Sha256, Digest};
    // We use SHA-256 here as a placeholder. In production with actual Era
    // submission, use keccak256 from the ethers/alloy crate.
    // The actual selector will be computed at deployment time.
    let hash = Sha256::digest(b"settleBatch(bytes,bytes)");
    [hash[0], hash[1], hash[2], hash[3]]
}

/// Encode calldata for settleBatch(bytes publicValues, bytes proofBytes).
///
/// ABI encoding for two dynamic `bytes` arguments:
///   [0..4)   function selector
///   [4..36)  offset to publicValues (= 64)
///   [36..68) offset to proofBytes (= 64 + 32 + padded_len(publicValues))
///   [68..)   encoded publicValues: length(32) + data(padded to 32)
///   [..)     encoded proofBytes: length(32) + data(padded to 32)
fn encode_settle_batch_calldata(
    selector: &[u8; 4],
    public_values: &[u8],
    proof_bytes: &[u8],
) -> Vec<u8> {
    let pv_padded = pad_to_32(public_values.len());
    let pb_padded = pad_to_32(proof_bytes.len());

    // Offset to first bytes arg = 64 (two 32-byte offset words)
    let offset_pv: u64 = 64;
    // Offset to second bytes arg = 64 + 32 (length) + padded data
    let offset_pb: u64 = offset_pv + 32 + pv_padded as u64;

    let mut calldata = Vec::new();

    // Function selector
    calldata.extend_from_slice(selector);

    // Offset to publicValues
    calldata.extend_from_slice(&encode_u256(offset_pv as u128));
    // Offset to proofBytes
    calldata.extend_from_slice(&encode_u256(offset_pb as u128));

    // publicValues: length + data
    calldata.extend_from_slice(&encode_u256(public_values.len() as u128));
    calldata.extend_from_slice(public_values);
    // Pad to 32-byte boundary
    let pad_pv = pv_padded - public_values.len();
    calldata.extend(std::iter::repeat(0u8).take(pad_pv));

    // proofBytes: length + data
    calldata.extend_from_slice(&encode_u256(proof_bytes.len() as u128));
    calldata.extend_from_slice(proof_bytes);
    let pad_pb = pb_padded - proof_bytes.len();
    calldata.extend(std::iter::repeat(0u8).take(pad_pb));

    calldata
}

/// Encode a u128 value as a 32-byte big-endian word (ABI uint256).
fn encode_u256(value: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..32].copy_from_slice(&value.to_be_bytes());
    word
}

/// Round up to nearest multiple of 32.
fn pad_to_32(len: usize) -> usize {
    (len + 31) & !31
}
