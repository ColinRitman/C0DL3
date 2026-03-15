// Confidential Transactions using Bulletproofs
// Implements production-grade amount and balance privacy
// Phase 1: Foundation - Pedersen Commitments

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use bulletproofs::{BulletproofGens, PedersenGens};
use curve25519_dalek_ng::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use rand::{thread_rng, RngCore};
use once_cell::sync::Lazy;
use merlin::Transcript;

/// Global Pedersen generators (create once, reuse for efficiency)
static PEDERSEN_GENS: Lazy<PedersenGens> = Lazy::new(|| PedersenGens::default());

/// Global Bulletproof generators (pre-computed for efficiency)
/// Supports range proofs up to 64 bits, up to 128 commitments in batch
static BP_GENS: Lazy<BulletproofGens> = Lazy::new(|| {
    BulletproofGens::new(64, 128)
});

/// Confidential transaction amount commitment
/// 
/// Uses Pedersen commitments: C = v*G + r*H
/// where:
/// - v = amount (hidden)
/// - r = blinding factor (secret)
/// - G, H = generator points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountCommitment {
    /// Pedersen commitment point (compressed, 32 bytes)
    pub commitment: [u8; 32],
    /// Blinding factor (kept secret by sender/receiver, stored as bytes for serialization)
    #[serde(skip)]
    pub blinding_factor: Vec<u8>,
}

impl AmountCommitment {
    /// Create commitment to an amount
    /// 
    /// commitment = amount * G + blinding_factor * H
    /// 
    /// This hides the amount cryptographically while allowing
    /// later verification and homomorphic operations.
    pub fn new(amount: u64) -> Result<Self> {
        if amount == 0 {
            return Err(anyhow!("Amount cannot be zero"));
        }

        // Generate random blinding factor (32 bytes)
        let mut rng = thread_rng();
        let mut blinding_bytes = [0u8; 32];
        rng.fill_bytes(&mut blinding_bytes);

        // Convert amount to Scalar (little-endian u64 → 32-byte padded → mod order)
        let mut amount_padded = [0u8; 32];
        amount_padded[..8].copy_from_slice(&amount.to_le_bytes());
        let value_scalar = Scalar::from_bytes_mod_order(amount_padded);

        // Convert blinding factor to Scalar
        let blinding_scalar = Scalar::from_bytes_mod_order(blinding_bytes);

        // Real Pedersen commitment: C = v*B + r*B_blinding
        // where B and B_blinding are the Pedersen generator points
        let commitment_point = PEDERSEN_GENS.commit(value_scalar, blinding_scalar);
        let commitment_bytes = commitment_point.compress().to_bytes();

        Ok(Self {
            commitment: commitment_bytes,
            blinding_factor: blinding_bytes.to_vec(),
        })
    }
    
    /// Get blinding factor as bytes array
    fn blinding_factor_bytes(&self) -> Result<[u8; 32]> {
        if self.blinding_factor.len() != 32 {
            return Err(anyhow!("Invalid blinding factor length"));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&self.blinding_factor);
        Ok(bytes)
    }
    
    /// Verify commitment matches amount and blinding factor
    ///
    /// Recomputes the Pedersen commitment C = v*B + r*B_blinding
    /// and checks it matches the stored commitment bytes.
    pub fn verify(&self, amount: u64) -> Result<bool> {
        let blinding_bytes = self.blinding_factor_bytes()?;

        // Recompute Pedersen commitment with same amount and blinding
        let mut amount_padded = [0u8; 32];
        amount_padded[..8].copy_from_slice(&amount.to_le_bytes());
        let value_scalar = Scalar::from_bytes_mod_order(amount_padded);
        let blinding_scalar = Scalar::from_bytes_mod_order(blinding_bytes);

        let expected_point = PEDERSEN_GENS.commit(value_scalar, blinding_scalar);
        let expected_bytes = expected_point.compress().to_bytes();

        Ok(self.commitment == expected_bytes)
    }
    
