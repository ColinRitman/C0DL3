use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use secp256k1::{Secp256k1, PublicKey, SecretKey, ecdsa};
use rand::Rng;
use anyhow::{Result, anyhow};
use tracing::{info, debug};

/// Stealth Address Implementation for C0DL3
/// Provides recipient privacy through one-time addresses

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthAddress {
    pub ephemeral_public_key: Vec<u8>,
    pub stealth_address: Vec<u8>,
    pub view_tag: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthMeta {
    pub stealth_address: StealthAddress,
    pub encrypted_memo: Vec<u8>,
    pub nonce: Vec<u8>,
}

#[derive(Debug)]
pub struct StealthAddressGenerator {
    secp: Secp256k1<secp256k1::All>,
}

impl StealthAddressGenerator {
    pub fn new() -> Self {
        StealthAddressGenerator {
            secp: Secp256k1::new(),
        }
    }

    /// Generate a stealth address for a recipient
    pub fn generate_stealth_address(&self, recipient_public_key: &PublicKey) -> Result<StealthAddress> {
        // Generate ephemeral keypair
        let mut rng = rand::thread_rng();
        let ephemeral_secret = SecretKey::from_slice(&rng.gen::<[u8; 32]>())?;
        let ephemeral_public = PublicKey::from_secret_key(&self.secp, &ephemeral_secret);

        // Compute shared secret: s = e * P_r where P_r is recipient's public key
        let shared_secret_point = recipient_public_key.mul_tweak(&self.secp, &ephemeral_secret.into())?;
        let shared_secret = shared_secret_point.serialize();

        // Derive stealth address: A = P_r + hash(shared_secret) * G
        let mut hasher = Sha256::new();
        hasher.update(&shared_secret);
        let hash_result = hasher.finalize();

        let stealth_scalar = SecretKey::from_slice(&hash_result)?;
        let stealth_public = recipient_public_key.combine(&PublicKey::from_secret_key(&self.secp, &stealth_scalar))?;

        // Generate view tag for efficient scanning
        let view_tag = self.generate_view_tag(&shared_secret, &stealth_public)?;

        Ok(StealthAddress {
            ephemeral_public_key: ephemeral_public.serialize().to_vec(),
            stealth_address: stealth_public.serialize().to_vec(),
            view_tag,
        })
    }

    /// Check if a stealth address belongs to us (recipient side)
    pub fn check_stealth_address(&self,
                                stealth_address: &StealthAddress,
                                recipient_secret: &SecretKey) -> Result<Option<SecretKey>> {
        let ephemeral_public = PublicKey::from_slice(&stealth_address.ephemeral_public_key)?;

        // Compute shared secret using our secret key
        let shared_secret_point = ephemeral_public.mul_tweak(&self.secp, recipient_secret)?;
        let shared_secret = shared_secret_point.serialize();

        // Verify view tag
        let expected_view_tag = self.generate_view_tag(&shared_secret, &PublicKey::from_slice(&stealth_address.stealth_address)?)?;
        if expected_view_tag != stealth_address.view_tag {
            return Ok(None); // Not our address
        }

        // Derive the stealth private key
        let mut hasher = Sha256::new();
        hasher.update(&shared_secret);
        let hash_result = hasher.finalize();

        let stealth_scalar = SecretKey::from_slice(&hash_result)?;
        let stealth_private = recipient_secret.add_tweak(&stealth_scalar.into())?;

        Ok(Some(stealth_private))
    }

    fn generate_view_tag(&self, shared_secret: &[u8], stealth_public: &PublicKey) -> Result<Vec<u8>> {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret);
        hasher.update(&stealth_public.serialize());
        let hash = hasher.finalize();

        // Use first byte as view tag
        Ok(hash[0..1].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_address_generation() {
        let generator = StealthAddressGenerator::new();

        // Generate recipient keypair
        let mut rng = rand::thread_rng();
        let recipient_secret = SecretKey::from_slice(&rng.gen::<[u8; 32]>()).unwrap();
        let recipient_public = PublicKey::from_secret_key(&generator.secp, &recipient_secret);

        // Generate stealth address
        let stealth = generator.generate_stealth_address(&recipient_public).unwrap();

        // Verify ownership
        let recovered_secret = generator.check_stealth_address(&stealth, &recipient_secret).unwrap().unwrap();

        // Verify the stealth address matches
        let recovered_public = PublicKey::from_secret_key(&generator.secp, &recovered_secret);
        assert_eq!(recovered_public.serialize().to_vec(), stealth.stealth_address);
    }
}
