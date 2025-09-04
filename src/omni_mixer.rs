use crate::error::{OmniMixerError, Result};
use crate::types::{LPPosition, MixingRound, MixingStatus, MixerStats, TreasuryPool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Configuration for the Omni-Mixer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmniMixerConfig {
    /// Minimum number of positions required to trigger a mixing round
    pub min_positions_per_round: usize,
    /// Maximum timeout for collecting positions (in seconds)
    pub max_round_timeout: u64,
    /// Treasury obfuscation ratio (percentage of total value to use from treasury)
    pub treasury_obfuscation_ratio: f64,
    /// Enable/disable Merkle root verification
    pub enable_merkle_verification: bool,
    /// Maximum number of active mixing rounds
    pub max_active_rounds: usize,
}

impl Default for OmniMixerConfig {
    fn default() -> Self {
        Self {
            min_positions_per_round: 3,
            max_round_timeout: 300, // 5 minutes
            treasury_obfuscation_ratio: 0.1, // 10%
            enable_merkle_verification: true,
            max_active_rounds: 10,
        }
    }
}

/// The main Omni-Mixer implementation
#[derive(Debug)]
pub struct SimpleOmniMixer {
    /// Configuration for the mixer
    config: OmniMixerConfig,
    /// Treasury pool for obfuscation
    treasury: Arc<Mutex<TreasuryPool>>,
    /// Currently active mixing rounds
    active_rounds: Arc<Mutex<HashMap<String, MixingRound>>>,
    /// Completed mixing rounds (for history)
    completed_rounds: Arc<Mutex<Vec<MixingRound>>>,
    /// Statistics tracker
    stats: Arc<Mutex<MixerStats>>,
}

impl SimpleOmniMixer {
    /// Create a new Omni-Mixer with custom configuration
    pub fn new(min_positions: usize, timeout_seconds: u64, treasury_heat: u128) -> Self {
        let config = OmniMixerConfig {
            min_positions_per_round: min_positions,
            max_round_timeout: timeout_seconds,
            ..Default::default()
        };

        let treasury = TreasuryPool::new(treasury_heat, 0); // Start with HEAT only

        Self {
            config,
            treasury: Arc::new(Mutex::new(treasury)),
            active_rounds: Arc::new(Mutex::new(HashMap::new())),
            completed_rounds: Arc::new(Mutex::new(Vec::new())),
            stats: Arc::new(Mutex::new(MixerStats::default())),
        }
    }

    /// Create a new mixer with default configuration
    pub fn new_default() -> Self {
        Self::new(3, 300, 100000) // 3 positions, 5min timeout, 100k HEAT
    }

    /// Add treasury funds (HEAT tokens)
    pub fn add_treasury_funds(&self, heat_amount: u128) -> Result<()> {
        let mut treasury = self.treasury.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock treasury: {}", e))
        })?;

        treasury.heat_balance += heat_amount;
        Ok(())
    }

    /// Add a liquidity position to the mixer
    pub async fn add_position(&self, position: LPPosition) -> Result<String> {
        // Validate position
        self.validate_position(&position)?;

        // Find or create a suitable mixing round
        let round_id = self.find_or_create_round().await?;

        // Add position to the round
        {
            let mut active_rounds = self.active_rounds.lock().map_err(|e| {
                OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
            })?;

            let round = active_rounds.get_mut(&round_id).ok_or_else(|| {
                OmniMixerError::RoundNotFound(format!("Round {} not found", round_id))
            })?;

            round.add_position(position);

            // Check if round should be triggered
            if round.position_count() >= self.config.min_positions_per_round {
                // Trigger mixing in background
                let round_id_clone = round_id.clone();
                let mixer_clone = Arc::new(self.clone());
                tokio::spawn(async move {
                    if let Err(e) = mixer_clone.trigger_mixing(&round_id_clone).await {
                        tracing::error!("Failed to trigger mixing for round {}: {}", round_id_clone, e);
                    }
                });
            }
        }

        // Update statistics
        self.update_stats().await?;

        Ok(round_id)
    }

    /// Manually trigger mixing for a specific round
    pub async fn trigger_mixing(&self, round_id: &str) -> Result<()> {
        let round = {
            let mut active_rounds = self.active_rounds.lock().map_err(|e| {
                OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
            })?;

            let round = active_rounds.get_mut(round_id).ok_or_else(|| {
                OmniMixerError::RoundNotFound(format!("Round {} not found", round_id))
            })?;

            // Check if round can be processed
            if round.status != MixingStatus::Collecting {
                return Err(OmniMixerError::InvalidRoundState(
                    format!("Round {} is not in collecting state", round_id)
                ));
            }

            round.status = MixingStatus::Processing;
            round.clone()
        };

        // Perform the mixing
        match self.process_mixing_round(round).await {
            Ok(processed_round) => {
                // Move to completed rounds
                {
                    let mut active_rounds = self.active_rounds.lock().map_err(|e| {
                        OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
                    })?;
                    active_rounds.remove(round_id);

                    let mut completed_rounds = self.completed_rounds.lock().map_err(|e| {
                        OmniMixerError::Internal(format!("Failed to lock completed rounds: {}", e))
                    })?;
                    completed_rounds.push(processed_round);
                }

                Ok(())
            }
            Err(e) => {
                // Mark round as failed
                let mut active_rounds = self.active_rounds.lock().map_err(|e| {
                    OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
                })?;
                if let Some(round) = active_rounds.get_mut(round_id) {
                    round.fail(e.to_string());
                }
                Err(e)
            }
        }
    }

    /// Get current mixer statistics
    pub async fn get_stats(&self) -> Result<MixerStats> {
        let stats = self.stats.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock stats: {}", e))
        })?;
        Ok(stats.clone())
    }

    /// Get all current positions in active rounds
    pub async fn get_current_positions(&self) -> Result<Vec<LPPosition>> {
        let active_rounds = self.active_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
        })?;

        let mut all_positions = Vec::new();
        for round in active_rounds.values() {
            all_positions.extend(round.positions.clone());
        }

        Ok(all_positions)
    }

    /// Get completed mixing rounds
    pub async fn get_completed_rounds(&self) -> Result<Vec<MixingRound>> {
        let completed_rounds = self.completed_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock completed rounds: {}", e))
        })?;
        Ok(completed_rounds.clone())
    }

    /// Internal method to find or create a suitable mixing round
    async fn find_or_create_round(&self) -> Result<String> {
        let active_rounds = self.active_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
        })?;

        // Find an existing round that's still collecting
        for (round_id, round) in active_rounds.iter() {
            if round.status == MixingStatus::Collecting {
                return Ok(round_id.clone());
            }
        }

        // Check if we can create a new round
        if active_rounds.len() >= self.config.max_active_rounds {
            return Err(OmniMixerError::Internal(
                "Maximum number of active rounds reached".to_string()
            ));
        }

        drop(active_rounds);

        // Create a new round
        let new_round = MixingRound::new();
        let round_id = new_round.id.clone();

        let mut active_rounds = self.active_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
        })?;
        active_rounds.insert(round_id.clone(), new_round);

        Ok(round_id)
    }

    /// Process a mixing round
    async fn process_mixing_round(&self, mut round: MixingRound) -> Result<MixingRound> {
        // Calculate obfuscation amount
        let total_value = round.total_value();
        let obfuscation_amount = (total_value as f64 * self.config.treasury_obfuscation_ratio) as u128;

        // Allocate treasury funds
        {
            let mut treasury = self.treasury.lock().map_err(|e| {
                OmniMixerError::Internal(format!("Failed to lock treasury: {}", e))
            })?;

            treasury.allocate_for_obfuscation(obfuscation_amount)?;
        }

        // Generate Merkle root if enabled
        let merkle_root = if self.config.enable_merkle_verification {
            self.calculate_merkle_root(&round.positions)?
        } else {
            "disabled".to_string()
        };

        // Complete the round
        round.complete(merkle_root, obfuscation_amount);

        Ok(round)
    }

    /// Calculate Merkle root for a set of positions
    fn calculate_merkle_root(&self, positions: &[LPPosition]) -> Result<String> {
        if positions.is_empty() {
            return Ok("empty".to_string());
        }

        let mut hashes: Vec<String> = positions
            .iter()
            .map(|pos| {
                let data = format!("{}{}{}{}{}",
                    pos.id,
                    pos.owner,
                    pos.pool_address,
                    pos.token_a_amount,
                    pos.token_b_amount
                );
                let hash = Sha256::digest(data.as_bytes());
                hex::encode(hash)
            })
            .collect();

        // Build Merkle tree
        while hashes.len() > 1 {
            let mut new_hashes = Vec::new();

            for chunk in hashes.chunks(2) {
                let combined = if chunk.len() == 2 {
                    format!("{}{}", chunk[0], chunk[1])
                } else {
                    format!("{}{}", chunk[0], chunk[0]) // Duplicate for odd numbers
                };

                let hash = Sha256::digest(combined.as_bytes());
                new_hashes.push(hex::encode(hash));
            }

            hashes = new_hashes;
        }

        Ok(hashes.into_iter().next().unwrap_or_default())
    }

    /// Validate a position before adding to mixer
    fn validate_position(&self, position: &LPPosition) -> Result<()> {
        if position.token_a_amount == 0 && position.token_b_amount == 0 {
            return Err(OmniMixerError::InvalidPosition(
                "Position must have at least one non-zero token amount".to_string()
            ));
        }

        if position.pool_address.is_empty() {
            return Err(OmniMixerError::InvalidPosition(
                "Pool address cannot be empty".to_string()
            ));
        }

        Ok(())
    }

    /// Update mixer statistics
    async fn update_stats(&self) -> Result<()> {
        let mut stats = self.stats.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock stats: {}", e))
        })?;

        let active_rounds = self.active_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock active rounds: {}", e))
        })?;
        let completed_rounds = self.completed_rounds.lock().map_err(|e| {
            OmniMixerError::Internal(format!("Failed to lock completed rounds: {}", e))
        })?;

        let total_positions: usize = active_rounds.values()
            .map(|r| r.position_count())
            .sum::<usize>() + completed_rounds.iter()
            .map(|r| r.position_count())
            .sum::<usize>();

        stats.total_positions = total_positions as u64;
        stats.completed_rounds = completed_rounds.len() as u64;
        stats.active_rounds = active_rounds.len();
        stats.total_treasury_used = completed_rounds.iter()
            .map(|r| r.treasury_used)
            .sum();

        if stats.completed_rounds > 0 {
            stats.avg_positions_per_round = total_positions as f64 / stats.completed_rounds as f64;
            stats.success_rate = 1.0; // Simplified - all completed rounds are successful
        }

        Ok(())
    }
}

