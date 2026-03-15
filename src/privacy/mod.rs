// Privacy module for zkC0DL3
// All transactions are private by default
// MIGRATION: Moving from STARK placeholders to Bulletproofs Confidential Transactions

use serde::{Deserialize, Serialize};

pub mod user_privacy;
pub mod amount_commitments;
pub mod address_encryption;
pub mod timing_privacy;
pub mod confidential_transactions; // Bulletproofs CT implementation
pub mod block_commitment_proof;    // SHA-256 Merkle commitment tree proof
pub mod stealth_address;           // Monero-style dual-key stealth addresses (Ristretto255)
pub mod shielded_pool;             // ZK state: committed balances + nullifiers + conservation
pub mod commitment_proof;          // Schnorr-style sigma protocol for Pedersen commitment knowledge

// STARK modules - DEPRECATED (placeholders only, being replaced by Bulletproofs CT)
#[cfg(feature = "deprecated-stark")]
pub mod stark_proofs;
#[cfg(feature = "deprecated-stark")]
pub mod production_stark_proofs;
#[cfg(feature = "deprecated-stark")]
pub mod production_stark_core;
#[cfg(feature = "deprecated-stark")]
pub mod transaction_privacy_starks;

pub mod performance_optimization;
pub mod security_audit_prep;
pub mod cross_chain_privacy;
pub mod xfg_winterfell_integration;
pub mod bidirectional_bridge;
pub mod production_deployment_prep;

#[cfg(test)]
mod tests;

// User-level privacy exports
pub use user_privacy::{
    UserPrivacyManager,
    PrivateTransaction,
    PrivateBlock,
    DecryptedTransaction,
};

// Confidential Transactions exports (primary privacy system)
pub use confidential_transactions::{
    AmountCommitment,
};

// Block commitment proof exports
pub use block_commitment_proof::{
    BlockCommitmentProof,
    BlockProofPublicInputs,
    generate_block_commitment_proof,
    verify_block_commitment_proof,
};

// Shielded pool exports (ZK state layer)
pub use shielded_pool::{
    ShieldedPool,
    ShieldedNote,
    SpendProof,
    compute_nullifier,
    compute_balance_commitment,
    verify_balance_conservation,
    create_spend_proof,
};

// Stealth address exports
pub use stealth_address::{
    StealthKeypair,
    StealthPaymentAddress,
    StealthOutput,
    generate_stealth_output,
    scan_output,
    derive_stealth_privkey,
    one_time_address_to_hex,
    encode_stealth_data,
    decode_stealth_data,
};

// Commitment knowledge proof exports
pub use commitment_proof::{
    CommitmentKnowledgeProof,
    prove_commitment_knowledge,
    verify_commitment_knowledge,
    prove_commitment_knowledge_with_commitment,
};

// STARK exports - DEPRECATED (feature-gated)
#[cfg(feature = "deprecated-stark")]
pub use stark_proofs::StarkProof;
#[cfg(feature = "deprecated-stark")]
pub use stark_proofs::StarkProofSystem;

// Legacy amount commitments (being replaced by confidential_transactions)
pub use amount_commitments::{
    AmountCommitment as LegacyAmountCommitment,
    RangeProof as LegacyRangeProof,
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

pub use performance_optimization::{
    OptimizedPrivacySystem,
    ProofCache,
    CacheStats,
    BatchManager,
    ProcessingBatch,
    BatchStatus,
    BatchStats,
    PerformanceBenchmark,
    BenchmarkResult,
};

pub use security_audit_prep::{
    SecurityAuditPrep,
    SecurityDocumentation,
    CryptographicPrimitive,
    SecurityAssumption,
    AttackVector,
    MitigationStrategy,
    ThreatModel,
    ThreatActor,
    Asset,
    ThreatScenario,
    RiskAssessment,
    SecurityControls,
    SecurityControl,
    AuditChecklist,
    AuditItem,
    AuditFinding,
    SecurityValidationResult,
    SecurityMetrics,
};

pub use cross_chain_privacy::{
    CrossChainPrivacyCoordinator,
    BlockchainNetwork,
    NetworkType,
    PrivacyCapabilities,
    BridgeConfiguration,
    BridgeType,
    BridgePrivacySettings,
    CrossChainPrivacyProof,
    CrossChainProofMetadata,
    CrossChainTransactionMapping,
    PrivacyStatus,
    PrivacyBridge,
    BridgeInstance,
    BridgeStatus,
    BridgeStatistics,
    CrossChainMetrics,
    CrossChainPrivacyAnalytics,
    CrossChainAnalyticsData,
    PrivacyTrend,
};

// XFG Winterfell Integration exports
pub use xfg_winterfell_integration::{
    XfgWinterfellManager,
    VerifiedBurn,
    VerifiedColdDeposit,
    HeatTokenState,
    PendingL1Mint,
    L1MintStatus,
    MessageBridgeStatus,
    TokenType,
    VerificationStatus,
    YieldGenerationState,
    YieldPool,
    YieldPoolType,
    PoolStatus,
    FuegoConnection,
    XfgWinterfellMetrics,
    SyncStatus,
    SyncResult,
    FuegoTxExtraTag,
};

// Bidirectional Bridge exports
pub use bidirectional_bridge::{
    BidirectionalBridgeManager,
    C0dl3ToFuegoBridge,
    FuegoToC0dl3Bridge,
    BridgeSyncState,
    C0dl3Event,
    C0dl3EventType,
    FuegoRpcClient,
    EventBatchConfig,
    BidirectionalMetrics,
};

/// Privacy feature flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyFlags {
    pub anonymous_validators: bool,
    pub encrypted_transactions: bool,
    pub private_governance: bool,
    pub cross_chain_privacy: bool,
}

impl Default for PrivacyFlags {
    fn default() -> Self {
        Self {
            anonymous_validators: false,
            encrypted_transactions: false,
            private_governance: false,
            cross_chain_privacy: false,
        }
    }
}

/// Privacy engine coordinator
pub struct PrivacyEngine {
    privacy_flags: PrivacyFlags,
}

impl PrivacyEngine {
    pub fn new(privacy_flags: PrivacyFlags) -> Self {
        Self { privacy_flags }
    }

    pub fn get_privacy_flags(&self) -> &PrivacyFlags {
        &self.privacy_flags
    }
}
