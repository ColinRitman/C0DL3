//! Fuego blockchain units and currency handling
//!
//! This module provides utilities for handling Fuego blockchain units,
//! including conversion between different denominations and gas price calculations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Fuego wei unit (smallest denomination)
/// 1 fwei = 0.001 HEAT = 10^15 wei
pub const FWEI: u128 = 1_000_000_000_000_000;

/// Fuego unit (HEAT token)
/// 1 HEAT = 10^18 wei = 10^3 fwei
pub const HEAT: u128 = 1_000_000_000_000_000_000;

/// Gas price in fwei per gas unit
pub const DEFAULT_GAS_PRICE_FWEI: u64 = 1;

/// Gas price in wei per gas unit
pub const DEFAULT_GAS_PRICE_WEI: u128 = DEFAULT_GAS_PRICE_FWEI as u128 * FWEI;

/// Represents an amount in Fuego units
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FuegoAmount {
    /// Amount in fwei (smallest unit)
    pub fwei: u128,
}

impl FuegoAmount {
    /// Create a new FuegoAmount from fwei
    pub fn from_fwei(fwei: u128) -> Self {
        Self { fwei }
    }

    /// Create a new FuegoAmount from HEAT
    pub fn from_heat(heat: u128) -> Result<Self, &'static str> {
        heat.checked_mul(HEAT)
            .map(|fwei| Self { fwei })
            .ok_or("HEAT amount overflow")
    }

    /// Create a new FuegoAmount from wei
    pub fn from_wei(wei: u128) -> Self {
        Self { fwei: wei / FWEI }
    }

    /// Convert to wei
    pub fn to_wei(&self) -> u128 {
        self.fwei * FWEI
    }

    /// Convert to HEAT
    pub fn to_heat(&self) -> f64 {
        self.fwei as f64 / HEAT as f64
    }

    /// Get amount in fwei
    pub fn as_fwei(&self) -> u128 {
        self.fwei
    }

    /// Add two FuegoAmount values
    pub fn add(&self, other: &FuegoAmount) -> Result<FuegoAmount, &'static str> {
        self.fwei.checked_add(other.fwei)
            .map(|fwei| Self { fwei })
            .ok_or("Addition overflow")
    }

    /// Subtract two FuegoAmount values
    pub fn sub(&self, other: &FuegoAmount) -> Result<FuegoAmount, &'static str> {
        self.fwei.checked_sub(other.fwei)
            .map(|fwei| Self { fwei })
            .ok_or("Subtraction underflow")
    }

    /// Multiply by a scalar
    pub fn mul(&self, scalar: u128) -> Result<FuegoAmount, &'static str> {
        self.fwei.checked_mul(scalar)
            .map(|fwei| Self { fwei })
            .ok_or("Multiplication overflow")
    }

    /// Divide by a scalar
    pub fn div(&self, divisor: u128) -> Result<FuegoAmount, &'static str> {
        if divisor == 0 {
            return Err("Division by zero");
        }
        Ok(Self { fwei: self.fwei / divisor })
    }
}

impl fmt::Display for FuegoAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} fwei ({:.6} HEAT)", self.fwei, self.to_heat())
    }
}

/// Gas price calculator for Fuego transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasPriceCalculator {
    /// Base gas price in fwei per gas unit
    pub base_price_fwei: u64,
    /// Gas price multiplier for congestion
    pub congestion_multiplier: f64,
}

impl GasPriceCalculator {
    /// Create a new gas price calculator
    pub fn new() -> Self {
        Self {
            base_price_fwei: DEFAULT_GAS_PRICE_FWEI,
            congestion_multiplier: 1.0,
        }
    }

    /// Calculate gas price for a transaction
    pub fn calculate_gas_price(&self, gas_limit: u64) -> FuegoAmount {
        let total_fwei = self.base_price_fwei as u128 * gas_limit as u128;
        let adjusted_fwei = (total_fwei as f64 * self.congestion_multiplier) as u128;
        FuegoAmount::from_fwei(adjusted_fwei)
    }

    /// Update congestion multiplier based on network conditions
    pub fn update_congestion(&mut self, network_load: f64) {
        // Simple congestion pricing
        self.congestion_multiplier = 1.0 + (network_load * 0.5).min(2.0);
    }
}

impl Default for GasPriceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Mining reward calculator for Fuego blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningRewardCalculator {
    /// Base block reward in HEAT
    pub base_reward_heat: u64,
    /// Halving interval in blocks
    pub halving_interval: u64,
    /// Current block height
    pub current_height: u64,
}

impl MiningRewardCalculator {
    /// Create a new mining reward calculator
    pub fn new() -> Self {
        Self {
            base_reward_heat: 50, // 50 HEAT base reward
            halving_interval: 210_000, // Standard halving interval
            current_height: 0,
        }
    }

    /// Calculate mining reward for current block height
    pub fn calculate_reward(&self) -> Result<FuegoAmount, &'static str> {
        let halvings = self.current_height / self.halving_interval;
        let reward_heat = self.base_reward_heat >> halvings; // Right shift for halving

        if reward_heat == 0 {
            return Ok(FuegoAmount::from_fwei(0));
        }

        FuegoAmount::from_heat(reward_heat as u128)
    }

    /// Advance to next block
    pub fn next_block(&mut self) {
        self.current_height += 1;
    }
}

impl Default for MiningRewardCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction fee calculator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFeeCalculator {
    /// Gas price calculator
    pub gas_calculator: GasPriceCalculator,
    /// Fee multiplier for priority transactions
    pub priority_multiplier: f64,
}

impl TransactionFeeCalculator {
    /// Create a new transaction fee calculator
    pub fn new() -> Self {
        Self {
            gas_calculator: GasPriceCalculator::new(),
            priority_multiplier: 1.0,
        }
    }

    /// Calculate transaction fee
    pub fn calculate_fee(&self, gas_used: u64, is_priority: bool) -> FuegoAmount {
        let base_fee = self.gas_calculator.calculate_gas_price(gas_used);

        if is_priority {
            let priority_fee_fwei = (base_fee.fwei as f64 * self.priority_multiplier) as u128;
            FuegoAmount::from_fwei(priority_fee_fwei)
        } else {
            base_fee
        }
    }

    /// Set priority multiplier
    pub fn set_priority(&mut self, multiplier: f64) {
        self.priority_multiplier = multiplier.max(1.0);
    }
}

impl Default for TransactionFeeCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuego_amount_conversions() {
        let amount = FuegoAmount::from_heat(1).unwrap();
        assert_eq!(amount.to_heat(), 1.0);
        assert_eq!(amount.as_fwei(), HEAT);

        let fwei_amount = FuegoAmount::from_fwei(FWEI);
        assert_eq!(fwei_amount.to_heat(), 0.001); // 1 fwei = 0.001 HEAT
    }

    #[test]
    fn test_gas_price_calculation() {
        let calculator = GasPriceCalculator::new();
        let gas_price = calculator.calculate_gas_price(21000); // Standard ETH transfer
        assert_eq!(gas_price.as_fwei(), 21000); // 21000 gas * 1 fwei/gas
    }

    #[test]
    fn test_transaction_fee_calculation() {
        let calculator = TransactionFeeCalculator::new();
        let fee = calculator.calculate_fee(21000, false);
        assert_eq!(fee.as_fwei(), 21000);
    }

    #[test]
    fn test_mining_reward_calculation() {
        let calculator = MiningRewardCalculator::new();
        let reward = calculator.calculate_reward().unwrap();
        assert_eq!(reward.to_heat(), 50.0); // 50 HEAT initial reward
    }
}
