// Genesis Module — Chain initialization and testnet bootstrap
//
// Defines the genesis state for C0DL3: chain identity, initial allocations,
// and system accounts. On first boot (no persisted state), the node loads
// the genesis config and seeds RollupState with the initial accounts.
//
// Chain IDs (EIP-155):
//   - C0DL3 Testnet: 0xC0D13 (789779)
//   - C0DL3 Mainnet: reserved — set when mainnet launches

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::aa::types::{AccountState, WalletType};
use crate::privacy::shielded_pool::compute_balance_commitment;
use crate::RollupState;
use crate::proving::ProverConfig;

// ── Chain IDs ────────────────────────────────────────────────────────────────

/// C0DL3 Testnet chain ID: 0xC0D13 = 789779
pub const TESTNET_CHAIN_ID: u64 = 0xC0D13;

/// C0DL3 Mainnet chain ID (reserved — set when mainnet launches).
pub const MAINNET_CHAIN_ID: u64 = 0; // placeholder

// ── Genesis Configuration ────────────────────────────────────────────────────

/// Genesis configuration — loaded from file or built-in defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Chain identifier (EIP-155).
    pub chain_id: u64,
    /// Human-readable chain name.
    pub chain_name: String,
    /// Unix timestamp of genesis block.
    pub timestamp: u64,
    /// Initial account allocations.
    pub alloc: Vec<GenesisAlloc>,
    /// Sequencer address (block proposer + settlement submitter).
    pub sequencer: String,
    /// Settlement contract address on zkSync Era (empty = not yet deployed).
    pub settlement_contract: String,
    /// SP1 program verification key hash (hex, empty = mock mode).
    pub program_vkey_hash: String,
}

/// A genesis allocation: seeds an account with an initial balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAlloc {
    /// Account address.
    pub address: String,
    /// Initial balance in fwei.
    pub balance: u64,
    /// Wallet type.
    pub wallet_type: WalletType,
    /// Label (for documentation only).
    pub label: String,
}

impl GenesisConfig {
    /// Built-in testnet genesis.
    pub fn testnet() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            chain_id: TESTNET_CHAIN_ID,
            chain_name: "C0DL3 Testnet".to_string(),
            timestamp: now,
            alloc: vec![
                // Faucet — large balance for distributing testnet HEAT
                GenesisAlloc {
                    address: "0xC0DL3_FAUCET_0000000000000000000001".to_string(),
                    balance: 1_000_000_000_000_000_000, // 1e18 fwei (1B HEAT)
                    wallet_type: WalletType::LegacyEOA,
                    label: "Testnet Faucet".to_string(),
                },
                // Sequencer operator — gas fees for settlement txs
                GenesisAlloc {
                    address: "0xC0DL3_SEQUENCER_000000000000000000001".to_string(),
                    balance: 100_000_000_000_000_000, // 1e17 fwei (100M HEAT)
                    wallet_type: WalletType::LegacyEOA,
                    label: "Sequencer Operator".to_string(),
                },
                // Paymaster — sponsors gas for private wallets
                GenesisAlloc {
                    address: "0xC0DL3_PAYMASTER_00000000000000000001".to_string(),
                    balance: 50_000_000_000_000_000, // 5e16 fwei (50M HEAT)
                    wallet_type: WalletType::LegacyEOA,
                    label: "Default Paymaster".to_string(),
                },
            ],
            sequencer: "0xC0DL3_SEQUENCER_000000000000000000001".to_string(),
            settlement_contract: String::new(), // Set after deploying COLDL3Settlement.sol
            program_vkey_hash: String::new(),   // Set after building SP1 guest program
        }
    }

    /// Load genesis from a JSON file, falling back to built-in testnet config.
    pub fn load_or_default(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                match serde_json::from_str::<GenesisConfig>(&contents) {
                    Ok(config) => {
                        info!("Loaded genesis config from {} (chain_id: {})", path, config.chain_id);
                        config
                    }
                    Err(e) => {
                        tracing::warn!("Invalid genesis file {}: {} — using testnet defaults", path, e);
                        Self::testnet()
                    }
                }
            }
            Err(_) => {
                info!("No genesis file at {} — using built-in testnet config", path);
                Self::testnet()
            }
        }
    }

    /// Apply genesis allocations to a fresh RollupState.
    /// Returns the initialized state with genesis accounts and computed state root.
    pub fn apply(&self, prover_config: ProverConfig) -> RollupState {
        let mut state = RollupState::with_prover_config(prover_config);

        for alloc in &self.alloc {
            let commitment = compute_balance_commitment(&alloc.address, alloc.balance, 0);
            let account = AccountState {
                balance_commitment: commitment,
                balance: alloc.balance,
                nonce: 0,
                wallet_type: alloc.wallet_type.clone(),
                owner_pubkey: None,
            };
            state.accounts.insert(alloc.address.clone(), account);
            info!(
                "Genesis alloc: {} = {} fwei ({})",
                alloc.address, alloc.balance, alloc.label
            );
        }

        // Compute initial state root from genesis accounts
        state.compute_state_root();

        info!(
            "Genesis applied: chain_id={}, accounts={}, state_root={}",
            self.chain_id,
            state.accounts.len(),
            &state.state_root[..18],
        );

        state
    }
}
