// zkC0DL3 Prover Economics
// Gas fee routing and prover reward calculation.
//
// Reward model (no inflation — HEAT is proof-of-burn only):
//   1. Gas base fee pool  — accumulated per block, paid to winning prover
//   2. Gas priority fee   — paid to block proposer (separate from prover)
//
// HEAT supply is fixed: XFG burned on Fuego → HEAT minted, tracked via Ethereal_XFG
// accounting. No new HEAT is ever created by block rewards or subsidies.
//
// COLDAO governance token is handled in src/tokens/coldao.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── HEAT unit: fwei ──────────────────────────────────────────────────────────
// 1 HEAT = 1_000_000 fwei  (6 decimal places, analogous to gwei for HEAT)

/// Base gas fee fraction that routes to the prover pool: 100% of base fee.
pub const BASE_FEE_TO_PROVER_PCT: u64 = 100;

/// Priority fee fraction that routes to the block proposer: 100% of priority fee.
pub const PRIORITY_FEE_TO_PROPOSER_PCT: u64 = 100;

// ── Proof window ─────────────────────────────────────────────────────────────

/// Default seconds a prover has to submit a valid proof after block proposal.
pub const DEFAULT_PROOF_WINDOW_SECS: u64 = 90;

// ── Gas fee split ─────────────────────────────────────────────────────────────

/// Split a total gas fee into prover pool (base fee) and proposer tip (priority).
///
/// `gas_price` is the total fwei/gas the sender paid.
/// `base_fee`  is the protocol-level minimum (here routed to prover).
/// `priority`  = gas_price - base_fee → proposer tip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasFeeSplit {
    /// fwei amount routed to the block's prover pool.
    pub prover_share: u64,
    /// fwei amount routed to the block proposer.
    pub proposer_share: u64,
}

impl GasFeeSplit {
    /// Split `total_gas_fwei` using `base_fee_per_gas` as the base/prover portion.
    pub fn split(total_gas_fwei: u64, gas_used: u64, base_fee_per_gas: u64) -> Self {
        let base_total = gas_used.saturating_mul(base_fee_per_gas);
        let base_total = base_total.min(total_gas_fwei); // cannot exceed total
        let priority_total = total_gas_fwei.saturating_sub(base_total);

        GasFeeSplit {
            prover_share: base_total * BASE_FEE_TO_PROVER_PCT / 100,
            proposer_share: priority_total * PRIORITY_FEE_TO_PROPOSER_PCT / 100,
        }
    }

    /// Simplified split: all gas to prover, nothing to proposer.
    pub fn all_to_prover(total_gas_fwei: u64) -> Self {
        GasFeeSplit {
            prover_share: total_gas_fwei,
            proposer_share: 0,
        }
    }
}

// ── Prover reward calculation ─────────────────────────────────────────────────

/// Reward a prover earns for proving a block.
/// Sole source: gas base fees accumulated in the block (no inflation/subsidy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverReward {
    /// Block height that was proved.
    pub block_height: u64,
    /// Address of the winning prover.
    pub prover_address: String,
    /// Gas base fee pool (fwei) accumulated in this block.
    pub gas_pool: u64,
    /// Total HEAT reward (= gas_pool; no subsidy).
    pub total_heat: u64,
}

impl ProverReward {
    pub fn calculate(block_height: u64, prover_address: String, gas_pool_fwei: u64) -> Self {
        ProverReward {
            block_height,
            prover_address,
            gas_pool: gas_pool_fwei,
            total_heat: gas_pool_fwei,
        }
    }
}

// ── Block economics state ─────────────────────────────────────────────────────

/// Per-block economic state: tracks gas accumulation and proposer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockEconomics {
    /// Block proposer address (set at block proposal time).
    pub proposer_address: String,
    /// Accumulated prover gas pool (fwei). Added to as txs are executed.
    pub prover_gas_pool: u64,
    /// Accumulated proposer tips (fwei).
    pub proposer_tips: u64,
    /// Total gas used in this block.
    pub total_gas_used: u64,
}

impl BlockEconomics {
    pub fn new(proposer_address: String) -> Self {
        Self {
            proposer_address,
            prover_gas_pool: 0,
            proposer_tips: 0,
            total_gas_used: 0,
        }
    }

    /// Record gas from one transaction execution.
    /// `gas_used` is the gas consumed, `gas_price` is total fwei/gas the sender paid.
    /// `base_fee_per_gas` is the minimum protocol fee per gas (routes to prover).
    pub fn record_tx_gas(&mut self, gas_used: u64, gas_price: u64, base_fee_per_gas: u64) {
        let total_fwei = gas_used.saturating_mul(gas_price);
        let split = GasFeeSplit::split(total_fwei, gas_used, base_fee_per_gas);
        self.prover_gas_pool = self.prover_gas_pool.saturating_add(split.prover_share);
        self.proposer_tips = self.proposer_tips.saturating_add(split.proposer_share);
        self.total_gas_used = self.total_gas_used.saturating_add(gas_used);
    }
}

// ── Prover registry (in-memory) ───────────────────────────────────────────────

