// CD / COLDAO Token — zkC0DL3 L3 Representation
//
// CD is the COLDAO governance token. On Ethereum it is FuegoCOLDAOToken (ERC-1155).
// On C0DL3 L3, CD balances are tracked in-state and map 1:1 to the on-chain token
// when bridged via the COLDDepositProofVerifier flow.
//
// How CD is earned:
//   1. User locks XFG on Fuego (Fuego L1) for a chosen amount × time tier
//   2. A Winterfell STARK proof is generated (commitment version 3)
//   3. Proof is verified by COLDDepositProofVerifier on Arbitrum
//   4. FuegoCOLDAOToken is minted on Ethereum to the user
//   5. User bridges CD to C0DL3 L3 — this module tracks those balances
//
// CD is NOT earned by proving blocks. Block provers earn HEAT (see src/economics.rs).
// CD holders govern protocol parameters (prover subsidy levels, proof window, etc.)
//
// Tier encoding: tier = (amountIndex * 2) + termIndex
//   Even tiers (0, 2, 4, 6) = 3-month lock
//   Odd tiers  (1, 3, 5, 7) = 12-month lock
//
// Decimals:
//   XFG: 7 decimals (1 XFG = 10_000_000 atomic units)
//   CD:  12 decimals (1 CD  = 1_000_000_000_000 atomic units)
//   HEAT: 18 decimals on L1 (on L3 we use fwei, 6 decimals — see economics.rs note)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Constants ─────────────────────────────────────────────────────────────────

/// CD decimals: 1 CD = 10^12 atomic units.
pub const CD_DECIMALS: u32 = 12;
pub const CD_UNIT: u64 = 1_000_000_000_000; // 10^12

/// Number of tiers.
pub const NUM_TIERS: usize = 8;

/// Legacy cutoff: deposits before 2026-01-01 00:00:00 UTC in tiers 6 & 7
/// get 80% APY instead of 33% / 69%.
pub const LEGACY_CUTOFF_TIMESTAMP: u64 = 1_735_689_600;

/// Standard tier CD interest amounts (atomic units, 12 decimals).
/// Index = tier_id (0–7).
/// Source: COLDDepositProofVerifier.sol (canonical on-chain contract).
pub const CD_TIER_INTEREST: [u64; NUM_TIERS] = [
    640_000,        // Tier 0: 0.8 XFG × 3mo  @ 8%
    1_680_000,      // Tier 1: 0.8 XFG × 12mo @ 21%
    16_800_000,     // Tier 2: 8   XFG × 3mo  @ 21%
    26_400_000,     // Tier 3: 8   XFG × 12mo @ 33%
    264_000_000,    // Tier 4: 80  XFG × 3mo  @ 33%
    440_000_000,    // Tier 5: 80  XFG × 12mo @ 55%
    4_400_000_000,  // Tier 6: 800 XFG × 3mo  @ 55%
    5_520_000_000,  // Tier 7: 800 XFG × 12mo @ 69%
];

/// Legacy CD interest amounts (only tier 6 and 7 before LEGACY_CUTOFF_TIMESTAMP).
pub const CD_TIER_INTEREST_LEGACY: [u64; 2] = [
    6_400_000_000, // Legacy Tier 6: 800 XFG × 3mo  @ 80%
    6_400_000_000, // Legacy Tier 7: 800 XFG × 12mo @ 80%
];

/// Lock term in months per tier.
pub const TIER_TERM_MONTHS: [u32; NUM_TIERS] = [3, 12, 3, 12, 3, 12, 3, 12];

/// XFG amount per tier in atomic units (7 decimals: 1 XFG = 10_000_000).
pub const TIER_XFG_ATOMIC: [u64; NUM_TIERS] = [
    8_000_000,      // Tier 0: 0.8 XFG
    8_000_000,      // Tier 1: 0.8 XFG
    80_000_000,     // Tier 2: 8 XFG
    80_000_000,     // Tier 3: 8 XFG
    800_000_000,    // Tier 4: 80 XFG
    800_000_000,    // Tier 5: 80 XFG
    8_000_000_000,  // Tier 6: 800 XFG
    8_000_000_000,  // Tier 7: 800 XFG
];

