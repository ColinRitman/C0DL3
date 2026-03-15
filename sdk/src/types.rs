// Shared types between the SDK, host, and guest.
//
// These types are the "wire format" — what the wallet sends to the sequencer
// and what gets included in block data for the SP1 guest to verify.

use serde::{Deserialize, Serialize};

/// A complete spend proof request constructed client-side.
///
/// This is the shielded pool equivalent of a signed transaction.
/// The wallet builds this locally and submits to the sequencer.
/// The sequencer verifies proofs for early rejection, but the
/// SP1 guest re-verifies everything inside the ZK circuit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendProofRequest {
    /// Nullifiers for each input note (one per spend).
    pub nullifiers: Vec<[u8; 32]>,
    /// Commitments to the input notes being spent.
    pub input_commitments: Vec<[u8; 32]>,
    /// Commitments to new output notes.
    pub output_commitments: Vec<[u8; 32]>,
    /// Commitment to the transaction fee.
    pub fee_commitment: [u8; 32],
    /// Kernel excess (should be identity if conservation holds).
    pub kernel_excess: [u8; 32],
    /// Bulletproofs range proofs for each output.
    pub range_proofs: Vec<Vec<u8>>,
    /// Knowledge proofs for each output commitment.
    pub output_knowledge_proofs: Vec<crate::CommitmentKnowledgeProof>,
    /// Merkle membership proofs for each input note.
    /// Each proof is a list of (sibling, is_left) pairs.
    pub input_merkle_proofs: Vec<Vec<([u8; 32], bool)>>,
}
