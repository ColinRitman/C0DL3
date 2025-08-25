use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use sha2::{Sha256, Digest};
use blake2::{Blake2b, Digest as Blake2Digest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HEATCommitment {
    pub commitment: Vec<u8>,
    pub nullifier: Vec<u8>,
    pub amount: u64,
    pub owner: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldCommitment {
    pub commitment: Vec<u8>,
    pub nullifier: Vec<u8>,
    pub yield_amount: u64,
    pub staking_period: u64,
    pub owner: Vec<u8>,
}

// Simple hash function to replace Poseidon
pub fn simple_hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

impl HEATCommitment {
    pub fn new(amount: u64, owner: Vec<u8>, secret: Vec<u8>) -> Result<Self> {
        info!("Creating HEAT commitment for amount: {}", amount);
        
        // Create commitment by hashing amount + owner + secret
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(&amount.to_le_bytes());
        commitment_data.extend_from_slice(&owner);
        commitment_data.extend_from_slice(&secret);
        
        let commitment = simple_hash(&commitment_data);
        
        // Create nullifier by hashing commitment + secret
        let mut nullifier_data = Vec::new();
        nullifier_data.extend_from_slice(&commitment);
        nullifier_data.extend_from_slice(&secret);
        
        let nullifier = simple_hash(&nullifier_data);
        
        Ok(Self {
            commitment,
            nullifier,
            amount,
            owner,
        })
    }
    
    pub fn verify(&self, secret: &[u8]) -> bool {
        // Recreate commitment to verify
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(&self.amount.to_le_bytes());
        commitment_data.extend_from_slice(&self.owner);
        commitment_data.extend_from_slice(secret);
        
        let expected_commitment = simple_hash(&commitment_data);
        
        // Recreate nullifier to verify
        let mut nullifier_data = Vec::new();
        nullifier_data.extend_from_slice(&expected_commitment);
        nullifier_data.extend_from_slice(secret);
        
        let expected_nullifier = simple_hash(&nullifier_data);
        
        self.commitment == expected_commitment && self.nullifier == expected_nullifier
    }
}

impl YieldCommitment {
    pub fn new(yield_amount: u64, staking_period: u64, owner: Vec<u8>, secret: Vec<u8>) -> Result<Self> {
        info!("Creating Yield commitment for amount: {}, period: {}", yield_amount, staking_period);
        
        // Create commitment by hashing yield_amount + staking_period + owner + secret
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(&yield_amount.to_le_bytes());
        commitment_data.extend_from_slice(&staking_period.to_le_bytes());
        commitment_data.extend_from_slice(&owner);
        commitment_data.extend_from_slice(&secret);
        
        let commitment = simple_hash(&commitment_data);
        
        // Create nullifier by hashing commitment + secret
        let mut nullifier_data = Vec::new();
        nullifier_data.extend_from_slice(&commitment);
        nullifier_data.extend_from_slice(&secret);
        
        let nullifier = simple_hash(&nullifier_data);
        
        Ok(Self {
            commitment,
            nullifier,
            yield_amount,
            staking_period,
            owner,
        })
    }
    
    pub fn verify(&self, secret: &[u8]) -> bool {
        // Recreate commitment to verify
        let mut commitment_data = Vec::new();
        commitment_data.extend_from_slice(&self.yield_amount.to_le_bytes());
        commitment_data.extend_from_slice(&self.staking_period.to_le_bytes());
        commitment_data.extend_from_slice(&self.owner);
        commitment_data.extend_from_slice(secret);
        
        let expected_commitment = simple_hash(&commitment_data);
        
        // Recreate nullifier to verify
        let mut nullifier_data = Vec::new();
        nullifier_data.extend_from_slice(&expected_commitment);
        nullifier_data.extend_from_slice(secret);
        
        let expected_nullifier = simple_hash(&nullifier_data);
        
        self.commitment == expected_commitment && self.nullifier == expected_nullifier
    }
}

// Utility functions for commitment calculations
pub fn calculate_merkle_root(commitments: &[Vec<u8>]) -> Result<Vec<u8>> {
    if commitments.is_empty() {
        return Ok(vec![0u8; 32]);
    }
    
    if commitments.len() == 1 {
        return Ok(commitments[0].clone());
    }
    
    let mut current_level: Vec<Vec<u8>> = commitments.to_vec();
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        
        for chunk in current_level.chunks(2) {
            let mut combined = Vec::new();
            combined.extend_from_slice(&chunk[0]);
            
            if chunk.len() > 1 {
                combined.extend_from_slice(&chunk[1]);
            } else {
                // Pad with zeros if odd number of elements
                combined.extend_from_slice(&vec![0u8; chunk[0].len()]);
            }
            
            let hash = simple_hash(&combined);
            next_level.push(hash);
        }
        
        current_level = next_level;
    }
    
    Ok(current_level[0].clone())
}

pub fn create_merkle_proof(commitments: &[Vec<u8>], index: usize) -> Result<Vec<Vec<u8>>> {
    if index >= commitments.len() {
        return Err(anyhow::anyhow!("Index out of bounds"));
    }
    
    let mut proof = Vec::new();
    let mut current_index = index;
    let mut current_level: Vec<Vec<u8>> = commitments.to_vec();
    
    while current_level.len() > 1 {
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        
        if sibling_index < current_level.len() {
            proof.push(current_level[sibling_index].clone());
        }
        
        // Move to next level
        let mut next_level = Vec::new();
        for chunk in current_level.chunks(2) {
            let mut combined = Vec::new();
            combined.extend_from_slice(&chunk[0]);
            
            if chunk.len() > 1 {
                combined.extend_from_slice(&chunk[1]);
            } else {
                combined.extend_from_slice(&vec![0u8; chunk[0].len()]);
            }
            
            let hash = simple_hash(&combined);
            next_level.push(hash);
        }
        
        current_level = next_level;
        current_index /= 2;
    }
    
    Ok(proof)
}

pub fn verify_merkle_proof(
    root: &[u8],
    leaf: &[u8],
    proof: &[Vec<u8>],
    index: usize,
) -> bool {
    let mut current_hash = leaf.to_vec();
    let mut current_index = index;
    
    for sibling in proof {
        let mut combined = Vec::new();
        
        if current_index % 2 == 0 {
            combined.extend_from_slice(&current_hash);
            combined.extend_from_slice(sibling);
        } else {
            combined.extend_from_slice(sibling);
            combined.extend_from_slice(&current_hash);
        }
        
        current_hash = simple_hash(&combined);
        current_index /= 2;
    }
    
    current_hash == root
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_heat_commitment_creation() {
        let amount = 1000u64;
        let owner = vec![1, 2, 3, 4];
        let secret = vec![5, 6, 7, 8];
        
        let commitment = HEATCommitment::new(amount, owner.clone(), secret.clone()).unwrap();
        
        assert_eq!(commitment.amount, amount);
        assert_eq!(commitment.owner, owner);
        assert!(commitment.verify(&secret));
    }
    
    #[test]
    fn test_yield_commitment_creation() {
        let yield_amount = 500u64;
        let staking_period = 30u64;
        let owner = vec![1, 2, 3, 4];
        let secret = vec![5, 6, 7, 8];
        
        let commitment = YieldCommitment::new(yield_amount, staking_period, owner.clone(), secret.clone()).unwrap();
        
        assert_eq!(commitment.yield_amount, yield_amount);
        assert_eq!(commitment.staking_period, staking_period);
        assert_eq!(commitment.owner, owner);
        assert!(commitment.verify(&secret));
    }
    
    #[test]
    fn test_merkle_root_calculation() {
        let commitments = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![9, 10, 11, 12],
        ];
        
        let root = calculate_merkle_root(&commitments).unwrap();
        assert_eq!(root.len(), 32); // SHA256 hash length
    }
    
    #[test]
    fn test_merkle_proof_verification() {
        let commitments = vec![
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![9, 10, 11, 12],
        ];
        
        let root = calculate_merkle_root(&commitments).unwrap();
        let proof = create_merkle_proof(&commitments, 0).unwrap();
        
        assert!(verify_merkle_proof(&root, &commitments[0], &proof, 0));
    }
}
