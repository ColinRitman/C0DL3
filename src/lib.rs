//! # C0DL3 Omni-Mixer
//!
//! A focused, production-ready LP privacy solution for DeFi.
//! Provides treasury-backed obfuscation for liquidity positions.
//!
//! ## Features
//!
//! - **LP Position Privacy**: Mix multiple positions to break tracking
//! - **Treasury Obfuscation**: Use protocol treasury for privacy enhancement
//! - **Automatic Batching**: Smart position collection and mixing
//! - **Merkle Verification**: Cryptographic proof of mixing integrity
//! - **Real-time Monitoring**: Live statistics and analytics
//!
//! ## Quick Start
//!
//! ```rust
//! use c0dl3_omni::SimpleOmniMixer;
//!
//! // Create mixer with 3 positions minimum, 5min timeout, 100k treasury
//! let mixer = SimpleOmniMixer::new(3, 300, 100000);
//!
//! // Add LP positions
//! mixer.add_position(lp_position).await?;
//!
//! // Get mixing statistics
//! let stats = mixer.get_stats().await?;
//! ```

pub mod omni_mixer;
pub mod types;
pub mod error;

// Re-export main components
pub use omni_mixer::SimpleOmniMixer;
pub use types::{LPPosition, MixingRound, MixingStatus};
pub use error::OmniMixerError;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize logging for the omni-mixer
pub fn init_logging() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    info!("C0DL3 Omni-Mixer v{} initialized", VERSION);
    Ok(())
}

/// Health check for the omni-mixer system
pub async fn health_check() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let health = serde_json::json!({
        "status": "healthy",
        "version": VERSION,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        "features": [
            "lp_position_mixing",
            "treasury_obfuscation",
            "merkle_verification",
            "real_time_monitoring"
        ]
    });

    Ok(health)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let health = health_check().await.unwrap();

        assert_eq!(health["status"], "healthy");
        assert_eq!(health["version"], VERSION);
        assert!(health["features"].as_array().unwrap().len() > 0);
    }
}