impl Clone for SimpleOmniMixer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            treasury: Arc::clone(&self.treasury),
            active_rounds: Arc::clone(&self.active_rounds),
            completed_rounds: Arc::clone(&self.completed_rounds),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_mixer_creation() {
        let mixer = SimpleOmniMixer::new(3, 300, 100000);
        let stats = mixer.get_stats().await.unwrap();

        assert_eq!(stats.total_positions, 0);
        assert_eq!(stats.active_rounds, 0);
    }

    #[test]
    async fn test_add_position() {
        let mixer = SimpleOmniMixer::new(3, 300, 100000);
        let position = LPPosition::new(
            "user123".to_string(),
            "pool456".to_string(),
            1000,
            2000,
        );

        let round_id = mixer.add_position(position).await.unwrap();
        assert!(!round_id.is_empty());

        let positions = mixer.get_current_positions().await.unwrap();
        assert_eq!(positions.len(), 1);
    }

    #[test]
    async fn test_merkle_root() {
        let mixer = SimpleOmniMixer::new_default();
        let positions = vec![
            LPPosition::new("user1".to_string(), "pool1".to_string(), 1000, 2000),
            LPPosition::new("user2".to_string(), "pool2".to_string(), 3000, 4000),
        ];

        let root = mixer.calculate_merkle_root(&positions).unwrap();
        assert!(!root.is_empty());
        assert_ne!(root, "empty");
    }

    #[test]
    async fn test_add_treasury_funds() {
        let mixer = SimpleOmniMixer::new(3, 300, 100000);
        mixer.add_treasury_funds(50000).unwrap();

        let treasury = mixer.treasury.lock().unwrap();
        assert_eq!(treasury.heat_balance, 150000);
    }
}
