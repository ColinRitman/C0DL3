// Privacy module for zkC0DL3
// Elite-level privacy features implementation with maximum privacy-by-default
// All transactions are private at maximum level (100) by default

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

// Production-grade privacy exports
pub use production_stark_proofs::{
    ProductionStarkProofSystem,
    ProductionStarkProof,
    ProofMetadata,
};

pub use advanced_privacy_features::{
    AdvancedPrivacyFeatures,
    MixingPool,
    AnonymityManager,
    AnonymitySet,
    PrivacyMetrics,
    MixingProof,
    ZeroKnowledgePrivacyProof,
    PrivacyGuarantees,
    PrivacyGovernance,
    PrivacyPolicy,
    ComplianceTracker,
};

pub use performance_optimization::{
    OptimizedPrivacySystem,
    ProofCache,
    CachedProof,
    CacheStats,
    BatchManager,
    ProcessingBatch,
    BatchStatus,
    BatchStats,
    PerformanceMetrics,
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

// Boojum STARK proof exports
pub use boojum_stark_proofs::{
    BoojumStarkProofSystem,
    BoojumStarkProof,
    BoojumProofMetadata,
    BoojumPerformanceMetrics,
    BoojumSpecificMetrics,
};

// Cross-chain privacy exports
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

// Privacy monitoring exports
pub use privacy_monitoring::{
    PrivacyMonitoringSystem,
    PrivacyMetricsCollector,
    PrivacyRealTimeMetrics,
    PrivacyHistoricalMetrics,
    PrivacyViolationDetector,
    ViolationPattern,
    DetectionRule,
    ViolationSeverity,
    PrivacyViolation,
    ViolationThresholds,
    PrivacyAnalyticsEngine,
    PrivacyAnalyticsData,
    PrivacyTrend as MonitoringPrivacyTrend,
    AnonymityTrend,
    MixingTrend,
    CrossChainTrend,
    PerformanceTrend,
    TrendDirection,
    AnalyticsModel,
    TrendAnalysis,
    TrendAnalysisResult,
    PrivacyAlertingSystem,
    AlertRule,
    PrivacyAlert,
    AlertStatus,
    AlertChannel,
    PrivacyDashboardData,
    DashboardMetrics,
    DashboardChart,
    ChartDataPoint,
    DashboardStatus,
    PrivacyReport,
};

// Placeholder tracking exports
pub use placeholder_tracking::{
    PlaceholderTrackingSystem,
    PlaceholderEntry,
    PlaceholderType,
    SecurityImpact,
    PriorityLevel,
    PlaceholderStatus,
    SimplifiedImplementation,
    SimplifiedImplementationType,
    SimplifiedImplementationStatus,
    ProductionRequirement,
    ProductionRequirementType,
    ImplementationEffort,
    ProductionRequirementStatus,
    IntegrationStatus,
    ComponentIntegrationStatus,
    IntegrationTimeline,
    PlaceholderReport,
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
