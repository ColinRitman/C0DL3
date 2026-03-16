// COLDL3 Wallet SDK — Client-Side Privacy Proof Generation
//
// This crate runs in the user's wallet (browser, mobile, CLI). It generates
// cryptographic proofs that allow the sequencer to process shielded operations
// without ever seeing plaintext amounts, blinding factors, or spend keys.
//
// The sequencer receives only:
//   - Pedersen commitments (opaque 32-byte points)
//   - Knowledge proofs (96-byte Schnorr proofs)
//   - Range proofs (Bulletproofs, ~700 bytes)
//   - Nullifiers (32-byte hashes)
//   - Merkle membership proofs
//
// NO SP1 DEPENDENCY. This is pure crypto — runs on any platform.

pub mod auto_shield;
pub mod commitment_proof;
pub mod encrypted_memo;
pub mod merkle;
pub mod shield;
pub mod stealth;
pub mod types;

pub use commitment_proof::{
    CommitmentKnowledgeProof,
    prove_commitment_knowledge,
    verify_commitment_knowledge,
};
pub use shield::{ShieldRequest, UnshieldRequest};
pub use types::SpendProofRequest;
