use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::{Result, anyhow};
use tracing::{info, warn};

/// Treasury Integration for Omni-Mixer
/// Manages HEAT and CD token allocation for privacy obfuscation

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryPool {
    pub token: String,
    pub total_supply: u128,
    pub allocated_for_mixing: u128,
    pub available_balance: u128,
    pub last_allocation: u64,
    pub allocation_history: Vec<TreasuryAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryAllocation {
    pub round_id: String,
    pub amount: u128,
    pub timestamp: u64,
    pub purpose: AllocationPurpose,
    pub return_expected: u64, // timestamp when assets should return
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AllocationPurpose {
    MixingObfuscation,
    PrivacyEnhancement,
    EmergencyLiquidity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryMetrics {
    pub total_heat_allocated: u128,
    pub total_cd_allocated: u128,
    pub current_utilization_ratio: f64,
    pub average_allocation_time: u64,
    pub total_allocations: u64,
    pub successful_returns: u64,
}

pub struct TreasuryIntegrator {
    pools: Arc<Mutex<HashMap<String, TreasuryPool>>>,
    metrics: Arc<Mutex<TreasuryMetrics>>,
}

impl TreasuryIntegrator {
    pub fn new() -> Result<Self> {
        let pools = HashMap::new();
        let metrics = TreasuryMetrics {
            total_heat_allocated: 0,
            total_cd_allocated: 0,
            current_utilization_ratio: 0.0,
            average_allocation_time: 0,
            total_allocations: 0,
            successful_returns: 0,
        };

        Ok(TreasuryIntegrator {
            pools: Arc::new(Mutex::new(pools)),
            metrics: Arc::new(Mutex::new(metrics)),
        })
    }

    /// Initialize treasury pools with available assets
    pub async fn initialize_pools(&self, heat_amount: u128, cd_amount: u128) -> Result<()> {
        let mut pools = self.pools.lock().unwrap();

        // Initialize HEAT pool
        let heat_pool = TreasuryPool {
            token: "HEAT".to_string(),
            total_supply: heat_amount,
            allocated_for_mixing: 0,
            available_balance: heat_amount,
            last_allocation: 0,
            allocation_history: Vec::new(),
        };

        // Initialize CD pool
        let cd_pool = TreasuryPool {
            token: "CD".to_string(),
            total_supply: cd_amount,
            allocated_for_mixing: 0,
            available_balance: cd_amount,
            last_allocation: 0,
            allocation_history: Vec::new(),
        };

        pools.insert("HEAT".to_string(), heat_pool);
        pools.insert("CD".to_string(), cd_pool);

        info!("Treasury pools initialized: HEAT={}, CD={}", heat_amount, cd_amount);
        Ok(())
    }

    /// Allocate treasury assets for a mixing round
    pub async fn allocate_for_mixing(&self, round_id: &str, requested_amount: u128) -> Result<HashMap<String, u128>> {
        let mut pools = self.pools.lock().unwrap();
        let mut metrics = self.metrics.lock().unwrap();

        let mut allocated = HashMap::new();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Strategy: Allocate 60% HEAT, 40% CD for optimal obfuscation
        let heat_allocation = (requested_amount as f64 * 0.6) as u128;
        let cd_allocation = (requested_amount as f64 * 0.4) as u128;

        // Check HEAT availability
        if let Some(heat_pool) = pools.get_mut("HEAT") {
            if heat_pool.available_balance >= heat_allocation {
                heat_pool.allocated_for_mixing += heat_allocation;
                heat_pool.available_balance -= heat_allocation;
                heat_pool.last_allocation = current_time;

                let allocation = TreasuryAllocation {
                    round_id: round_id.to_string(),
                    amount: heat_allocation,
                    timestamp: current_time,
                    purpose: AllocationPurpose::MixingObfuscation,
                    return_expected: current_time + 3600, // 1 hour
                };

                heat_pool.allocation_history.push(allocation);
                allocated.insert("HEAT".to_string(), heat_allocation);
                metrics.total_heat_allocated += heat_allocation;
            }
        }

        // Check CD availability
        if let Some(cd_pool) = pools.get_mut("CD") {
            if cd_pool.available_balance >= cd_allocation {
                cd_pool.allocated_for_mixing += cd_allocation;
                cd_pool.available_balance -= cd_allocation;
                cd_pool.last_allocation = current_time;

                let allocation = TreasuryAllocation {
                    round_id: round_id.to_string(),
                    amount: cd_allocation,
                    timestamp: current_time,
                    purpose: AllocationPurpose::MixingObfuscation,
                    return_expected: current_time + 3600, // 1 hour
                };

                cd_pool.allocation_history.push(allocation);
                allocated.insert("CD".to_string(), cd_allocation);
                metrics.total_cd_allocated += cd_allocation;
            }
        }

        metrics.total_allocations += 1;

        if !allocated.is_empty() {
            info!("Allocated treasury assets for round {}: {:?}", round_id, allocated);
        } else {
            warn!("Failed to allocate sufficient treasury assets for round {}", round_id);
        }

        Ok(allocated)
    }

    /// Return allocated assets after mixing round completion
    pub async fn return_allocated_assets(&self, round_id: &str) -> Result<()> {
        let mut pools = self.pools.lock().unwrap();
        let mut metrics = self.metrics.lock().unwrap();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Return HEAT assets
        if let Some(heat_pool) = pools.get_mut("HEAT") {
            if let Some(allocation_index) = heat_pool.allocation_history
                .iter()
                .position(|a| a.round_id == round_id) {
                let allocation = &heat_pool.allocation_history[allocation_index];

                heat_pool.allocated_for_mixing -= allocation.amount;
                heat_pool.available_balance += allocation.amount;

                heat_pool.allocation_history.remove(allocation_index);
                metrics.successful_returns += 1;

                info!("Returned HEAT allocation for round {}: {}", round_id, allocation.amount);
            }
        }

        // Return CD assets
        if let Some(cd_pool) = pools.get_mut("CD") {
            if let Some(allocation_index) = cd_pool.allocation_history
                .iter()
                .position(|a| a.round_id == round_id) {
                let allocation = &cd_pool.allocation_history[allocation_index];

                cd_pool.allocated_for_mixing -= allocation.amount;
                cd_pool.available_balance += allocation.amount;

                cd_pool.allocation_history.remove(allocation_index);
                metrics.successful_returns += 1;

                info!("Returned CD allocation for round {}: {}", round_id, allocation.amount);
            }
        }

        Ok(())
    }

    /// Get treasury utilization metrics
    pub async fn get_utilization_metrics(&self) -> Result<serde_json::Value> {
        let pools = self.pools.lock().unwrap();
        let metrics = self.metrics.lock().unwrap();

        let mut total_allocated = 0u128;
        let mut total_available = 0u128;

        for pool in pools.values() {
            total_allocated += pool.allocated_for_mixing;
            total_available += pool.available_balance;
        }

        let utilization_ratio = if total_allocated + total_available > 0 {
            total_allocated as f64 / (total_allocated + total_available) as f64
        } else {
            0.0
        };

        let utilization_data = json!({
            "total_allocated": total_allocated,
            "total_available": total_available,
            "utilization_ratio": utilization_ratio,
            "heat_allocated": metrics.total_heat_allocated,
            "cd_allocated": metrics.total_cd_allocated,
            "total_allocations": metrics.total_allocations,
            "successful_returns": metrics.successful_returns,
            "pool_details": pools.values().collect::<Vec<_>>(),
        });

        Ok(utilization_data)
    }

    /// Emergency treasury rotation for enhanced privacy
    pub async fn emergency_rotation(&self) -> Result<()> {
        let mut pools = self.pools.lock().unwrap();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        info!("Starting emergency treasury rotation");

        // Rotate HEAT and CD allocations to break patterns
        for (token, pool) in pools.iter_mut() {
            if pool.allocated_for_mixing > 0 {
                // Reallocate 20% of current allocations randomly
                let rotation_amount = (pool.allocated_for_mixing as f64 * 0.2) as u128;

                if rotation_amount > 0 {
                    pool.allocated_for_mixing -= rotation_amount;
                    pool.available_balance += rotation_amount;

                    // Create new pseudo-allocation for tracking
                    let allocation = TreasuryAllocation {
                        round_id: format!("rotation_{}", current_time),
                        amount: rotation_amount,
                        timestamp: current_time,
                        purpose: AllocationPurpose::PrivacyEnhancement,
                        return_expected: current_time + 1800, // 30 minutes
                    };

                    pool.allocation_history.push(allocation);
                }
            }
        }

        info!("Emergency treasury rotation completed");
        Ok(())
    }

    /// Check if treasury has sufficient assets for allocation
    pub async fn check_availability(&self, requested_amount: u128) -> Result<bool> {
        let pools = self.pools.lock().unwrap();

        let heat_available = pools.get("HEAT")
            .map(|p| p.available_balance)
            .unwrap_or(0);
        let cd_available = pools.get("CD")
            .map(|p| p.available_balance)
            .unwrap_or(0);

        let total_available = heat_available + cd_available;
        Ok(total_available >= requested_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_treasury_initialization() {
        let integrator = TreasuryIntegrator::new().unwrap();
        integrator.initialize_pools(1000000, 500000).await.unwrap();

        let metrics = integrator.get_utilization_metrics().await.unwrap();
        assert_eq!(metrics["total_available"], 1500000);
        assert_eq!(metrics["utilization_ratio"], 0.0);
    }

    #[tokio::test]
    async fn test_asset_allocation() {
        let integrator = TreasuryIntegrator::new().unwrap();
        integrator.initialize_pools(1000000, 500000).await.unwrap();

        let allocation = integrator.allocate_for_mixing("round_1", 10000).await.unwrap();

        assert!(allocation.contains_key("HEAT"));
        assert!(allocation.contains_key("CD"));

        let metrics = integrator.get_utilization_metrics().await.unwrap();
        assert!(metrics["total_allocated"].as_u64().unwrap() > 0);
        assert!(metrics["utilization_ratio"].as_f64().unwrap() > 0.0);
    }
}