/// APY in basis points (1 bps = 0.01%).
/// Source: COLDDepositProofVerifier.sol (canonical on-chain contract).
pub const TIER_APY_BPS: [u32; NUM_TIERS] = [
    800,   // Tier 0: 8%
    2100,  // Tier 1: 21%
    2100,  // Tier 2: 21%
    3300,  // Tier 3: 33%
    3300,  // Tier 4: 33%
    5500,  // Tier 5: 55%
    5500,  // Tier 6: 55%
    6900,  // Tier 7: 69%
];

// ── Tier info ─────────────────────────────────────────────────────────────────

/// Static information about a CD deposit tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdTierInfo {
    pub tier_id: u8,
    /// Lock amount in XFG atomic units.
    pub xfg_amount_atomic: u64,
    /// Lock term in months.
    pub term_months: u32,
    /// Annual percentage yield in basis points.
    pub apy_bps: u32,
    /// Standard CD interest minted (atomic units).
    pub cd_interest_atomic: u64,
    /// Legacy CD interest minted (only tiers 6-7 before LEGACY_CUTOFF_TIMESTAMP).
    pub cd_interest_legacy: Option<u64>,
}

impl CdTierInfo {
    /// Returns the tier info for a given tier_id (0–7).
    pub fn for_tier(tier_id: u8) -> Result<Self> {
        if tier_id as usize >= NUM_TIERS {
            return Err(anyhow!("invalid tier_id: {} (max {})", tier_id, NUM_TIERS - 1));
        }
        let i = tier_id as usize;
        let legacy = if tier_id == 6 {
            Some(CD_TIER_INTEREST_LEGACY[0])
        } else if tier_id == 7 {
            Some(CD_TIER_INTEREST_LEGACY[1])
        } else {
            None
        };
        Ok(CdTierInfo {
            tier_id,
            xfg_amount_atomic: TIER_XFG_ATOMIC[i],
            term_months: TIER_TERM_MONTHS[i],
            apy_bps: TIER_APY_BPS[i],
            cd_interest_atomic: CD_TIER_INTEREST[i],
            cd_interest_legacy: legacy,
        })
    }

    /// Returns the CD amount to mint for a deposit in this tier.
    /// `deposit_timestamp` determines legacy eligibility (tiers 6-7 before 2026-01-01).
    pub fn cd_to_mint(&self, deposit_timestamp: u64) -> u64 {
        if self.tier_id >= 6 && deposit_timestamp < LEGACY_CUTOFF_TIMESTAMP {
            // Legacy bonus: 80% APY for tiers 6-7 before 2026
            self.cd_interest_legacy.unwrap_or(self.cd_interest_atomic)
        } else {
            self.cd_interest_atomic
        }
    }
}

// ── CD deposit record ──────────────────────────────────────────────────────────

/// Records a single CD deposit on L3. Created when a COLD STARK proof is processed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdDeposit {
    /// Address that receives CD on L3.
    pub depositor: String,
    /// Tier (0–7).
    pub tier_id: u8,
    /// STARK proof nullifier — prevents replay.
    pub nullifier: [u8; 32],
    /// CD minted (atomic units).
    pub cd_minted: u64,
    /// Unix timestamp of deposit on Fuego L1.
    pub deposit_timestamp: u64,
    /// Whether legacy APY was applied.
    pub is_legacy: bool,
    /// Block height on L3 when this deposit was processed.
    pub processed_at_height: u64,
    /// Whether the XFG principal has been unlocked and claimed back on Fuego.
    pub principal_unlocked: bool,
}

impl CdDeposit {
    pub fn new(
        depositor: String,
        tier_id: u8,
        nullifier: [u8; 32],
        deposit_timestamp: u64,
        processed_at_height: u64,
    ) -> Result<Self> {
        let tier = CdTierInfo::for_tier(tier_id)?;
        let cd_minted = tier.cd_to_mint(deposit_timestamp);
        let is_legacy = tier_id >= 6 && deposit_timestamp < LEGACY_CUTOFF_TIMESTAMP;
        Ok(CdDeposit {
            depositor,
            tier_id,
            nullifier,
            cd_minted,
            deposit_timestamp,
            is_legacy,
            processed_at_height,
            principal_unlocked: false,
        })
    }
}

// ── CD ledger ─────────────────────────────────────────────────────────────────

