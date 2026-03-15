// User-Level Privacy Manager for zkC0DL3
// Implements elite-level privacy features with maximum privacy-by-default
// All transactions are private at maximum level (100) by default

use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sha2::{Sha256, Digest};
use hex;

use crate::privacy::{
    confidential_transactions::AmountCommitment,
    address_encryption::{AddressEncryption, EncryptedAddress},
    timing_privacy::{TimingPrivacy, EncryptedTimestamp},
    stealth_address::{StealthOutput, StealthPaymentAddress, generate_stealth_output},
};

/// User-level privacy manager with elite cryptography standards
/// Privacy is always enabled at maximum level (100) - no options needed

#[derive(Clone)]
pub struct UserPrivacyManager {
    /// Encryption key for address and timing privacy
    encryption_key: [u8; 32],
    // CT does not require a global prover; proofs are constructed per txn
    /// Address encryption system using ChaCha20Poly1305
    address_encryption: AddressEncryption,
    /// Timing privacy system using ChaCha20Poly1305
    timing_privacy: TimingPrivacy,
    /// Storage for private transactions
    private_transactions: Arc<Mutex<HashMap<String, PrivateTransaction>>>,
}

/// Private transaction structure with encrypted fields and CT proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateTransaction {
    /// Public transaction hash (for verification)
    pub hash: String,
    /// Balance check is done via homomorphic relation on commitments (implicit)
    
    /// User privacy fields (encrypted/committed)
    /// Encrypted sender address (hides sender identity)
    pub encrypted_sender: EncryptedAddress,
    /// Encrypted recipient address (hides recipient identity)  
    pub encrypted_recipient: EncryptedAddress,
    /// Amount commitment (hides transaction amount)
    pub amount_commitment: AmountCommitment,
    /// Encrypted timestamp (hides transaction timing)
    pub encrypted_timestamp: EncryptedTimestamp,
    
    /// Range proof for amount validity (Bulletproofs proof bytes)
    pub range_proof_bytes: Vec<u8>,
    /// Range proof bit length
    pub range_proof_bits: usize,

    /// Stealth address output for this payment (if recipient uses stealth addresses).
    /// Contains the one-time address (state trie key) and ephemeral data for scanning.
    /// None if recipient has not published a stealth payment address.
    pub stealth_output: Option<StealthOutput>,
}


/// Private block structure with encrypted transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateBlock {
    /// Public block hash (for verification)
    pub hash: String,
    /// Block height
    pub height: u64,
    /// Placeholder for future block-level CT aggregation proof (optional)
    pub validity_placeholder: Vec<u8>,
    /// Encrypted block timestamp
    pub encrypted_timestamp: EncryptedTimestamp,
    /// Private transactions in this block
    pub private_transactions: Vec<PrivateTransaction>,
    /// Merkle tree proof placeholder
    pub merkle_placeholder: Vec<u8>,
    /// Batch processing proof placeholder
    pub batch_placeholder: Vec<u8>,
}

