// Shared types between prover service, node RPC, and SP1 guest program.
//
// These types define the wire format for:
//   1. Node → Prover: block data + pre-state witnesses (via /proof/block_input/{height})
//   2. Prover → SP1 guest: GuestBlockInput (serialized into SP1Stdin)
//   3. Prover → Node: proof submission (via POST /proof/submit)
//   4. Guest → Verifier: BlockExecutionClaim (public outputs, 152 bytes)
//
// IMPORTANT: Any changes here must be mirrored in:
//   - program/src/main.rs (GuestBlockInput, GuestTransaction, BlockExecutionClaim)
//   - program/src/state.rs (AccountWitness)
//   - program/src/privacy.rs (ShieldedBlockData, GuestSpendProof)
//   - src/proving/mod.rs (BlockExecutionClaim)

use serde::{Deserialize, Serialize};

// ── Block Execution Claim ─────────────────────────────────────────────────────
//
// Public outputs committed by the SP1 proof. Both prover and verifier agree.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockExecutionClaim {
    pub block_height: u64,
    pub prev_state_root: [u8; 32],
    pub new_state_root: [u8; 32],
    pub tx_merkle_root: [u8; 32],
    pub tx_count: u32,
    pub total_gas_used: u64,
    pub note_tree_root: [u8; 32],
    pub nullifier_count: u32,
}

impl BlockExecutionClaim {
    /// Decode from the SP1 proof's public values (152 bytes).
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() < 152 {
            anyhow::bail!("BlockExecutionClaim: expected 152 bytes, got {}", bytes.len());
        }
        Ok(Self {
            block_height: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            prev_state_root: bytes[8..40].try_into().unwrap(),
            new_state_root: bytes[40..72].try_into().unwrap(),
            tx_merkle_root: bytes[72..104].try_into().unwrap(),
            tx_count: u32::from_le_bytes(bytes[104..108].try_into().unwrap()),
            total_gas_used: u64::from_le_bytes(bytes[108..116].try_into().unwrap()),
            note_tree_root: bytes[116..148].try_into().unwrap(),
            nullifier_count: u32::from_le_bytes(bytes[148..152].try_into().unwrap()),
        })
    }
}

// ── Guest Input Types ─────────────────────────────────────────────────────────
//
// These are serialized into SP1Stdin and read by the guest program.

/// Transaction for guest-side re-execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestTransaction {
    pub from: [u8; 20],
    /// None = contract creation, Some = call
    pub to: Option<[u8; 20]>,
    pub value: u64,
    pub gas_limit: u64,
    pub gas_price: u64,
    pub nonce: u64,
    pub data: Vec<u8>,
}

/// Pre-state account data — private witness fed to the guest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWitness {
    /// Address as string (e.g. "0x1234...") — must match host HashMap key format.
    pub address: String,
    /// Plaintext balance in fwei.
    pub balance: u64,
    /// Account nonce.
    pub nonce: u64,
    /// Contract bytecode (empty for EOAs).
    pub code: Vec<u8>,
    /// Storage slots: (key_bytes_32, value_bytes_32).
    pub storage: Vec<([u8; 32], [u8; 32])>,
}

/// Spend proof — proves valid consumption of shielded notes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestSpendProof {
    pub nullifiers: Vec<[u8; 32]>,
    pub input_commitments: Vec<[u8; 32]>,
    pub output_commitments: Vec<[u8; 32]>,
    pub fee_commitment: [u8; 32],
    pub kernel_excess: [u8; 32],
    pub range_proofs: Vec<Vec<u8>>,
}

/// All shielded pool data for a single block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldedBlockData {
    pub spend_proofs: Vec<GuestSpendProof>,
    pub new_notes: Vec<[u8; 32]>,
    pub nullifiers: Vec<[u8; 32]>,
    pub prev_nullifiers: Vec<[u8; 32]>,
    pub prev_note_commitments: Vec<[u8; 32]>,
}

impl Default for ShieldedBlockData {
    fn default() -> Self {
        Self {
            spend_proofs: vec![],
            new_notes: vec![],
            nullifiers: vec![],
            prev_nullifiers: vec![],
            prev_note_commitments: vec![],
        }
    }
}

/// Block input data — everything the SP1 guest needs to prove a block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestBlockInput {
    pub block_height: u64,
    pub prev_state_root: [u8; 32],
    pub transactions: Vec<GuestTransaction>,
    pub accounts: Vec<AccountWitness>,
    pub prev_note_tree_root: [u8; 32],
    pub shielded: ShieldedBlockData,
    pub block_gas_limit: u64,
    pub timestamp: u64,
}

// ── Node RPC Response Types ───────────────────────────────────────────────────

/// Response from GET /proof/block_input/{height}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInputResponse {
    pub block_height: u64,
    pub block_input: GuestBlockInput,
    /// Expected claim — the node's view of what the proof should commit to.
    /// Prover can verify locally before running the expensive SP1 proof.
    pub expected_claim: BlockExecutionClaimWire,
}

/// Wire format for BlockExecutionClaim in JSON responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecutionClaimWire {
    pub block_height: u64,
    pub prev_state_root: String,
    pub new_state_root: String,
    pub tx_merkle_root: String,
    pub tx_count: u32,
    pub total_gas_used: u64,
    pub note_tree_root: String,
    pub nullifier_count: u32,
}

/// Proof submission — sent to POST /proof/submit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofSubmission {
    pub block_height: u64,
    pub prover_address: String,
    pub proof_bytes: Vec<u8>,
    pub submitted_at: u64,
}

/// Response from GET /proof/pending
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBlock {
    pub block_height: u64,
    pub status: String,
    pub tx_count: usize,
    pub submissions: usize,
}