/// Tracks lifetime stats for each prover address.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProverStats {
    /// How many blocks this address has proved.
    pub blocks_proved: u64,
    /// Total HEAT earned (fwei) from gas fees.
    pub total_heat_earned: u64,
    /// Last proved block height.
    pub last_proved_block: u64,
}

/// In-memory prover registry: address → lifetime stats.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProverRegistry {
    pub provers: HashMap<String, ProverStats>,
}

impl ProverRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `prover_address` proved block `block_height` and earned `heat_fwei` HEAT.
    pub fn record_proof(&mut self, prover_address: &str, block_height: u64, heat_fwei: u64) {
        let entry = self.provers.entry(prover_address.to_string()).or_default();
        entry.blocks_proved += 1;
        entry.total_heat_earned = entry.total_heat_earned.saturating_add(heat_fwei);
        entry.last_proved_block = block_height;
    }

    /// Leaderboard: top provers sorted by blocks proved.
    pub fn leaderboard(&self) -> Vec<(String, ProverStats)> {
        let mut entries: Vec<_> = self
            .provers
            .iter()
            .map(|(addr, stats)| (addr.clone(), stats.clone()))
            .collect();
        entries.sort_by(|a, b| b.1.blocks_proved.cmp(&a.1.blocks_proved));
        entries
    }
}

// ── Economic summary ──────────────────────────────────────────────────────────

/// Public-facing economics summary for the /economics endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicsSummary {
    pub total_provers_registered: usize,
    pub proof_window_secs: u64,
    /// Human-readable description of the prover reward model.
    pub prover_reward_model: String,
    /// Human-readable description of HEAT supply model.
    pub heat_supply_model: String,
}

impl EconomicsSummary {
    pub fn build(registry: &ProverRegistry, _current_height: u64) -> Self {
        EconomicsSummary {
            total_provers_registered: registry.provers.len(),
            proof_window_secs: DEFAULT_PROOF_WINDOW_SECS,
            prover_reward_model: "gas-fees-only (no inflation — HEAT is proof-of-burn)".to_string(),
            heat_supply_model: "fixed: XFG burned on Fuego → HEAT minted via Ethereal_XFG accounting".to_string(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_fee_split_all_base() {
        // gas_price == base_fee → all goes to prover
        let split = GasFeeSplit::split(21_000 * 100, 21_000, 100);
        assert_eq!(split.prover_share, 21_000 * 100);
        assert_eq!(split.proposer_share, 0);
    }

    #[test]
    fn test_gas_fee_split_with_priority() {
        // base_fee = 80, gas_price = 100, gas_used = 21_000
        // prover: 21_000 * 80 = 1_680_000, proposer: 21_000 * 20 = 420_000
        let split = GasFeeSplit::split(21_000 * 100, 21_000, 80);
        assert_eq!(split.prover_share, 21_000 * 80);
        assert_eq!(split.proposer_share, 21_000 * 20);
    }

    #[test]
    fn test_prover_reward_gas_only() {
        let reward = ProverReward::calculate(
            0,
            "0xprover_address_0000000000000000000".to_string(),
            500_000,
        );
        assert_eq!(reward.gas_pool, 500_000);
        assert_eq!(reward.total_heat, 500_000); // no subsidy
    }

    #[test]
    fn test_prover_reward_zero_gas() {
        // Empty block: prover earns nothing
        let reward = ProverReward::calculate(
            42,
            "0xprover_address_0000000000000000000".to_string(),
            0,
        );
        assert_eq!(reward.total_heat, 0);
    }

    #[test]
    fn test_block_economics_gas_accumulation() {
        let mut econ = BlockEconomics::new("0xproposer_address_00000000000000".to_string());
        // Two txs: gas_used=21_000, gas_price=100, base_fee=80
        econ.record_tx_gas(21_000, 100, 80);
        econ.record_tx_gas(21_000, 100, 80);
        // Each tx: prover gets 21_000*80=1_680_000, proposer gets 21_000*20=420_000
        assert_eq!(econ.prover_gas_pool, 2 * 21_000 * 80);
        assert_eq!(econ.proposer_tips, 2 * 21_000 * 20);
        assert_eq!(econ.total_gas_used, 42_000);
    }

    #[test]
    fn test_prover_registry_record_and_leaderboard() {
        let mut registry = ProverRegistry::new();
        registry.record_proof("0xprover_a_addr_000000000000000000", 1, 1_000_000);
        registry.record_proof("0xprover_b_addr_000000000000000000", 2, 2_000_000);
        registry.record_proof("0xprover_a_addr_000000000000000000", 3, 1_000_000);

        let lb = registry.leaderboard();
        assert_eq!(lb[0].0, "0xprover_a_addr_000000000000000000"); // a has 2 blocks
        assert_eq!(lb[0].1.blocks_proved, 2);
        assert_eq!(lb[0].1.total_heat_earned, 2_000_000);
        assert_eq!(lb[1].1.blocks_proved, 1);
    }

    #[test]
    fn test_economics_summary() {
        let registry = ProverRegistry::new();
        let summary = EconomicsSummary::build(&registry, 0);
        assert_eq!(summary.total_provers_registered, 0);
        assert_eq!(summary.proof_window_secs, DEFAULT_PROOF_WINDOW_SECS);
        assert!(summary.prover_reward_model.contains("no inflation"));
    }
}
