// SP1 + revm Proving Module — zkC0DL3 Sovereign Prover
//
// Architecture (Option B — LOCKED IN):
//   Public EVM layer (revm — Solidity contracts, ERC-20 tokens, bridge logic)
//     + Privacy precompiles (Ristretto255, Pedersen, Bulletproofs — callable from Solidity)
//     + Shielded pool (private balances, private transfers, private DeFi)
//     = Proven together by SP1 → SP1Verifier.sol on zkSync Era
//
// Three Privacy Layers:
//   L1: Transfer privacy — full (amount + addresses) via shielded pool + stealth
//   L2: Amount privacy in contracts — Solidity contracts using Pedersen/Bulletproofs precompiles
//   L3: Full contract privacy (Phase 3) — client-side ZK circuits verified in SP1
//
// Proof flow:
//   1. Block proposed (2s cadence) — soft-confirmed
//   2. Prover downloads block data + pre-state
//   3. Prover runs SP1 guest program (revm execution + privacy validation)
//   4. Prover submits proof to sequencer
//   5. Sequencer verifies proof via SP1 SDK
//   6. Block is hard-confirmed — proof batched for L2 settlement
//
// Settlement:
//   L3 batch → SP1 proof → SP1Verifier.sol on zkSync Era L2 → Ethereum L1

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use tracing::info;

// ── Block Execution Claim ────────────────────────────────────────────────────
//
// Public inputs committed to by the SP1 block execution proof.
// The prover must show: executing all transactions from `prev_state_root`
// produces `new_state_root`, consuming `total_gas_used` gas.

/// Public inputs for a block execution proof.
/// Both the prover (SP1 guest) and verifier (node) agree on these values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockExecutionClaim {
    /// Block height being proven.
    pub block_height: u64,
    /// State root before executing this block's transactions.
    pub prev_state_root: [u8; 32],
    /// State root after executing all transactions in this block.
    pub new_state_root: [u8; 32],
    /// Merkle root over transaction hashes in this block.
    pub tx_merkle_root: [u8; 32],
    /// Number of transactions in this block.
    pub tx_count: u32,
    /// Total gas consumed by all transactions.
    pub total_gas_used: u64,
    /// Shielded pool note tree root after this block.
    pub note_tree_root: [u8; 32],
    /// Number of nullifiers consumed in this block.
    pub nullifier_count: u32,
    /// Bridge withdrawal tree root after this block.
    /// Committed in the proof so Era-side bridge can verify withdrawal Merkle proofs.
    pub withdrawal_tree_root: [u8; 32],
}

impl BlockExecutionClaim {
    /// Encode as deterministic bytes for SP1 public values commitment.
    /// Layout: block_height(8) || prev_state_root(32) || new_state_root(32)
    ///         || tx_merkle_root(32) || tx_count(4) || total_gas_used(8)
    ///         || note_tree_root(32) || nullifier_count(4) || withdrawal_tree_root(32)
    /// Total: 184 bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(184);
        buf.extend_from_slice(&self.block_height.to_le_bytes());
        buf.extend_from_slice(&self.prev_state_root);
        buf.extend_from_slice(&self.new_state_root);
        buf.extend_from_slice(&self.tx_merkle_root);
        buf.extend_from_slice(&self.tx_count.to_le_bytes());
        buf.extend_from_slice(&self.total_gas_used.to_le_bytes());
        buf.extend_from_slice(&self.note_tree_root);
        buf.extend_from_slice(&self.nullifier_count.to_le_bytes());
        buf.extend_from_slice(&self.withdrawal_tree_root);
        buf
    }

    /// Decode from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 184 {
            return Err(anyhow!(
                "BlockExecutionClaim: expected 184 bytes, got {}",
                bytes.len()
            ));
        }
        let block_height = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let prev_state_root: [u8; 32] = bytes[8..40].try_into().unwrap();
        let new_state_root: [u8; 32] = bytes[40..72].try_into().unwrap();
        let tx_merkle_root: [u8; 32] = bytes[72..104].try_into().unwrap();
        let tx_count = u32::from_le_bytes(bytes[104..108].try_into().unwrap());
        let total_gas_used = u64::from_le_bytes(bytes[108..116].try_into().unwrap());
        let note_tree_root: [u8; 32] = bytes[116..148].try_into().unwrap();
        let nullifier_count = u32::from_le_bytes(bytes[148..152].try_into().unwrap());
        let withdrawal_tree_root: [u8; 32] = bytes[152..184].try_into().unwrap();

        Ok(Self {
            block_height,
            prev_state_root,
            new_state_root,
            tx_merkle_root,
            tx_count,
            total_gas_used,
            note_tree_root,
            nullifier_count,
            withdrawal_tree_root,
        })
    }

    /// Compute a binding digest: SHA-256("COLDL3:exec:v1" || encoded_claim).
    /// Used to verify that proof public values match the expected claim.
    pub fn binding_digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"COLDL3:exec:v1");
        h.update(&self.encode());
        h.finalize().into()
    }
}

