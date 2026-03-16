// src/aa/wallet_factory.rs
// Wallet factory — creates new PrivateWallet accounts.
//
// CREATE2 address derivation: address = SHA-256("C0DL3:wallet:" || owner_pubkey || salt)[12..32]
// This makes addresses deterministic and stealth-capable.

use sha2::{Digest, Sha256};
use crate::privacy::shielded_pool::compute_balance_commitment;

/// Derive a wallet address from owner pubkey + salt.
/// address = "0x" || hex(SHA-256("C0DL3:wallet:" || pubkey || salt)[12..32])
pub fn derive_wallet_address(owner_pubkey: &[u8; 32], salt: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"C0DL3:wallet:");
    hasher.update(owner_pubkey);
    hasher.update(salt);
    let hash: [u8; 32] = hasher.finalize().into();
    let addr_bytes = &hash[12..32];
    format!("0x{}", hex::encode(addr_bytes))
}

/// Derive a stealth wallet address for a one-time interaction.
/// Uses ECDH: sender picks ephemeral key, computes shared secret with
/// recipient's pubkey, derives a stealth salt and offset pubkey.
pub fn derive_stealth_address(
    recipient_pubkey: &[u8; 32],
    ephemeral_scalar_bytes: &[u8; 32],
) -> (String, [u8; 32]) {
    use curve25519_dalek_ng::{
        ristretto::CompressedRistretto,
        scalar::Scalar,
        constants::RISTRETTO_BASEPOINT_POINT as G,
    };

    let recipient = CompressedRistretto(*recipient_pubkey)
        .decompress()
        .expect("invalid recipient pubkey");
    let ephemeral = Scalar::from_bytes_mod_order(*ephemeral_scalar_bytes);

    // Shared secret via ECDH
    let shared = (ephemeral * recipient).compress().to_bytes();

    // Stealth salt = H("C0DL3:stealth:" || shared_secret)
    let mut salt_hasher = Sha256::new();
    salt_hasher.update(b"C0DL3:stealth:");
    salt_hasher.update(&shared);
    let salt: [u8; 32] = salt_hasher.finalize().into();

    // Stealth pubkey = recipient_pubkey + H(shared)*G
    let stealth_offset = Scalar::from_bytes_mod_order(salt);
    let stealth_pubkey = (recipient + stealth_offset * G).compress().to_bytes();

    let address = derive_wallet_address(&stealth_pubkey, &salt);
    (address, stealth_pubkey)
}

/// Create the initial AccountState for a new PrivateWallet.
pub fn create_wallet_account(
    address: &str,
    owner_pubkey: [u8; 32],
) -> crate::aa::types::AccountState {
    crate::aa::types::AccountState {
        balance_commitment: compute_balance_commitment(address, 0, 0),
        balance: 0,
        nonce: 0,
        wallet_type: crate::aa::types::WalletType::PrivateWallet,
        owner_pubkey: Some(owner_pubkey),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek_ng::{constants::RISTRETTO_BASEPOINT_POINT as G, scalar::Scalar};
    use rand::RngCore;

    fn random_scalar() -> Scalar {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Scalar::from_bytes_mod_order(bytes)
    }

    #[test]
    fn test_derive_wallet_address_deterministic() {
        let pubkey = (random_scalar() * G).compress().to_bytes();
        let salt = [0x42u8; 32];

        let addr1 = derive_wallet_address(&pubkey, &salt);
        let addr2 = derive_wallet_address(&pubkey, &salt);

        assert_eq!(addr1, addr2, "same inputs must produce same address");
        assert!(addr1.starts_with("0x"), "address must start with 0x");
        assert_eq!(addr1.len(), 42, "address must be 42 chars (0x + 40 hex)");
    }

    #[test]
    fn test_different_salts_different_addresses() {
        let pubkey = (random_scalar() * G).compress().to_bytes();
        let salt_a = [0x01u8; 32];
        let salt_b = [0x02u8; 32];

        let addr_a = derive_wallet_address(&pubkey, &salt_a);
        let addr_b = derive_wallet_address(&pubkey, &salt_b);

        assert_ne!(addr_a, addr_b, "different salts must produce different addresses");
    }

    #[test]
    fn test_stealth_address_derivation() {
        let recipient_scalar = random_scalar();
        let recipient_pubkey = (recipient_scalar * G).compress().to_bytes();
        let ephemeral_scalar = random_scalar();
        let ephemeral_bytes = ephemeral_scalar.to_bytes();

        let (address, stealth_pubkey) = derive_stealth_address(&recipient_pubkey, &ephemeral_bytes);

        assert_ne!(stealth_pubkey, recipient_pubkey, "stealth pubkey must differ from recipient");
        assert!(address.starts_with("0x"), "stealth address must start with 0x");
        assert_eq!(address.len(), 42, "stealth address must be 42 chars");
    }

    #[test]
    fn test_create_wallet_account() {
        let pubkey = (random_scalar() * G).compress().to_bytes();
        let salt = [0xAA; 32];
        let address = derive_wallet_address(&pubkey, &salt);

        let account = create_wallet_account(&address, pubkey);

        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
        assert_eq!(account.wallet_type, crate::aa::types::WalletType::PrivateWallet);
        assert_eq!(account.owner_pubkey, Some(pubkey));
    }
}