/// ERC-1155-style CD ledger on L3.
/// Balances are keyed by (address, tier_id) since different tier CDs may have
/// different governance weights in the future.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColdaoLedger {
    /// (address, tier_id) → CD atomic units
    pub balances: HashMap<(String, u8), u64>,
    /// Total CD in circulation on L3 (atomic units).
    pub total_circulating: u64,
}

impl ColdaoLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mint `amount` CD (atomic units) of `tier_id` to `address`.
    pub fn mint(&mut self, address: &str, tier_id: u8, amount: u64) {
        let key = (address.to_string(), tier_id);
        *self.balances.entry(key).or_insert(0) += amount;
        self.total_circulating = self.total_circulating.saturating_add(amount);
    }

    /// Transfer `amount` CD of `tier_id` from `from` to `to`.
    pub fn transfer(&mut self, from: &str, to: &str, tier_id: u8, amount: u64) -> Result<()> {
        let from_bal = self
            .balances
            .get(&(from.to_string(), tier_id))
            .copied()
            .unwrap_or(0);
        if from_bal < amount {
            return Err(anyhow!(
                "insufficient CD tier {}: have {}, need {}",
                tier_id, from_bal, amount
            ));
        }
        *self.balances.entry((from.to_string(), tier_id)).or_insert(0) -= amount;
        *self.balances.entry((to.to_string(), tier_id)).or_insert(0) += amount;
        Ok(())
    }

    /// Returns CD balance for `address` in tier `tier_id`.
    pub fn balance_of(&self, address: &str, tier_id: u8) -> u64 {
        self.balances
            .get(&(address.to_string(), tier_id))
            .copied()
            .unwrap_or(0)
    }

    /// Returns total CD held by `address` across all tiers (governance weight).
    pub fn total_balance_of(&self, address: &str) -> u64 {
        self.balances
            .iter()
            .filter(|((addr, _), _)| addr == address)
            .map(|(_, &bal)| bal)
            .sum()
    }

    /// Returns all (tier_id, balance) pairs for `address`.
    pub fn balances_for(&self, address: &str) -> Vec<(u8, u64)> {
        let mut result: Vec<(u8, u64)> = self
            .balances
            .iter()
            .filter(|((addr, _), &bal)| addr == address && bal > 0)
            .map(|((_, tier), &bal)| (*tier, bal))
            .collect();
        result.sort_by_key(|(t, _)| *t);
        result
    }
}

// ── COLDAO manager ─────────────────────────────────────────────────────────────

/// Top-level CD/COLDAO manager on L3.
/// Validates COLD deposits (STARK proof nullifiers) and mints CD.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColdaoManager {
    /// CD balances (ERC-1155 style).
    pub ledger: ColdaoLedger,
    /// Processed deposit nullifiers — prevents replay.
    pub nullifiers: std::collections::HashSet<[u8; 32]>,
    /// All deposit records.
    pub deposits: Vec<CdDeposit>,
}

