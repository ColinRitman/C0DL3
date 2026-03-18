// Shared types between prover service, node RPC, and SP1 guest program.
//
// These types define the wire format for:
//   1. Node → Prover: block data + pre-state witnesses (via /proof/block_input/{height})
//   2. Prover → SP1 guest: GuestBlockInput (serialized into SP1Stdin)
//   3. Prover → Node: proof submission (via POST /proof/submit)
//   4. Guest → Verifier: BlockExecutionClaim (public outputs, 152 bytes)
//
// IMPORTANT: Any changes here must be mirrored in:
//   - program/src/main.rs (GuestBlockInput, GuestTransaction, GuestUserOperation, BlockExecutionClaim)
//   - program/src/state.rs (AccountWitness)
//   - program/src/privacy.rs (ShieldedBlockData, GuestSpendProof, CommitmentKnowledgeProof)
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
///
/// PRIVACY: For accounts not involved in EVM transactions,
/// `precomputed_commitment` can be provided instead of the plaintext balance.
/// This prevents the prover from learning balances of shielded-only accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountWitness {
    /// Address as string (e.g. "0x1234...") — must match host HashMap key format.
    pub address: String,
    /// Plaintext balance in fwei.
    /// For commitment-only witnesses, this is 0 (unused — commitment is authoritative).
    pub balance: u64,
    /// Account nonce.
    pub nonce: u64,
    /// Contract bytecode (empty for EOAs).
    pub code: Vec<u8>,
    /// Storage slots: (key_bytes_32, value_bytes_32).
    pub storage: Vec<([u8; 32], [u8; 32])>,
    /// Pre-computed balance commitment. When Some, the guest uses this directly
    /// instead of computing from plaintext balance.
    #[serde(default)]
    pub precomputed_commitment: Option<[u8; 32]>,
    /// 0 = LegacyEOA, 1 = PrivateWallet
    #[serde(default)]
    pub wallet_type: u8,
    /// Owner public key for PrivateWallet accounts.
    #[serde(default)]
    pub owner_pubkey: Option<[u8; 32]>,
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

/// Schnorr-style proof of knowledge for a Pedersen commitment.
/// Proves: "I know (v, r) such that C = v*G + r*H" without revealing v or r.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentKnowledgeProof {
    pub commitment: [u8; 32],
    pub announcement: [u8; 32],
    pub response_v: [u8; 32],
    pub response_r: [u8; 32],
}

/// Shield request — client-side proven deposit into shielded pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestShieldRequest {
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub recipient_pubkey: [u8; 32],
    pub knowledge_proof: CommitmentKnowledgeProof,
}

/// Unshield request — client-side proven withdrawal from shielded pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestUnshieldRequest {
    pub nullifier: [u8; 32],
    pub note_commitment: [u8; 32],
    pub value_commitment: [u8; 32],
    pub knowledge_proof: CommitmentKnowledgeProof,
    pub merkle_proof: Vec<([u8; 32], bool)>,
}

/// All shielded pool data for a single block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldedBlockData {
    pub spend_proofs: Vec<GuestSpendProof>,
    pub new_notes: Vec<[u8; 32]>,
    pub nullifiers: Vec<[u8; 32]>,
    pub prev_nullifiers: Vec<[u8; 32]>,
    pub prev_note_commitments: Vec<[u8; 32]>,
    /// Client-side proven shield requests (deposits into shielded pool).
    #[serde(default)]
    pub shield_requests: Vec<GuestShieldRequest>,
    /// Client-side proven unshield requests (withdrawals from shielded pool).
    #[serde(default)]
    pub unshield_requests: Vec<GuestUnshieldRequest>,
}

impl Default for ShieldedBlockData {
    fn default() -> Self {
        Self {
            spend_proofs: vec![],
            new_notes: vec![],
            nullifiers: vec![],
            prev_nullifiers: vec![],
            prev_note_commitments: vec![],
            shield_requests: vec![],
            unshield_requests: vec![],
        }
    }
}

/// Guest-side UserOperation for AA wallet verification.
///
/// Minimal representation for in-circuit verification of AA transfers.
/// The guest verifies: Schnorr auth, conservation, and knowledge proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestUserOperation {
    pub sender: String,
    pub nonce: u64,
    pub to: String,
    pub sender_old_commitment: [u8; 32],
    pub sender_new_commitment: [u8; 32],
    pub recipient_old_commitment: [u8; 32],
    pub recipient_new_commitment: [u8; 32],
    pub amount_commitment: [u8; 32],
    pub knowledge_proof: CommitmentKnowledgeProof,
    pub auth_signature_r: [u8; 32],
    pub auth_signature_s: [u8; 32],
    pub sender_pubkey: [u8; 32],
    pub gas_limit: u64,
    pub gas_price: u64,
    pub paymaster: Option<String>,
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
    /// AA UserOperations for this block.
    #[serde(default)]
    pub user_operations: Vec<GuestUserOperation>,
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
