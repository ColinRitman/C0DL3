// Privacy module for zkC0DL3
// Elite-level privacy features implementation with maximum privacy-by-default
// All transactions are private at maximum level (100) by default

pub mod mining_privacy;
pub mod user_privacy;
pub mod stark_proofs;
pub mod amount_commitments;
pub mod address_encryption;
pub mod timing_privacy;

#[cfg(test)]
mod tests;

// Legacy mining privacy exports (existing functionality)
pub use mining_privacy::{
    MiningPrivacyEngine,
    MiningPrivacyConfig,
    PrivateMiningReward,
    AnonymousRewardClaim,
    PrivacyStats,
    MergeMiningPrivacyConfig,
};

// New user-level privacy exports
pub use user_privacy::{
    UserPrivacyManager,
    PrivateTransaction,
    PrivateBlock,
    DecryptedTransaction,
    StarkProof,
};

pub use stark_proofs::StarkProofSystem;
pub use amount_commitments::{
    AmountCommitment,
    RangeProof,
    CommitmentBatch,
};

pub use address_encryption::{
    AddressEncryption,
    EncryptedAddress,
    AddressEncryptionBatch,
};

pub use timing_privacy::{
    TimingPrivacy,
    EncryptedTimestamp,
    TimestampRangeProof,
    TimingPrivacyBatch,
};

/// Privacy feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFlags {
    pub private_mining_rewards: bool,
    pub anonymous_validators: bool,
    pub encrypted_transactions: bool,
    pub private_governance: bool,
    pub cross_chain_privacy: bool,
}

impl Default for PrivacyFlags {
    fn default() -> Self {
        Self {
            private_mining_rewards: true,
            anonymous_validators: false,
            encrypted_transactions: false,
            private_governance: false,
            cross_chain_privacy: false,
        }
    }
}

/// Privacy engine coordinator
pub struct PrivacyEngine {
    mining_privacy: MiningPrivacyEngine,
    privacy_flags: PrivacyFlags,
}

impl PrivacyEngine {
    pub fn new(privacy_flags: PrivacyFlags) -> Self {
        let mining_config = MiningPrivacyConfig::default();
        let mining_privacy = MiningPrivacyEngine::new(mining_config);
        
        Self {
            mining_privacy,
            privacy_flags,
        }
    }
    
    pub fn get_mining_privacy(&self) -> &MiningPrivacyEngine {
        &self.mining_privacy
    }
    
    pub fn get_privacy_flags(&self) -> &PrivacyFlags {
        &self.privacy_flags
    }
}