// ── Proof Verification ──────────────────────────────────────────────────────
//
// SP1 cryptographic verification. Requires `real-proofs` feature flag
// which pulls in sp1-sdk + bincode. Without it, proof submission is
// rejected — there is no silent pass-through.

/// Verify a submitted proof against the expected block execution claim.
///
/// Requires the `real-proofs` feature. Without it, returns an error —
/// proofs cannot be verified without the SP1 SDK compiled in.
pub fn verify_execution_proof(
    proof_bytes: &[u8],
    claim: &BlockExecutionClaim,
    vkey: &[u8],
) -> Result<bool> {
    #[cfg(feature = "real-proofs")]
    {
        verify_sp1_proof(proof_bytes, claim, vkey)
    }
    #[cfg(not(feature = "real-proofs"))]
    {
        let _ = (proof_bytes, claim, vkey);
        Err(anyhow!(
            "Proof verification unavailable: compile with --features real-proofs to enable SP1 verification"
        ))
    }
}

/// SP1 proof verification — deserializes vkey + proof, checks public values
/// match the expected claim, then runs cryptographic verification via SP1 SDK.
#[cfg(feature = "real-proofs")]
fn verify_sp1_proof(
    proof_bytes: &[u8],
    claim: &BlockExecutionClaim,
    vkey_bytes: &[u8],
) -> Result<bool> {
    use sp1_sdk::{ProverClient, SP1ProofWithPublicValues};

    info!(
        "Verifying SP1 proof for block {} ({} txs, {} gas)",
        claim.block_height, claim.tx_count, claim.total_gas_used,
    );

    // Deserialize verification key
    let vk: sp1_sdk::SP1VerifyingKey = bincode::deserialize(vkey_bytes)
        .map_err(|e| anyhow!("failed to deserialize SP1 verification key: {}", e))?;

    // Deserialize proof
    let proof: SP1ProofWithPublicValues = bincode::deserialize(proof_bytes)
        .map_err(|e| anyhow!("failed to deserialize SP1 proof: {}", e))?;

    // Verify that the proof's public values match our expected claim
    let expected_encoding = claim.encode();
    let proof_public_values = proof.public_values.as_slice();
    if proof_public_values.len() < expected_encoding.len() {
        return Err(anyhow!(
            "SP1 proof public values too short: {} < {}",
            proof_public_values.len(),
            expected_encoding.len(),
        ));
    }
    if &proof_public_values[..expected_encoding.len()] != expected_encoding.as_slice() {
        return Err(anyhow!(
            "SP1 proof public values mismatch — claim does not match proof output",
        ));
    }

    // Cryptographic verification
    let client = ProverClient::builder().build();
    client
        .verify(&proof, &vk)
        .map_err(|e| anyhow!("SP1 proof verification failed: {}", e))?;

    info!(
        "SP1 proof verified for block {} ✓",
        claim.block_height,
    );

    Ok(true)
}

// ── Prover Configuration ────────────────────────────────────────────────────

/// Configuration for the proving subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverConfig {
    /// SP1 verification key bytes (loaded from file via --prover-vkey).
    pub vkey: Vec<u8>,
    /// SP1 program ELF hash (identifies which guest program is expected).
    /// Used for sanity checks but not for cryptographic verification (that's the vkey).
    pub program_elf_hash: Option<String>,
}

impl ProverConfig {
    /// Create a configuration with a verification key.
    pub fn new(vkey: Vec<u8>) -> Self {
        Self {
            vkey,
            program_elf_hash: None,
        }
    }

    /// Whether SP1 verification is available (real-proofs feature compiled in).
    pub fn verification_available() -> bool {
        cfg!(feature = "real-proofs")
    }

    pub fn label() -> &'static str {
        if cfg!(feature = "real-proofs") {
            "sp1-sovereign"
        } else {
            "sp1-sovereign (verification requires --features real-proofs)"
        }
    }
}

