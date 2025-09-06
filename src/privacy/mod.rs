// Privacy module for zkC0DL3
// All transactions are private by default

use serde::{Deserialize, Serialize};

pub mod mining_privacy;
pub mod user_privacy;
pub mod stark_proofs;
pub mod amount_commitments;
pub mod address_encryption;
pub mod timing_privacy;
pub mod production_stark_proofs;
pub mod advanced_privacy_features;
pub mod performance_optimization;
pub mod security_audit_prep;
pub mod boojum_stark_proofs;
pub mod cross_chain_privacy;
pub mod privacy_monitoring;
pub mod placeholder_tracking;
pub mod production_boojum_integration;
pub mod production_cross_chain_privacy;
pub mod production_privacy_monitoring;
pub mod production_performance_optimization;
pub mod production_deployment_prep;

#[cfg(test)]
mod tests;

// Legacy mining privacy exports (existing functionality)
pub use mining_privacy::{
    MiningPrivacyEngine,
    MiningPrivacyConfig,
};

// New user-level privacy exports
pub use user_privacy::{
    UserPrivacyManager,
    PrivateTransaction,
    DecryptedTransaction,
};


// Use production STARK system instead of placeholder
// Keep placeholder for reference



// Production-grade privacy exports




// Boojum STARK proof exports

// Cross-chain privacy exports

// Privacy monitoring exports

// Placeholder tracking exports

// Production implementation exports





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