    /// Decompress the stored commitment bytes into a RistrettoPoint
    ///
    /// # Post-Quantum Note
    /// Pedersen commitments rely on the discrete log assumption (Ristretto/Curve25519).
    /// For PQ migration, replace with STARK-based commitments via xfg-stark/Winterfell.
    pub fn to_point(&self) -> Result<RistrettoPoint> {
        let compressed = CompressedRistretto(self.commitment);
        compressed.decompress()
            .ok_or_else(|| anyhow!("Invalid commitment: cannot decompress to RistrettoPoint"))
    }

    /// Homomorphic addition: C(a, r_a) + C(b, r_b) = C(a+b, r_a+r_b)
    ///
    /// Adds two Pedersen commitments using Ristretto point addition.
    /// The resulting commitment hides the sum of the two amounts.
    pub fn add(&self, other: &AmountCommitment) -> Result<AmountCommitment> {
        let point_a = self.to_point()?;
        let point_b = other.to_point()?;
        let sum_point = point_a + point_b;

        // Blinding factors also add: r_sum = r_a + r_b
        let r_a = Scalar::from_bytes_mod_order(self.blinding_factor_bytes()?);
        let r_b = Scalar::from_bytes_mod_order(other.blinding_factor_bytes()?);
        let r_sum = r_a + r_b;

        Ok(AmountCommitment {
            commitment: sum_point.compress().to_bytes(),
            blinding_factor: r_sum.to_bytes().to_vec(),
        })
    }

    /// Homomorphic subtraction: C(a, r_a) - C(b, r_b) = C(a-b, r_a-r_b)
    ///
    /// Subtracts two Pedersen commitments using Ristretto point subtraction.
    pub fn subtract(&self, other: &AmountCommitment) -> Result<AmountCommitment> {
        let point_a = self.to_point()?;
        let point_b = other.to_point()?;
        let diff_point = point_a - point_b;

        let r_a = Scalar::from_bytes_mod_order(self.blinding_factor_bytes()?);
        let r_b = Scalar::from_bytes_mod_order(other.blinding_factor_bytes()?);
        let r_diff = r_a - r_b;

        Ok(AmountCommitment {
            commitment: diff_point.compress().to_bytes(),
            blinding_factor: r_diff.to_bytes().to_vec(),
        })
    }
    