// ── Custom Precompile Addresses ─────────────────────────────────────────────
//
// These addresses are used for privacy precompiles callable from Solidity.
// Address range: 0x0100-0x01FF (above standard EVM precompiles at 0x01-0x0A).

/// Ristretto255 point operations (add, mul, decompress, compress).
pub const PRECOMPILE_RISTRETTO255: u64 = 0x0100;
/// Pedersen commitment: C = v·G + r·H. Input: (value_scalar, blinding_scalar).
pub const PRECOMPILE_PEDERSEN_COMMIT: u64 = 0x0101;
/// Bulletproofs range proof verification. Input: (commitment, proof_bytes, bits).
pub const PRECOMPILE_BULLETPROOFS_VERIFY: u64 = 0x0102;
/// Shielded pool deposit: shield tokens from EVM into the private pool.
pub const PRECOMPILE_SHIELD: u64 = 0x0110;
/// Shielded pool withdrawal: unshield tokens from private pool to EVM.
pub const PRECOMPILE_UNSHIELD: u64 = 0x0111;

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_execution_claim_encode_decode() {
        let claim = BlockExecutionClaim {
            block_height: 42,
            prev_state_root: [1u8; 32],
            new_state_root: [2u8; 32],
            tx_merkle_root: [3u8; 32],
            tx_count: 10,
            total_gas_used: 210_000,
            note_tree_root: [4u8; 32],
            nullifier_count: 5,
            withdrawal_tree_root: [5u8; 32],
        };
        let encoded = claim.encode();
        assert_eq!(encoded.len(), 184);

        let decoded = BlockExecutionClaim::decode(&encoded).unwrap();
        assert_eq!(decoded, claim);
    }

    #[test]
    fn test_block_execution_claim_decode_too_short() {
        let short = vec![0u8; 100];
        assert!(BlockExecutionClaim::decode(&short).is_err());
    }

    #[test]
    fn test_binding_digest_deterministic() {
        let claim = BlockExecutionClaim {
            block_height: 1,
            prev_state_root: [0u8; 32],
            new_state_root: [0xFFu8; 32],
            tx_merkle_root: [0xABu8; 32],
            tx_count: 3,
            total_gas_used: 63_000,
            note_tree_root: [0xCDu8; 32],
            nullifier_count: 2,
            withdrawal_tree_root: [0xEFu8; 32],
        };
        let d1 = claim.binding_digest();
        let d2 = claim.binding_digest();
        assert_eq!(d1, d2);
        // Different claim → different digest
        let mut claim2 = claim.clone();
        claim2.block_height = 2;
        assert_ne!(d1, claim2.binding_digest());
    }

    #[test]
    fn test_verification_requires_real_proofs_feature() {
        let claim = BlockExecutionClaim {
            block_height: 99,
            prev_state_root: [0u8; 32],
            new_state_root: [1u8; 32],
            tx_merkle_root: [2u8; 32],
            tx_count: 0,
            total_gas_used: 0,
            note_tree_root: [0u8; 32],
            nullifier_count: 0,
            withdrawal_tree_root: [0u8; 32],
        };
        let result = verify_execution_proof(b"garbage", &claim, &[]);
        if cfg!(feature = "real-proofs") {
            // With real-proofs: should fail because "garbage" isn't a valid SP1 proof
            assert!(result.is_err());
        } else {
            // Without real-proofs: should fail because verification is unavailable
            assert!(result.is_err());
            assert!(
                result.unwrap_err().to_string().contains("real-proofs"),
                "Error should mention real-proofs feature"
            );
        }
    }

    #[test]
    fn test_prover_config_label() {
        if cfg!(feature = "real-proofs") {
            assert_eq!(ProverConfig::label(), "sp1-sovereign");
        } else {
            assert!(ProverConfig::label().contains("real-proofs"));
        }
    }

    #[test]
    fn test_precompile_addresses_unique() {
        let addrs = [
            PRECOMPILE_RISTRETTO255,
            PRECOMPILE_PEDERSEN_COMMIT,
            PRECOMPILE_BULLETPROOFS_VERIFY,
            PRECOMPILE_SHIELD,
            PRECOMPILE_UNSHIELD,
        ];
        let unique: std::collections::HashSet<_> = addrs.iter().collect();
        assert_eq!(unique.len(), addrs.len());
    }
}