impl ColdaoManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a COLD deposit STARK proof and mint CD on L3.
    ///
    /// In production: `nullifier` and tier data come from the verified STARK proof.
    /// On testnet: caller provides tier_id and deposit_timestamp directly.
    pub fn process_cold_deposit(
        &mut self,
        depositor: String,
        tier_id: u8,
        nullifier: [u8; 32],
        deposit_timestamp: u64,
        current_block_height: u64,
    ) -> Result<CdDeposit> {
        // Nullifier protection — each deposit claimed once
        if self.nullifiers.contains(&nullifier) {
            return Err(anyhow!("COLD deposit nullifier already claimed"));
        }
        if depositor.len() < 20 {
            return Err(anyhow!("depositor address too short"));
        }

        let deposit = CdDeposit::new(
            depositor.clone(),
            tier_id,
            nullifier,
            deposit_timestamp,
            current_block_height,
        )?;

        // Mint CD
        self.ledger.mint(&depositor, tier_id, deposit.cd_minted);
        self.nullifiers.insert(nullifier);
        self.deposits.push(deposit.clone());

        Ok(deposit)
    }

    /// Returns CD balance for `address` across all tiers.
    pub fn balance_of(&self, address: &str) -> Vec<(u8, u64)> {
        self.ledger.balances_for(address)
    }

    /// Returns total governance weight (CD) for `address`.
    pub fn governance_weight(&self, address: &str) -> u64 {
        self.ledger.total_balance_of(address)
    }

    /// Returns total CD in circulation on L3.
    pub fn circulating_supply(&self) -> u64 {
        self.ledger.total_circulating
    }

    /// Returns all deposit records for `address`.
    pub fn deposits_for(&self, address: &str) -> Vec<&CdDeposit> {
        self.deposits
            .iter()
            .filter(|d| d.depositor == address)
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_info_all_valid() {
        for t in 0..NUM_TIERS {
            let info = CdTierInfo::for_tier(t as u8).unwrap();
            assert_eq!(info.tier_id, t as u8);
            assert!(info.cd_interest_atomic > 0);
            assert!(info.apy_bps > 0);
        }
    }

    #[test]
    fn test_tier_info_invalid() {
        assert!(CdTierInfo::for_tier(8).is_err());
        assert!(CdTierInfo::for_tier(255).is_err());
    }

    #[test]
    fn test_cd_interest_exact_values() {
        // Verify constants match COLDDepositProofVerifier.sol (canonical)
        assert_eq!(CD_TIER_INTEREST[0], 640_000);        // Tier 0: 0.8 XFG × 3mo @ 8%
        assert_eq!(CD_TIER_INTEREST[1], 1_680_000);      // Tier 1: 0.8 XFG × 12mo @ 21%
        assert_eq!(CD_TIER_INTEREST[2], 16_800_000);     // Tier 2: 8 XFG × 3mo @ 21%
        assert_eq!(CD_TIER_INTEREST[3], 26_400_000);     // Tier 3: 8 XFG × 12mo @ 33%
        assert_eq!(CD_TIER_INTEREST[4], 264_000_000);    // Tier 4: 80 XFG × 3mo @ 33%
        assert_eq!(CD_TIER_INTEREST[5], 440_000_000);    // Tier 5: 80 XFG × 12mo @ 55%
        assert_eq!(CD_TIER_INTEREST[6], 4_400_000_000);  // Tier 6: 800 XFG × 3mo @ 55%
        assert_eq!(CD_TIER_INTEREST[7], 5_520_000_000);  // Tier 7: 800 XFG × 12mo @ 69%
    }

    #[test]
    fn test_legacy_cd_values() {
        assert_eq!(CD_TIER_INTEREST_LEGACY[0], 6_400_000_000); // Legacy Tier 6: 80%
        assert_eq!(CD_TIER_INTEREST_LEGACY[1], 6_400_000_000); // Legacy Tier 7: 80%
    }

    #[test]
    fn test_standard_deposit_tier7() {
        let tier = CdTierInfo::for_tier(7).unwrap();
        // After 2026: standard APY (69%)
        let cd = tier.cd_to_mint(LEGACY_CUTOFF_TIMESTAMP + 1);
        assert_eq!(cd, 5_520_000_000);
    }

    #[test]
    fn test_legacy_deposit_tier7() {
        let tier = CdTierInfo::for_tier(7).unwrap();
        // Before 2026: legacy APY (80%)
        let cd = tier.cd_to_mint(LEGACY_CUTOFF_TIMESTAMP - 1);
        assert_eq!(cd, 6_400_000_000);
    }

    #[test]
    fn test_legacy_cutoff_boundary_tier6() {
        let tier6 = CdTierInfo::for_tier(6).unwrap();
        // Exactly at cutoff = not legacy
        assert_eq!(tier6.cd_to_mint(LEGACY_CUTOFF_TIMESTAMP), CD_TIER_INTEREST[6]);
        // One second before = legacy
        assert_eq!(tier6.cd_to_mint(LEGACY_CUTOFF_TIMESTAMP - 1), CD_TIER_INTEREST_LEGACY[0]);
    }

    #[test]
    fn test_tier0_no_legacy() {
        let tier0 = CdTierInfo::for_tier(0).unwrap();
        // Tier 0 never has legacy bonus
        assert_eq!(tier0.cd_interest_legacy, None);
        let cd = tier0.cd_to_mint(0); // timestamp 0 = very old
        assert_eq!(cd, 640_000); // always standard
    }

    #[test]
    fn test_ledger_mint_and_balance() {
        let mut ledger = ColdaoLedger::new();
        ledger.mint("0xalice_addr_000000000000000000", 7, 5_520_000_000);
        assert_eq!(ledger.balance_of("0xalice_addr_000000000000000000", 7), 5_520_000_000);
        assert_eq!(ledger.total_circulating, 5_520_000_000);
    }

    #[test]
    fn test_ledger_multi_tier_balance() {
        let mut ledger = ColdaoLedger::new();
        ledger.mint("0xalice_addr_000000000000000000", 5, 440_000_000);
        ledger.mint("0xalice_addr_000000000000000000", 7, 5_520_000_000);
        let total = ledger.total_balance_of("0xalice_addr_000000000000000000");
        assert_eq!(total, 440_000_000 + 5_520_000_000);
    }

    #[test]
    fn test_ledger_transfer() {
        let mut ledger = ColdaoLedger::new();
        ledger.mint("0xalice_addr_000000000000000000", 3, 26_400_000);
        ledger.transfer("0xalice_addr_000000000000000000", "0xbob_addr_0000000000000000000", 3, 10_000_000).unwrap();
        assert_eq!(ledger.balance_of("0xalice_addr_000000000000000000", 3), 16_400_000);
        assert_eq!(ledger.balance_of("0xbob_addr_0000000000000000000", 3), 10_000_000);
    }

    #[test]
    fn test_ledger_transfer_insufficient() {
        let mut ledger = ColdaoLedger::new();
        ledger.mint("0xalice_addr_000000000000000000", 0, 640_000);
        assert!(ledger.transfer("0xalice_addr_000000000000000000", "0xbob_addr_0000000000000000000", 0, 640_001).is_err());
    }

    #[test]
    fn test_manager_process_deposit() {
        let mut mgr = ColdaoManager::new();
        let nullifier = [1u8; 32];
        let deposit = mgr.process_cold_deposit(
            "0xalice_addr_000000000000000000".to_string(),
            7, // tier 7: 800 XFG × 12mo
            nullifier,
            LEGACY_CUTOFF_TIMESTAMP + 1000, // standard APY
            100,
        ).unwrap();
        assert_eq!(deposit.cd_minted, 5_520_000_000);
        assert!(!deposit.is_legacy);
        assert_eq!(mgr.circulating_supply(), 5_520_000_000);
    }

    #[test]
    fn test_manager_legacy_deposit() {
        let mut mgr = ColdaoManager::new();
        let nullifier = [2u8; 32];
        let deposit = mgr.process_cold_deposit(
            "0xalice_addr_000000000000000000".to_string(),
            6, // tier 6: 800 XFG × 3mo
            nullifier,
            LEGACY_CUTOFF_TIMESTAMP - 3600, // before 2026 → legacy
            50,
        ).unwrap();
        assert_eq!(deposit.cd_minted, 6_400_000_000); // 80% APY
        assert!(deposit.is_legacy);
    }

    #[test]
    fn test_manager_nullifier_replay_protection() {
        let mut mgr = ColdaoManager::new();
        let nullifier = [3u8; 32];
        mgr.process_cold_deposit(
            "0xalice_addr_000000000000000000".to_string(),
            0, nullifier, 0, 1,
        ).unwrap();
        // Second claim with same nullifier must fail
        let result = mgr.process_cold_deposit(
            "0xbob_addr_0000000000000000000".to_string(),
            0, nullifier, 0, 2,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_short_address_rejected() {
        let mut mgr = ColdaoManager::new();
        let result = mgr.process_cold_deposit(
            "short".to_string(), // < 20 chars
            0, [0u8; 32], 0, 1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_governance_weight() {
        let mut mgr = ColdaoManager::new();
        mgr.process_cold_deposit(
            "0xalice_addr_000000000000000000".to_string(),
            5, [10u8; 32], LEGACY_CUTOFF_TIMESTAMP + 1, 1,
        ).unwrap(); // tier 5: 440M
        mgr.process_cold_deposit(
            "0xalice_addr_000000000000000000".to_string(),
            7, [11u8; 32], LEGACY_CUTOFF_TIMESTAMP + 1, 2,
        ).unwrap(); // tier 7: 5.52B
        let weight = mgr.governance_weight("0xalice_addr_000000000000000000");
        assert_eq!(weight, 440_000_000 + 5_520_000_000);
    }
}