    /// Create commitment hash for indexing/fingerprinting
    pub fn hash(&self) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&self.commitment);
        let result = hasher.finalize();
        hex::encode(result)
    }

    /// Generate a Bulletproofs range proof that amount is in [0, 2^bits)
    /// Returns the (proof_bytes, commitment_bytes) pair, where commitment must match self.commitment
    pub fn prove_range(&self, amount: u64, bits: usize) -> Result<(Vec<u8>, [u8; 32])> {
        if bits == 0 || bits > 64 { return Err(anyhow!("invalid range bit length")); }
        let r_bytes = self.blinding_factor_bytes()?;
        let r = Scalar::from_bytes_mod_order(r_bytes);

        // Build transcript and produce proof
        let mut transcript = Transcript::new(b"C0DL3-CT-RangeProof");
        let (proof, committed): (bulletproofs::RangeProof, CompressedRistretto) =
            bulletproofs::RangeProof::prove_single(
                &BP_GENS,
                &PEDERSEN_GENS,
                &mut transcript,
                amount,
                &r,
                bits,
            ).map_err(|e| anyhow!("range proof generation failed: {e:?}"))?;

        let committed_bytes = committed.to_bytes();
        if committed_bytes != self.commitment {
            return Err(anyhow!("range proof commitment mismatch"));
        }
        Ok((proof.to_bytes(), committed_bytes))
    }

    /// Verify a Bulletproofs range proof against this commitment
    pub fn verify_range(&self, proof_bytes: &[u8], bits: usize) -> Result<bool> {
        if bits == 0 || bits > 64 { return Err(anyhow!("invalid range bit length")); }
        let proof = bulletproofs::RangeProof::from_bytes(proof_bytes)
            .map_err(|e| anyhow!("invalid range proof bytes: {e:?}"))?;
        let mut transcript = Transcript::new(b"C0DL3-CT-RangeProof");
        let committed = CompressedRistretto(self.commitment);
        match proof.verify_single(&BP_GENS, &PEDERSEN_GENS, &mut transcript, &committed, bits) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_commitment_creation() {
        let amount = 1000u64;
        let commitment = AmountCommitment::new(amount).unwrap();
        
        // Commitment should be 32 bytes
        assert_eq!(commitment.commitment.len(), 32);
        assert_eq!(commitment.blinding_factor.len(), 32);
        
        // Verification should pass
        assert!(commitment.verify(amount).unwrap());
    }
    
    #[test]
    fn test_commitment_hiding() {
        let amount = 5000u64;
        
        // Create two commitments to the same amount
        let c1 = AmountCommitment::new(amount).unwrap();
        let c2 = AmountCommitment::new(amount).unwrap();
        
        // Same amount, different commitments (due to random blinding)
        assert_ne!(c1.commitment, c2.commitment);
        
        // Both should verify correctly
        assert!(c1.verify(amount).unwrap());
        assert!(c2.verify(amount).unwrap());
    }
    
    #[test]
    fn test_commitment_verification() {
        let amount = 10000u64;
        let commitment = AmountCommitment::new(amount).unwrap();
        
        // Correct amount should verify
        assert!(commitment.verify(amount).unwrap());
        
        // Wrong amount should fail
        assert!(!commitment.verify(amount + 1).unwrap());
    }
    
    #[test]
    fn test_homomorphic_addition() {
        let amount1 = 1000u64;
        let amount2 = 2000u64;

        let c1 = AmountCommitment::new(amount1).unwrap();
        let c2 = AmountCommitment::new(amount2).unwrap();
        let c_sum = c1.add(&c2).unwrap();

        // Verify the sum commitment: C_sum should equal commit(amount1+amount2, r1+r2)
        assert!(c_sum.verify(amount1 + amount2).unwrap());
    }

    #[test]
    fn test_homomorphic_subtraction() {
        let amount1 = 5000u64;
        let amount2 = 2000u64;

        let c1 = AmountCommitment::new(amount1).unwrap();
        let c2 = AmountCommitment::new(amount2).unwrap();
        let c_diff = c1.subtract(&c2).unwrap();

        assert!(c_diff.verify(amount1 - amount2).unwrap());
    }
    
    #[test]
    fn test_zero_amount_rejected() {
        let result = AmountCommitment::new(0);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_commitment_hash() {
        let commitment = AmountCommitment::new(1000).unwrap();
        let hash = commitment.hash();
        
        // Hash should be hex string
        assert_eq!(hash.len(), 64); // SHA256 produces 256 bits = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
    
    #[test]
    fn test_commitment_point_conversion() {
        let commitment = AmountCommitment::new(5000).unwrap();
        let point = commitment.to_point().unwrap();
        let compressed = point.compress().to_bytes();
        assert_eq!(commitment.commitment, compressed);
    }

    #[test]
    fn test_range_proof_valid() {
        let amount = 42u64;
        let bits = 64usize;
        let commitment = AmountCommitment::new(amount).unwrap();
        let (proof_bytes, _cBytes) = commitment.prove_range(amount, bits).unwrap();
        assert!(commitment.verify_range(&proof_bytes, bits).unwrap());
    }

    #[test]
    fn test_range_proof_invalid_amount() {
        let amount = 500u64;
        let bits = 16usize; // range up to 65536 (Bulletproofs requires power-of-2 bit sizes)
        let commitment = AmountCommitment::new(amount).unwrap();
        // Prove for a different amount (wrong witness) should fail to verify with this commitment
        let other = AmountCommitment::new(amount + 1).unwrap();
        let (proof_bytes, _c) = other.prove_range(amount + 1, bits).unwrap();
        // Verification against original commitment should fail
        assert!(!commitment.verify_range(&proof_bytes, bits).unwrap());
    }
}