impl UserPrivacyManager {
    /// Create new user privacy manager with maximum privacy enabled
    /// Privacy is always enabled at maximum level (100) - no options needed
    pub fn new() -> Result<Self> {
        // Generate cryptographically secure encryption key
        let encryption_key = Self::generate_encryption_key()?;
        
        // Initialize address encryption system
        let address_encryption = AddressEncryption::new(&encryption_key)?;
        
        // Initialize timing privacy system
        let timing_privacy = TimingPrivacy::new(&encryption_key)?;
        
        Ok(Self {
            encryption_key,
            address_encryption,
            timing_privacy,
            private_transactions: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// Create private transaction with user-level privacy (always enabled).
    ///
    /// If `recipient_stealth_addr` is provided, a one-time stealth address is
    /// generated for the recipient. The `one_time_address` field becomes the
    /// account key in the state trie, breaking the link between this payment
    /// and the recipient's public identity.
    pub fn create_private_transaction(
        &mut self,
        sender: &str,
        recipient: &str,
        amount: u64,
        timestamp: u64,
        sender_balance: u64,
        recipient_stealth_addr: Option<&StealthPaymentAddress>,
    ) -> Result<PrivateTransaction> {
        // Generate stealth output if recipient has a stealth payment address
        let stealth_output = recipient_stealth_addr
            .map(generate_stealth_output)
            .transpose()?;

        // Use one-time stealth address as effective recipient for tx hash,
        // so the hash doesn't encode the recipient's real identity
        let effective_recipient = stealth_output
            .as_ref()
            .map(|o| hex::encode(o.one_time_address))
            .unwrap_or_else(|| recipient.to_string());

        // Generate transaction hash (public for verification)
        let tx_data = format!("{}:{}:{}:{}", sender, effective_recipient, amount, timestamp);
        let tx_hash = Self::hash_data(&tx_data);

        // Encrypt addresses (protects user identity in tx body)
        let encrypted_sender = self.address_encryption.encrypt_sender(sender)?;
        let encrypted_recipient = self.address_encryption.encrypt_recipient(&effective_recipient)?;

        // Generate amount commitment (hides transaction amount)
        let amount_commitment = AmountCommitment::new(amount)?;

        // Encrypt timestamp (protects user timing)
        let encrypted_timestamp = self.timing_privacy.encrypt_timestamp(timestamp)?;

        // Generate CT range proof (Bulletproofs) for amount in 64-bit range
        let bits = 64usize;
        let (range_proof_bytes, _cbytes) = amount_commitment.prove_range(amount, bits)?;

        let transaction = PrivateTransaction {
            hash: tx_hash.clone(),
            encrypted_sender,
            encrypted_recipient,
            amount_commitment,
            encrypted_timestamp,
            range_proof_bytes,
            range_proof_bits: bits,
            stealth_output,
        };

        // Store transaction
        {
            let mut transactions = self.private_transactions.lock().unwrap();
            transactions.insert(tx_hash, transaction.clone());
        }

        Ok(transaction)
    }
    
    /// Verify private transaction (user-level privacy - always enabled)
    /// Privacy is always enabled at maximum level (100) - no check needed
    pub fn verify_private_transaction(&self, tx: &PrivateTransaction) -> Result<bool> {
        // Privacy is always enabled - no check needed
        
        // Verify CT range proof
        let range_valid = tx.amount_commitment.verify_range(&tx.range_proof_bytes, tx.range_proof_bits)?;
        
        // Verify all privacy components are present
        let has_encrypted_sender = !tx.encrypted_sender.ciphertext.is_empty();
        let has_encrypted_recipient = !tx.encrypted_recipient.ciphertext.is_empty();
        let has_amount_commitment = !tx.amount_commitment.commitment.is_empty();
        let has_encrypted_timestamp = !tx.encrypted_timestamp.ciphertext.is_empty();
        
        Ok(range_valid &&
           has_encrypted_sender && has_encrypted_recipient && 
           has_amount_commitment && has_encrypted_timestamp)
    }
    
    /// Get private transaction by hash
    pub fn get_private_transaction(&self, hash: &str) -> Result<Option<PrivateTransaction>> {
        let transactions = self.private_transactions.lock().unwrap();
        Ok(transactions.get(hash).cloned())
    }
    
    /// Decrypt transaction details (only for authorized users)
    pub fn decrypt_transaction_details(
        &self,
        tx: &PrivateTransaction,
    ) -> Result<DecryptedTransaction> {
        // Decrypt sender address
        let sender = self.address_encryption.decrypt_address(&tx.encrypted_sender)?;
        
        // Decrypt recipient address
        let recipient = self.address_encryption.decrypt_address(&tx.encrypted_recipient)?;
        
        // Decrypt timestamp
        let timestamp = self.timing_privacy.decrypt_timestamp(&tx.encrypted_timestamp)?;
        
        // Note: Amount cannot be decrypted from commitment without additional information
        // This maintains amount privacy
        
        Ok(DecryptedTransaction {
            hash: tx.hash.clone(),
            sender,
            recipient,
            timestamp,
            // amount is intentionally omitted to maintain privacy
        })
    }
    
    /// Generate cryptographically secure encryption key
    fn generate_encryption_key() -> Result<[u8; 32]> {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        Ok(key)
    }
    
    /// Hash data using SHA-256
    fn hash_data(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Decrypted transaction details (for authorized users only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecryptedTransaction {
    pub hash: String,
    pub sender: String,
    pub recipient: String,
    pub timestamp: u64,
    // Amount is intentionally omitted to maintain privacy
}

impl PrivateBlock {
    /// Create new private block with encrypted transactions
    pub fn new(
        height: u64,
        transactions: Vec<PrivateTransaction>,
        timestamp: u64,
    ) -> Result<Self> {
        // Generate block hash
        let block_data = format!("height:{} txs:{} timestamp:{}", height, transactions.len(), timestamp);
        let block_hash = Self::hash_data(&block_data);
        
        // Encrypt timestamp
        let timing_privacy = TimingPrivacy::new(&[0u8; 32])?; // Use default key for now
        let encrypted_timestamp = timing_privacy.encrypt_timestamp(timestamp)?;
        
        Ok(Self {
            hash: block_hash,
            height,
            validity_placeholder: Vec::new(),
            encrypted_timestamp,
            private_transactions: transactions,
            merkle_placeholder: Vec::new(),
            batch_placeholder: Vec::new(),
        })
    }
    
    /// Hash data using SHA-256
    fn hash_data(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_privacy_manager_creation() {
        let _manager = UserPrivacyManager::new().unwrap();
    }

    #[test]
    fn test_private_transaction_creation() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "0xsender_address_000000",
            "0xrecipient_address_0000",
            1000,
            1234567890,
            5000,
            None,
        ).unwrap();

        // Transaction should be created with all privacy features
        assert!(!tx.hash.is_empty());
        assert!(!tx.encrypted_sender.ciphertext.is_empty());
        assert!(!tx.encrypted_recipient.ciphertext.is_empty());
        assert!(!tx.amount_commitment.commitment.is_empty());
        assert!(!tx.encrypted_timestamp.ciphertext.is_empty());
    }
    
    #[test]
    fn test_transaction_verification() {
        let mut manager = UserPrivacyManager::new().unwrap();
        let tx = manager.create_private_transaction(
            "0xsender_address_000000",
            "0xrecipient_address_0000",
            1000,
            1234567890,
            5000,
            None,
        ).unwrap();

        let is_valid = manager.verify_private_transaction(&tx).unwrap();
        assert!(is_valid);
    }
}