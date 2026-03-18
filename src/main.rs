// C0DL3 zkSync Era Hyperchain Node
//
// A privacy-focused L3 rollup that settles to Ethereum via zkSync Era.
// Uses Bulletproofs for confidential transaction amounts.
//
// Settlement: C0DL3 blocks → zkSync Era (L2) → Ethereum L1
// Fuego connection: proof verification for HEAT/COLD assets + Elderfier validator qualification
// Validators: Elderado validators qualify via Elderfier STARK proof and/or 40B HEAT native stake

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use anyhow::{Result, anyhow};
use tracing::{info, error, debug, warn};

mod aa;
mod crypto;
mod fuego_l1_client;
// fuego_daemon removed — legacy merge-mining module no longer used
// mining (cn_upx2) removed — C0DL3 is a rollup, not a PoW chain
mod fuego_units;
mod privacy;
mod security;
mod validator;
mod economics;
mod tokens;
mod genesis;
mod proving;
mod settlement;
mod storage;
#[cfg(feature = "cli-ui")]
mod unified_cli;
#[cfg(feature = "cli-ui")]
mod cli_interface;
#[cfg(feature = "cli-ui")]
mod visual_cli;
#[cfg(feature = "cli-ui")]
mod enhanced_cli;
#[cfg(feature = "cli-ui")]
mod simple_visual_cli;

use fuego_l1_client::{FuegoL1Client, FuegoL1Config, FuegoNetwork};
use validator::{ValidatorRegistry, ValidatorRegistration, ValidatorInfo};
use privacy::shielded_pool::{self, compute_balance_commitment, ShieldedPool};
use economics::{ProverRegistry, BlockEconomics, EconomicsSummary, ProverReward, DEFAULT_PROOF_WINDOW_SECS};
use tokens::coldao::ColdaoManager;
use proving::{ProverConfig, BlockExecutionClaim, verify_execution_proof};

use serde_json::json;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};

use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use sha2::{Sha256, Digest};
use hex;
use libp2p::{
    identity, PeerId,
    Transport,
};
use futures::StreamExt;

// ──────────────────────────────────────────────
// zkSync-specific structures
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Batch {
    pub batch_number: u64,
    pub l1_tx_hash: String,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    pub state_root: String,
    pub priority_ops_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperchainConfig {
    pub chain_id: u64,
    pub name: String,
    pub rpc_url: String,
    pub bridge_address: String,
    pub validator_address: String,
    pub l1_contract_address: String,
}

// ──────────────────────────────────────────────
// P2P Network Behaviour
// ──────────────────────────────────────────────

#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(to_swarm = "C0DL3Event")]
pub struct C0DL3Behaviour {
    pub floodsub: libp2p::floodsub::Behaviour,
    pub kademlia: libp2p::kad::Behaviour<libp2p::kad::store::MemoryStore>,
}

#[derive(Debug)]
pub enum C0DL3Event {
    Floodsub(libp2p::floodsub::Event),
    Kademlia(libp2p::kad::Event),
}

impl From<libp2p::floodsub::Event> for C0DL3Event {
    fn from(event: libp2p::floodsub::Event) -> Self {
        C0DL3Event::Floodsub(event)
    }
}

impl From<libp2p::kad::Event> for C0DL3Event {
    fn from(event: libp2p::kad::Event) -> Self {
        C0DL3Event::Kademlia(event)
    }
}

// ──────────────────────────────────────────────
// Core data structures
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    /// AA UserOperations included in this block.
    #[serde(default)]
    pub user_operations: Vec<aa::types::UserOperation>,
    pub zk_proof: Option<ZkProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub parent_hash: String,
    pub timestamp: u64,
    pub merkle_root: String,
    pub state_root: String,
    pub validator: String,
    pub gas_used: u64,
    pub gas_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub value: u64,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub nonce: u64,
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    pub proof_type: String,
    pub proof_data: Vec<u8>,
    pub public_inputs: Vec<String>,
    pub verification_key: Vec<u8>,
}

// ──────────────────────────────────────────────
// Prover market structs (proposer/prover split)
// ──────────────────────────────────────────────

/// Proof submission from a prover node.
/// Any GPU holder can submit a proof for any pending block within the proof window.
/// First valid proof accepted wins the block reward.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofSubmission {
    pub block_height: u64,
    pub prover_address: String,
    /// Proof bytes (STARK-COMMITMENT-V2 for testnet; SP1/RISC Zero for mainnet).
    pub proof_bytes: Vec<u8>,
    /// Unix timestamp when this submission arrived at the sequencer.
    pub submitted_at: u64,
}

/// A block that has been successfully proven.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenBlock {
    pub block_height: u64,
    pub prover_address: String,
    /// HEAT subsidy minted to the prover (fwei).
    pub heat_subsidy: u64,
    /// Gas pool paid to the prover (fwei).
    pub gas_pool_paid: u64,
    /// Total HEAT reward (subsidy + gas pool).
    pub total_heat_reward: u64,
    /// Proposal tip credited to the block proposer (fwei).
    pub proposer_tip: u64,
    /// Unix timestamp of proof acceptance.
    pub proven_at: u64,
    /// SP1 proof bytes (stored for settlement submission to L2).
    #[serde(default)]
    pub proof_bytes: Vec<u8>,
}

/// Block proof status for API responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlockProofStatus {
    /// Block proposed, awaiting proof (within proof window).
    Pending,
    /// Proof accepted — block is hard-confirmed.
    Proven,
    /// Proof window expired with no valid proof.
    TimedOut,
}

// ──────────────────────────────────────────────
// Rollup state (real state transitions)
// ──────────────────────────────────────────────

pub use crate::aa::types::AccountState;

#[derive(Debug, Clone)]
pub struct RollupState {
    pub accounts: HashMap<String, AccountState>,
    pub state_root: String,
    pub block_height: u64,
    pub blocks: HashMap<u64, Block>,
    /// Shielded pool: tracks note commitments and nullifiers for Mode 2 (private P2P transfers)
    pub shielded_pool: ShieldedPool,
    /// Account-level execution nullifiers for Mode 1 (account/DeFi transactions).
    /// H("C0DL3:acct:" || sender_addr || nonce_le) — commitment-level binding per executed tx.
    pub account_nullifiers: HashSet<[u8; 32]>,

    // ── Prover market state ──────────────────────────────────────────────────
    /// Pending proof submissions per block (block_height → list of submissions).
    /// Any GPU holder may submit for any block within the proof window.
    pub pending_proofs: HashMap<u64, Vec<ProofSubmission>>,
    /// Proven blocks: block_height → winner + reward record.
    pub proven_blocks: HashMap<u64, ProvenBlock>,
    /// Per-block gas economics: block_height → accumulated gas routing data.
    pub block_economics_map: HashMap<u64, BlockEconomics>,
    /// Prover registry: lifetime proving stats per address + HEAT subsidy schedule.
    pub prover_registry: ProverRegistry,
    /// COLDAO manager: XFG time-lock interest deposits (governance token — NOT prover reward).
    pub coldao_manager: ColdaoManager,

    // ── Proving configuration ────────────────────────────────────────────────
    /// SP1 prover configuration (verification key + program ELF hash).
    pub prover_config: ProverConfig,
}

impl RollupState {
    pub fn new() -> Self {
        Self::with_prover_config(ProverConfig::new(vec![]))
    }

    pub fn with_prover_config(prover_config: ProverConfig) -> Self {
        Self {
            accounts: HashMap::new(),
            state_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            block_height: 0,
            blocks: HashMap::new(),
            shielded_pool: ShieldedPool::new(),
            account_nullifiers: HashSet::new(),
            pending_proofs: HashMap::new(),
            proven_blocks: HashMap::new(),
            block_economics_map: HashMap::new(),
            prover_registry: ProverRegistry::new(),
            coldao_manager: ColdaoManager::new(),
            prover_config,
        }
    }

    /// Accept a proof submission for a block.
    ///
    /// First valid submission for a block within the proof window is accepted.
    /// On acceptance: computes HEAT reward (subsidy + gas pool), records `ProvenBlock`,
    /// credits prover stats in the registry, and removes the pending proof slot.
    pub fn accept_proof(&mut self, submission: ProofSubmission) -> Result<ProvenBlock> {
        let block_height = submission.block_height;

        // Block must exist
        if !self.blocks.contains_key(&block_height) {
            return Err(anyhow!("Block {} does not exist", block_height));
        }

        // Must not already be proven
        if self.proven_blocks.contains_key(&block_height) {
            return Err(anyhow!("Block {} is already proven", block_height));
        }

        // Basic prover address validation (same threshold as tx addresses)
        if submission.prover_address.len() < 20 {
            return Err(anyhow!("Invalid prover address: must be ≥ 20 chars"));
        }

        // ── SP1 proof verification ──────────────────────────────────────────
        // Construct the expected claim from block data and verify the proof.
        // In mock mode: always passes. In SP1 mode: cryptographic verification.
        let block = self.blocks.get(&block_height)
            .ok_or_else(|| anyhow!("Block {} not found for proof verification", block_height))?;

        // Compute prev_state_root from parent block (or genesis zero root)
        let prev_state_root = if block_height > 1 {
            if let Some(parent) = self.blocks.get(&(block_height - 1)) {
                let decoded = hex::decode(parent.header.state_root.trim_start_matches("0x"))
                    .unwrap_or_else(|_| vec![0u8; 32]);
                let mut arr = [0u8; 32];
                let len = decoded.len().min(32);
                arr[..len].copy_from_slice(&decoded[..len]);
                arr
            } else {
                [0u8; 32]
            }
        } else {
            [0u8; 32]
        };

        let new_state_root = {
            let decoded = hex::decode(block.header.state_root.trim_start_matches("0x"))
                .unwrap_or_else(|_| vec![0u8; 32]);
            let mut arr = [0u8; 32];
            let len = decoded.len().min(32);
            arr[..len].copy_from_slice(&decoded[..len]);
            arr
        };

        let tx_merkle_root = {
            let decoded = hex::decode(block.header.merkle_root.trim_start_matches("0x"))
                .unwrap_or_else(|_| vec![0u8; 32]);
            let mut arr = [0u8; 32];
            let len = decoded.len().min(32);
            arr[..len].copy_from_slice(&decoded[..len]);
            arr
        };

        let claim = BlockExecutionClaim {
            block_height,
            prev_state_root,
            new_state_root,
            tx_merkle_root,
            tx_count: block.transactions.len() as u32,
            total_gas_used: block.header.gas_used,
            note_tree_root: self.shielded_pool.note_tree_root,
            nullifier_count: self.shielded_pool.nullifier_count() as u32,
        };

        // Verify proof against claim — SP1 cryptographic verification.
        // Requires `real-proofs` feature; without it, rejects the submission.
        let valid = verify_execution_proof(
            &submission.proof_bytes,
            &claim,
            &self.prover_config.vkey,
        )?;

        if !valid {
            return Err(anyhow!(
                "Invalid proof for block {} — verification failed",
                block_height
            ));
        }

        // Retrieve accumulated gas pool for this block (base fee portion → prover)
        let gas_pool = self.block_economics_map
            .get(&block_height)
            .map(|e| e.prover_gas_pool)
            .unwrap_or(0);

        let proposer_tip = self.block_economics_map
            .get(&block_height)
            .map(|e| e.proposer_tips)
            .unwrap_or(0);

        // Prover reward = gas fees only (HEAT has no inflation — proof-of-burn supply only)
        let reward = ProverReward::calculate(block_height, submission.prover_address.clone(), gas_pool);
        let total_heat_reward = reward.total_heat;

        let proven = ProvenBlock {
            block_height,
            prover_address: submission.prover_address.clone(),
            heat_subsidy: 0, // no inflation
            gas_pool_paid: gas_pool,
            total_heat_reward,
            proposer_tip,
            proven_at: submission.submitted_at,
            proof_bytes: submission.proof_bytes.clone(),
        };

        // Commit the proven record
        self.proven_blocks.insert(block_height, proven.clone());

        // Update lifetime prover stats in the registry
        self.prover_registry.record_proof(
            &submission.prover_address,
            block_height,
            total_heat_reward,
        );

        // Clear pending proof slot
        self.pending_proofs.remove(&block_height);

        info!(
            "Block {} proven by {} — HEAT reward: {} fwei (gas fees only, no inflation)",
            block_height,
            submission.prover_address,
            total_heat_reward,
        );

        Ok(proven)
    }

    /// Returns the proof status for a given block.
    ///
    /// `Proven`  — proof accepted, block is hard-confirmed.
    /// `Pending` — within the proof window, awaiting proof.
    /// `TimedOut`— proof window expired with no valid proof submitted.
    pub fn proof_status(&self, block_height: u64, proof_window_secs: u64) -> BlockProofStatus {
        if self.proven_blocks.contains_key(&block_height) {
            return BlockProofStatus::Proven;
        }
        if let Some(block) = self.blocks.get(&block_height) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now <= block.header.timestamp.saturating_add(proof_window_secs) {
                return BlockProofStatus::Pending;
            }
        }
        BlockProofStatus::TimedOut
    }

    /// Compute state root from all account states.
    /// Uses Pedersen balance commitments (not plaintext balances) so the
    /// state root is opaque to external observers.
    pub fn compute_state_root(&mut self) {
        let mut hasher = Sha256::new();
        let mut sorted_accounts: Vec<_> = self.accounts.iter().collect();
        sorted_accounts.sort_by_key(|(k, _)| k.clone());

        for (address, state) in &sorted_accounts {
            hasher.update(address.as_bytes());
            hasher.update(&state.balance_commitment); // commitment, not plaintext
            hasher.update(state.nonce.to_le_bytes());
        }

        let result = hasher.finalize();
        self.state_root = format!("0x{}", hex::encode(result));
    }
}

// ──────────────────────────────────────────────
// Node state (for monitoring/stats)
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub current_height: u64,
    pub latest_block_hash: String,
    pub connected_peers: u32,
    pub pending_transactions: u32,
    pub blocks_produced: u64,
    pub uptime_seconds: u64,
}

// ──────────────────────────────────────────────
// Configuration structures
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub rpc: RPCConfig,
    pub sequencer: SequencerConfig,
    pub zksync: ZkSyncConfig,
    pub fuego: FuegoL1Config,
    pub settlement: SettlementConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub data_dir: String,
    pub p2p_port: u16,
    pub listen_addr: String,
    pub bootstrap_peers: Vec<String>,
    pub max_peers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPCConfig {
    pub port: u16,
    pub host: String,
    pub cors_origins: Vec<String>,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencerConfig {
    pub enabled: bool,
    pub block_time_secs: u64,
    pub max_txs_per_block: u32,
    pub batch_size: u32,       // Blocks per L1 batch
    pub batch_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkSyncConfig {
    pub hyperchain_id: u64,
    pub validator_address: String,
    pub bridge_address: String,
    pub l1_contract_address: String,
    pub l1_batch_commitment: bool,
    pub zk_proof_generation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementConfig {
    /// zkSync Era RPC URL (settlement layer)
    pub era_rpc_url: String,
    /// Ethereum L1 RPC URL (finality monitoring)
    pub eth_l1_rpc_url: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            p2p_port: 30333,
            listen_addr: "0.0.0.0".to_string(),
            bootstrap_peers: vec![
                "/ip4/127.0.0.1/tcp/30334/p2p/QmBootstrap1".to_string(),
            ],
            max_peers: 50,
        }
    }
}

impl Default for RPCConfig {
    fn default() -> Self {
        Self {
            port: 9944,
            host: "127.0.0.1".to_string(),
            cors_origins: vec!["*".to_string()],
            max_connections: 100,
        }
    }
}

impl Default for SequencerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_time_secs: 2,     // 2-second blocks for L3
            max_txs_per_block: 100,
            batch_size: 10,          // 10 blocks per L1 batch
            batch_timeout_secs: 300, // 5 minutes max
        }
    }
}

// ──────────────────────────────────────────────
// Main node implementation
// ──────────────────────────────────────────────

#[derive(Clone)]
pub struct C0DL3ZkSyncNode {
    config: NodeConfig,
    /// Fuego L1 client — for proof verification + validator qualification ONLY, NOT settlement
    fuego_client: Arc<FuegoL1Client>,
    /// Validator registry (dual-path: Elderfier STARK + HEAT native)
    validator_registry: Arc<ValidatorRegistry>,
    /// L3 rollup state (accounts, blocks, state root)
    rollup_state: Arc<Mutex<RollupState>>,
    /// Persistent state database (sled) — write-through from RollupState
    state_db: Arc<storage::StateDb>,
    /// Pending transaction pool
    pending_transactions: Arc<Mutex<HashMap<String, Transaction>>>,
    /// Pending AA UserOperation pool (drained into blocks by the sequencer)
    pending_aa_operations: Arc<Mutex<Vec<aa::types::UserOperation>>>,
    /// L1 batches submitted to zkSync Era
    l1_batches: Arc<Mutex<HashMap<u64, L1Batch>>>,
    /// Settlement manager — batches proven blocks and submits to L2
    settlement_manager: Arc<Mutex<settlement::SettlementManager>>,
    /// Node monitoring state
    node_state: Arc<Mutex<NodeState>>,
    /// Hyperchain configuration
    hyperchain_config: HyperchainConfig,
    /// Genesis configuration (chain identity, allocations)
    genesis_config: genesis::GenesisConfig,
    /// Privacy manager (Bulletproofs CT, address encryption)
    privacy_manager: Option<privacy::UserPrivacyManager>,
    /// Node start time
    start_time: Instant,
}

impl C0DL3ZkSyncNode {
    pub fn new(config: NodeConfig, prover_config: ProverConfig) -> Self {
        let hyperchain_config = HyperchainConfig {
            chain_id: config.zksync.hyperchain_id,
            name: "C0DL3-Hyperchain".to_string(),
            rpc_url: format!("http://{}:{}", config.rpc.host, config.rpc.port),
            bridge_address: config.zksync.bridge_address.clone(),
            validator_address: config.zksync.validator_address.clone(),
            l1_contract_address: config.zksync.l1_contract_address.clone(),
        };

        // Create Fuego L1 client (for proof verification, NOT settlement)
        let fuego_client = Arc::new(FuegoL1Client::new(&config.fuego));

        // Create validator registry (testnet mode if Fuego is testnet)
        let testnet_mode = config.fuego.network == FuegoNetwork::Testnet;
        let validator_registry = Arc::new(ValidatorRegistry::new(fuego_client.clone(), testnet_mode));

        // Initialize privacy manager
        let privacy_manager = match privacy::UserPrivacyManager::new() {
            Ok(manager) => {
                info!("Privacy manager initialized (Bulletproofs CT enabled)");
                Some(manager)
            },
            Err(e) => {
                error!("Failed to initialize privacy manager: {}", e);
                None
            }
        };

        // Open persistent state database
        let db_path = format!("{}/state_db", config.network.data_dir);
        let state_db = Arc::new(
            storage::StateDb::open(&db_path)
                .expect("Failed to open state database")
        );

        // Load persisted state or initialize from genesis
        let genesis_path = format!("{}/genesis.json", config.network.data_dir);
        let genesis_config = genesis::GenesisConfig::load_or_default(&genesis_path);

        let rollup_state = match state_db.load_rollup_state() {
            Ok(Some(state)) => {
                info!(
                    "Resumed from persisted state (height: {}, accounts: {})",
                    state.block_height, state.accounts.len()
                );
                state
            }
            Ok(None) => {
                info!("No persisted state — applying genesis (chain_id: {})", genesis_config.chain_id);
                let state = genesis_config.apply(prover_config);
                // Persist genesis accounts
                for (addr, acct) in &state.accounts {
                    if let Err(e) = state_db.put_account(addr, acct) {
                        error!("Failed to persist genesis account {}: {}", addr, e);
                    }
                }
                state
            }
            Err(e) => {
                error!("Failed to load persisted state: {} — applying genesis", e);
                genesis_config.apply(prover_config)
            }
        };

        // Reconstruct node_state from loaded rollup_state
        let loaded_height = rollup_state.block_height;
        let loaded_block_hash = rollup_state.blocks.get(&loaded_height)
            .map(|b| {
                // Recompute block hash
                compute_block_hash(b)
            })
            .unwrap_or_else(|| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string());

        let era_rpc_url = config.settlement.era_rpc_url.clone();

        Self {
            config,
            fuego_client,
            validator_registry,
            rollup_state: Arc::new(Mutex::new(rollup_state)),
            state_db,
            pending_transactions: Arc::new(Mutex::new(HashMap::new())),
            pending_aa_operations: Arc::new(Mutex::new(Vec::new())),
            l1_batches: Arc::new(Mutex::new(HashMap::new())),
            settlement_manager: Arc::new(Mutex::new(settlement::SettlementManager::new(
                settlement::SettlementModuleConfig {
                    era_rpc_url,
                    ..Default::default()
                },
            ))),
            node_state: Arc::new(Mutex::new(NodeState {
                current_height: loaded_height,
                latest_block_hash: loaded_block_hash,
                connected_peers: 0,
                pending_transactions: 0,
                blocks_produced: loaded_height,
                uptime_seconds: 0,
            })),
            hyperchain_config,
            genesis_config,
            privacy_manager,
            start_time: Instant::now(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting C0DL3 zkSync Era Hyperchain Node...");
        info!("Settlement: C0DL3 -> zkSync Era -> Ethereum L1");
        info!("Fuego connection: proof verification + validator qualification");

        self.init_logging()?;

        // Check Fuego L1 connection (for proof verification)
        self.check_fuego_connection().await;

        // Start subsystems
        self.start_rpc_server().await?;
        self.start_sequencer().await?;
        self.start_l1_batch_submitter().await?;
        self.start_fuego_slash_monitor().await?;

        // Start P2P network (blocks the task)
        self.init_p2p_network().await?;

        info!("C0DL3 zkSync Era Hyperchain Node started");
        Ok(())
    }

    fn init_logging(&self) -> Result<()> {
        // tracing_subscriber::fmt::init() should only be called once
        // It's typically called in main() before node.start()
        Ok(())
    }

    /// Check Fuego L1 connection (proof verification, not settlement)
    async fn check_fuego_connection(&self) {
        info!("Checking Fuego L1 connection (proof verification)...");
        match self.fuego_client.check_connection().await {
            Ok(true) => info!("Fuego L1 connection established"),
            Ok(false) => warn!("Fuego L1 not reachable — validator registration via Elderfier STARK will be unavailable"),
            Err(e) => warn!("Fuego L1 connection check failed: {}", e),
        }
    }

    // ──────────────────────────────────────────
    // Sequencer (block production)
    // ──────────────────────────────────────────

    async fn start_sequencer(&self) -> Result<()> {
        if !self.config.sequencer.enabled {
            info!("Sequencer disabled");
            return Ok(());
        }

        info!("Starting sequencer (block time: {}s, max txs: {})",
            self.config.sequencer.block_time_secs, self.config.sequencer.max_txs_per_block);

        let config = self.config.clone();
        let rollup_state = self.rollup_state.clone();
        let pending_txs = self.pending_transactions.clone();
        let pending_aa_ops = self.pending_aa_operations.clone();
        let node_state = self.node_state.clone();
        let validator_registry = self.validator_registry.clone();
        let l1_batches = self.l1_batches.clone();
        let fuego_client = self.fuego_client.clone();
        let state_db = self.state_db.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.sequencer.block_time_secs));
            let mut blocks_since_batch: u32 = 0;
            let mut batch_number: u64 = 0;

            loop {
                interval.tick().await;

                // Get current block height
                let next_height = {
                    let state = rollup_state.lock().unwrap();
                    state.block_height + 1
                };

                // Select block proposer
                let proposer = validator_registry.select_block_proposer(next_height);
                let proposer_id = match &proposer {
                    Some(v) => v.validator_id.clone(),
                    None => {
                        // No validators registered — produce block as self (testnet only)
                        debug!("No validators registered, producing block as node operator");
                        config.zksync.validator_address.clone()
                    }
                };

                // Drain pending transactions
                let txs: Vec<Transaction> = {
                    let mut pending = pending_txs.lock().unwrap();
                    let max = config.sequencer.max_txs_per_block as usize;
                    let drain_keys: Vec<String> = pending.keys()
                        .take(max)
                        .cloned()
                        .collect();
                    drain_keys.iter()
                        .filter_map(|k| pending.remove(k))
                        .collect()
                };

                // Drain pending AA operations
                let aa_ops: Vec<aa::types::UserOperation> = {
                    let mut pending_aa = pending_aa_ops.lock().unwrap();
                    std::mem::take(&mut *pending_aa)
                };

                // Execute transactions and update state
                let (state_root, gas_used, merkle_root, confirmed_txs) = {
                    let mut state = rollup_state.lock().unwrap();

                    // Create per-block economics tracker: accumulates gas fees for the prover
                    let mut block_econ = BlockEconomics::new(proposer_id.clone());
                    let mut gas_used: u64 = 0;
                    let mut confirmed = Vec::new();

                    for mut tx in txs {
                        match execute_transaction(&mut state, &tx) {
                            Ok(gas) => {
                                // Route gas fees:
                                // testnet model — all gas price is treated as base fee → prover pool.
                                // base_fee_per_gas = tx.gas_price means 100% goes to prover_gas_pool.
                                block_econ.record_tx_gas(gas, tx.gas_price, tx.gas_price);
                                gas_used += gas;
                                tx.status = TransactionStatus::Confirmed;
                                confirmed.push(tx);
                            }
                            Err(e) => {
                                debug!("Transaction {} failed: {}", tx.hash, e);
                                tx.status = TransactionStatus::Failed;
                            }
                        }
                    }

                    // Store block economics + register block as pending-proof
                    state.block_economics_map.insert(next_height, block_econ);
                    state.pending_proofs.entry(next_height).or_default();

                    // Compute new state root
                    state.compute_state_root();

                    // Compute merkle root of transactions
                    let merkle_root = compute_merkle_root(&confirmed);

                    (state.state_root.clone(), gas_used, merkle_root, confirmed)
                };

                // Get parent hash
                let parent_hash = {
                    let ns = node_state.lock().unwrap();
                    ns.latest_block_hash.clone()
                };

                // Create block
                let block = Block {
                    header: BlockHeader {
                        height: next_height,
                        parent_hash,
                        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        merkle_root,
                        state_root: state_root.clone(),
                        validator: proposer_id.clone(),
                        gas_used,
                        gas_limit: 30_000_000,
                    },
                    transactions: confirmed_txs,
                    user_operations: aa_ops,
                    zk_proof: None,
                };

                // Compute block hash
                let block_hash = compute_block_hash(&block);

                // Store block (in-memory + persistent)
                {
                    let mut state = rollup_state.lock().unwrap();
                    state.blocks.insert(next_height, block.clone());
                    state.block_height = next_height;

                    // Write-through to sled: block + state root + touched accounts
                    if let Err(e) = state_db.put_block(next_height, &block, &state.state_root) {
                        error!("Failed to persist block {}: {}", next_height, e);
                    }
                    // Persist all accounts that were touched by this block's transactions
                    let touched: Vec<(String, AccountState)> = block.transactions.iter()
                        .flat_map(|tx| vec![tx.from.clone(), tx.to.clone()])
                        .filter_map(|addr| {
                            state.accounts.get(&addr).map(|a| (addr, a.clone()))
                        })
                        .collect();
                    if !touched.is_empty() {
                        if let Err(e) = state_db.put_accounts(&touched) {
                            error!("Failed to persist accounts for block {}: {}", next_height, e);
                        }
                    }
                    // Persist block economics
                    if let Some(econ) = state.block_economics_map.get(&next_height) {
                        if let Err(e) = state_db.put_block_economics(next_height, econ) {
                            error!("Failed to persist block economics {}: {}", next_height, e);
                        }
                    }
                }

                // Update node state
                {
                    let mut ns = node_state.lock().unwrap();
                    ns.current_height = next_height;
                    ns.latest_block_hash = block_hash;
                    ns.blocks_produced += 1;
                    ns.pending_transactions = pending_txs.lock().unwrap().len() as u32;
                }

                // Record block for validator
                if let Some(ref p) = proposer {
                    validator_registry.record_block_produced(&p.validator_id, next_height);
                }

                if !block.transactions.is_empty() || next_height % 100 == 0 {
                    info!("Block {} produced (txs: {}, gas: {}, validator: {})",
                        next_height, block.transactions.len(), gas_used, proposer_id);
                }

                // Accumulate into L1 batch
                blocks_since_batch += 1;
                if blocks_since_batch >= config.sequencer.batch_size {
                    batch_number += 1;
                    let batch = L1Batch {
                        batch_number,
                        l1_tx_hash: String::new(),
                        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        transactions: vec![], // Batches reference blocks, not individual txs
                        state_root,
                        priority_ops_hash: format!("0x{:064x}", batch_number),
                    };

                    l1_batches.lock().unwrap().insert(batch_number, batch.clone());
                    info!("L1 batch {} accumulated (state root: {})", batch_number, batch.state_root);
                    blocks_since_batch = 0;
                }
            }
        });

        info!("Sequencer started");
        Ok(())
    }

    // ──────────────────────────────────────────
    // L1 Batch submission (to zkSync Era)
    // ──────────────────────────────────────────

    async fn start_l1_batch_submitter(&self) -> Result<()> {
        info!("Starting L1 batch submitter (settlement: zkSync Era)");
        info!("Era RPC: {}", self.config.settlement.era_rpc_url);

        let l1_batches = self.l1_batches.clone();
        let rollup_state = self.rollup_state.clone();
        let settlement_mgr = self.settlement_manager.clone();
        let batch_size = self.config.sequencer.batch_size as u64;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            let mut last_batched_height: u64 = 0;

            loop {
                interval.tick().await;

                // Find proven blocks that haven't been batched for settlement yet
                let rollup = rollup_state.lock().unwrap();
                let mut batchable: Vec<u64> = rollup.proven_blocks.keys()
                    .filter(|&&h| h > last_batched_height)
                    .copied()
                    .collect();
                batchable.sort();
                drop(rollup);

                if batchable.is_empty() {
                    continue;
                }

                // Take up to batch_size proven blocks
                let batch_heights: Vec<u64> = batchable.into_iter()
                    .take(batch_size as usize)
                    .collect();

                if batch_heights.is_empty() {
                    continue;
                }

                // Get the proof from the last block in the batch
                let rollup = rollup_state.lock().unwrap();
                let last_height = *batch_heights.last().unwrap();
                let first_height = *batch_heights.first().unwrap();

                // Compute prev_state_root from block before batch start
                let prev_state_root = if first_height > 1 {
                    rollup.blocks.get(&(first_height - 1))
                        .map(|b| {
                            let decoded = hex::decode(b.header.state_root.trim_start_matches("0x"))
                                .unwrap_or_else(|_| vec![0u8; 32]);
                            let mut arr = [0u8; 32];
                            let len = decoded.len().min(32);
                            arr[..len].copy_from_slice(&decoded[..len]);
                            arr
                        })
                        .unwrap_or([0u8; 32])
                } else {
                    [0u8; 32]
                };

                // Get new_state_root from last block in batch
                let new_state_root = rollup.blocks.get(&last_height)
                    .map(|b| {
                        let decoded = hex::decode(b.header.state_root.trim_start_matches("0x"))
                            .unwrap_or_else(|_| vec![0u8; 32]);
                        let mut arr = [0u8; 32];
                        let len = decoded.len().min(32);
                        arr[..len].copy_from_slice(&decoded[..len]);
                        arr
                    })
                    .unwrap_or([0u8; 32]);

                // Get proof bytes from the proven block record
                let proof_bytes = rollup.proven_blocks.get(&last_height)
                    .map(|pb| pb.proof_bytes.clone())
                    .unwrap_or_default();

                // Build BlockExecutionClaim public values
                let public_values = if let Some(block) = rollup.blocks.get(&last_height) {
                    let tx_merkle_root = {
                        let decoded = hex::decode(block.header.merkle_root.trim_start_matches("0x"))
                            .unwrap_or_else(|_| vec![0u8; 32]);
                        let mut arr = [0u8; 32];
                        let len = decoded.len().min(32);
                        arr[..len].copy_from_slice(&decoded[..len]);
                        arr
                    };

                    let claim = proving::BlockExecutionClaim {
                        block_height: last_height,
                        prev_state_root,
                        new_state_root,
                        tx_merkle_root,
                        tx_count: block.transactions.len() as u32,
                        total_gas_used: block.header.gas_used,
                        note_tree_root: rollup.shielded_pool.note_tree_root,
                        nullifier_count: rollup.shielded_pool.nullifier_count() as u32,
                    };
                    claim.encode()
                } else {
                    vec![]
                };
                drop(rollup);

                // Create settlement batch
                let mut sm = settlement_mgr.lock().unwrap();
                match sm.create_batch(
                    batch_heights.clone(),
                    proof_bytes,
                    public_values,
                    prev_state_root,
                    new_state_root,
                ) {
                    Ok(batch_id) => {
                        info!(
                            "Settlement batch {} created: blocks {}-{}",
                            batch_id, first_height, last_height,
                        );
                        last_batched_height = last_height;
                    }
                    Err(e) => {
                        warn!("Failed to create settlement batch: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    // ──────────────────────────────────────────
    // Fuego slash monitor (cascading slash)
    // ──────────────────────────────────────────

    async fn start_fuego_slash_monitor(&self) -> Result<()> {
        let fuego_client = self.fuego_client.clone();
        let validator_registry = self.validator_registry.clone();
        let poll_interval = self.config.fuego.poll_interval_secs;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(poll_interval));

            loop {
                interval.tick().await;

                // Check if Fuego is connected
                if !fuego_client.is_connected() {
                    continue;
                }

                // Get all validators with Elderfier addresses
                let validators = validator_registry.get_active_validators();
                for v in &validators {
                    if let Some(ref elderfier_addr) = v.elderfier_address {
                        // Check if Elderfier is still active on Fuego
                        match fuego_client.check_elderfier_stake(elderfier_addr).await {
                            Ok(info) => {
                                if !info.is_qualified {
                                    warn!("Elderfier {} no longer qualified on Fuego — cascading slash",
                                        elderfier_addr);
                                    validator_registry.process_l1_slash(elderfier_addr);
                                }
                            }
                            Err(e) => {
                                debug!("Cannot check Elderfier {}: {}", elderfier_addr, e);
                            }
                        }
                    }
                }
            }
        });

        info!("Fuego slash monitor started (poll interval: {}s)", poll_interval);
        Ok(())
    }

    // ──────────────────────────────────────────
    // Transaction handling
    // ──────────────────────────────────────────

    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        // Basic validation
        if tx.gas_limit > 30_000_000 {
            return Err(anyhow!("Gas limit exceeds block gas limit"));
        }
        if tx.gas_price == 0 {
            return Err(anyhow!("Gas price must be > 0"));
        }

        let tx_hash = tx.hash.clone();
        {
            let mut pending = self.pending_transactions.lock().unwrap();
            pending.insert(tx_hash.clone(), tx);
        }
        {
            let mut ns = self.node_state.lock().unwrap();
            ns.pending_transactions = self.pending_transactions.lock().unwrap().len() as u32;
        }

        debug!("Added transaction: {}", tx_hash);
        Ok(())
    }

    // ──────────────────────────────────────────
    // ZK Proof generation
    // ──────────────────────────────────────────

    pub async fn generate_zk_proof(&self, block: &Block) -> Result<ZkProof> {
        // STARK-COMMITMENT-V2: Block commitment + shielded state proof.
        //
        // Binds together:
        // 1. TX commitment tree root (SHA-256 Merkle over tx commitment hashes)
        // 2. Committed state root (Merkle over Pedersen balance commitments)
        // 3. Shielded pool note tree root
        // 4. Nullifier count consumed in this block
        //
        // SCOPE: Commitment integrity + state privacy. NOT a full execution proof.
        // Full execution proofs require Boojum/Airbender (zkSync Era mainnet path).
        use crate::privacy::block_commitment_proof::{
            generate_block_commitment_proof, BlockProofPublicInputs,
        };

        // Use each transaction's hash as its commitment input.
        let tx_commitments: Vec<Vec<u8>> = block.transactions
            .iter()
            .map(|tx| tx.hash.as_bytes().to_vec())
            .collect();

        let commitment_proof = generate_block_commitment_proof(
            block.header.height,
            &tx_commitments,
        )?;

        // Read shielded pool + account nullifier state
        let rollup_state = self.rollup_state.lock().unwrap();
        let note_tree_root = rollup_state.shielded_pool.note_tree_root;
        let note_nullifier_count = rollup_state.shielded_pool.nullifier_count();
        let account_nullifier_count = rollup_state.account_nullifiers.len() as u64;
        drop(rollup_state);

        let proof = ZkProof {
            proof_type: "STARK-COMMITMENT-V2".to_string(),
            proof_data: commitment_proof.to_bytes()?,
            public_inputs: vec![
                block.header.height.to_string(),
                block.header.merkle_root.clone(),
                block.header.state_root.clone(),                      // Merkle(committed balances)
                hex::encode(commitment_proof.commitment_tree_root),    // tx commitment tree
                hex::encode(note_tree_root),                           // Mode 2: note tree root
                note_nullifier_count.to_string(),                      // Mode 2: note nullifiers
                account_nullifier_count.to_string(),                   // Mode 1: account nullifiers
            ],
            verification_key: Vec::new(),
        };
        Ok(proof)
    }

    // ──────────────────────────────────────────
    // L1 Batch methods
    // ──────────────────────────────────────────

    pub async fn submit_l1_batch(&self, batch: L1Batch) -> Result<()> {
        info!("Submitting L1 batch: {}", batch.batch_number);
        self.l1_batches.lock().unwrap().insert(batch.batch_number, batch);
        Ok(())
    }

    pub async fn get_l1_batch(&self, batch_number: u64) -> Option<L1Batch> {
        self.l1_batches.lock().unwrap().get(&batch_number).cloned()
    }

    // ──────────────────────────────────────────
    // Stats
    // ──────────────────────────────────────────

    pub fn get_node_state(&self) -> NodeState {
        let mut ns = self.node_state.lock().unwrap().clone();
        ns.uptime_seconds = self.start_time.elapsed().as_secs();
        ns
    }

    pub fn get_network_stats(&self) -> Result<serde_json::Value> {
        let ns = self.get_node_state();
        let batches = self.l1_batches.lock().unwrap();
        let validator_status = self.validator_registry.get_status_json();

        let stats = json!({
            "network": {
                "connected_peers": ns.connected_peers,
                "uptime_seconds": ns.uptime_seconds,
            },
            "chain": {
                "current_height": ns.current_height,
                "blocks_produced": ns.blocks_produced,
                "latest_block_hash": ns.latest_block_hash,
                "pending_transactions": ns.pending_transactions,
            },
            "settlement": {
                "layer": "zkSync Era -> Ethereum L1",
                "era_rpc_url": self.config.settlement.era_rpc_url,
                "l1_batches_submitted": batches.len(),
            },
            "fuego": {
                "role": "proof verification + validator qualification",
                "connected": self.fuego_client.is_connected(),
                "rpc_url": self.fuego_client.rpc_url(),
            },
            "validators": validator_status,
            "hyperchain": {
                "chain_id": self.hyperchain_config.chain_id,
                "name": self.hyperchain_config.name,
            }
        });

        Ok(stats)
    }

    // ──────────────────────────────────────────
    // P2P Network
    // ──────────────────────────────────────────

    async fn init_p2p_network(&mut self) -> Result<()> {
        info!("Initializing P2P network...");

        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer ID: {}", local_peer_id);

        let mut floodsub = libp2p::floodsub::Behaviour::new(local_peer_id);
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-blocks"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-transactions"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-privacy-proofs"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-validator-proofs"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-state-roots"));

        let store = libp2p::kad::store::MemoryStore::new(local_peer_id);
        let kademlia = libp2p::kad::Behaviour::new(local_peer_id, store);

        let behaviour = C0DL3Behaviour { floodsub, kademlia };

        let transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(libp2p::noise::Config::new(&local_key)?)
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_other_transport(|_| Ok(transport))?
            .with_behaviour(|_| Ok(behaviour))?
            .build();

        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", self.config.network.p2p_port);
        swarm.listen_on(listen_addr.parse()?)?;

        info!("P2P listening on port {}", self.config.network.p2p_port);

        self.start_p2p_event_loop(swarm).await?;
        Ok(())
    }

    async fn start_p2p_event_loop(&mut self, mut swarm: libp2p::Swarm<C0DL3Behaviour>) -> Result<()> {
        // Only bootstrap if we have known peers; otherwise it's a no-op on first start
        let _ = swarm.behaviour_mut().kademlia.bootstrap();

        loop {
            match swarm.select_next_some().await {
                libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                    info!("Listening on: {}", address);
                }
                libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!("Connected to peer: {}", peer_id);
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
                }
                libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    info!("Disconnected from peer: {}", peer_id);
                }
                libp2p::swarm::SwarmEvent::Behaviour(event) => {
                    match event {
                        C0DL3Event::Floodsub(libp2p::floodsub::Event::Message(message)) => {
                            let data = String::from_utf8_lossy(&message.data);
                            for topic in &message.topics {
                                let topic_str = format!("{:?}", topic);
                                debug!("P2P message on {}: {} bytes", topic_str, message.data.len());
                            }
                        }
                        C0DL3Event::Kademlia(libp2p::kad::Event::OutboundQueryProgressed { result, .. }) => {
                            match result {
                                libp2p::kad::QueryResult::Bootstrap(Ok(ok)) => {
                                    if ok.num_remaining == 0 {
                                        info!("DHT bootstrap completed");
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                libp2p::swarm::SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    warn!("Connection error to {}: {}",
                        peer_id.map_or("unknown".to_string(), |p| p.to_string()), error);
                }
                _ => {}
            }
        }
    }

    // ──────────────────────────────────────────
    // RPC Server
    // ──────────────────────────────────────────

    async fn start_rpc_server(&self) -> Result<()> {
        info!("Starting RPC server on {}:{}", self.config.rpc.host, self.config.rpc.port);

        let app_state = AppState {
            node: Arc::new(Mutex::new(self.clone())),
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            // Core
            .route("/", get(root))
            .route("/health", get(health))
            .route("/stats", get(get_stats))

            // Blocks & transactions
            .route("/blocks/{height}", get(get_block))
            .route("/transactions/{hash}", get(get_transaction))
            .route("/submit_transaction", post(submit_transaction))

            // Hyperchain / settlement
            .route("/hyperchain/info", get(get_hyperchain_info))
            .route("/hyperchain/batches", get(get_l1_batches))
            .route("/settlement/status", get(get_settlement_status))

            // Validators (Elderados)
            .route("/validators", get(get_validators))
            .route("/validator/register", post(register_validator))
            .route("/validator/{id}", get(get_validator))

            // Fuego proof verification (NOT settlement)
            .route("/fuego/status", get(get_fuego_status))
            .route("/fuego/commitment/{hash}", get(get_fuego_commitment))

            // Privacy
            .route("/privacy/status", get(get_privacy_status))
            .route("/privacy/create_transaction", post(create_private_transaction))
            .route("/privacy/submit_transaction", post(submit_private_transaction))
            .route("/privacy/get_transaction/{hash}", get(get_private_transaction))
            .route("/privacy/verify_transaction", post(verify_private_transaction))

            // Shielded pool (client-side partial proving — sequencer never sees plaintext)
            .route("/shield", post(accept_shield))
            .route("/unshield", post(accept_unshield))
            .route("/shield/note_tree", get(get_note_tree_info))

            // Account Abstraction
            .route("/aa/create_wallet", post(create_aa_wallet))
            .route("/aa/send", post(aa_send))

            // Prover market
            .route("/proof/submit", post(submit_proof))
            .route("/proof/pending", get(get_pending_proofs))
            .route("/proof/block_input/{height}", get(get_block_input))
            .route("/provers", get(get_provers))
            .route("/economics", get(get_economics))

            // Settlement (L3 → zkSync Era)
            .route("/settlement", get(get_settlement_status))
            .route("/settlement/batches", get(get_settlement_batches))

            // Storage
            .route("/storage/status", get(get_storage_status))

            // Faucet (testnet only)
            .route("/faucet", post(faucet_drip))

            // Ethereum JSON-RPC compatibility (POST /)
            .route("/rpc", post(eth_json_rpc))

            .layer(ServiceBuilder::new().layer(cors))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(
            format!("{}:{}", self.config.rpc.host, self.config.rpc.port)
        ).await?;

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("RPC server error: {}", e);
            }
        });

        info!("RPC server started");
        Ok(())
    }

    // ──────────────────────────────────────────
    // Privacy methods (kept from original)
    // ──────────────────────────────────────────

    pub fn create_private_transaction_rpc(
        &mut self, sender: &str, recipient: &str, amount: u64, sender_balance: u64,
    ) -> Result<privacy::PrivateTransaction> {
        if let Some(ref mut pm) = self.privacy_manager {
            let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            pm.create_private_transaction(sender, recipient, amount, ts, sender_balance, None)
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }

    pub fn process_private_transaction(&self, tx: privacy::PrivateTransaction) -> Result<()> {
        if let Some(ref pm) = self.privacy_manager {
            if !pm.verify_private_transaction(&tx)? {
                return Err(anyhow!("Invalid private transaction"));
            }

            // If a stealth output is present, use the one-time address as the
            // account key and encode ephemeral_pubkey + view_tag in tx.data
            // so recipients can scan for their payments.
            let (to_addr, tx_data) = if let Some(ref stealth) = tx.stealth_output {
                let addr = privacy::stealth_address::one_time_address_to_hex(&stealth.one_time_address);
                let data = privacy::stealth_address::encode_stealth_data(stealth);
                (addr, data)
            } else {
                ("encrypted".to_string(), vec![])
            };

            let mut pending = self.pending_transactions.lock().unwrap();
            pending.insert(tx.hash.clone(), Transaction {
                hash: tx.hash.clone(),
                from: "encrypted".to_string(), // sender identity hidden
                to: to_addr,
                value: 0, // Amount hidden in Bulletproof commitment
                gas_price: 0,
                gas_limit: 0,
                nonce: 0,
                data: tx_data,
                signature: vec![],
                status: TransactionStatus::Pending,
            });
            Ok(())
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }

    pub fn get_private_transaction(&self, hash: &str) -> Result<Option<privacy::PrivateTransaction>> {
        if let Some(ref pm) = self.privacy_manager {
            pm.get_private_transaction(hash)
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }

    pub fn get_privacy_status(&self) -> serde_json::Value {
        json!({
            "enabled": self.privacy_manager.is_some(),
            "privacy_level": if self.privacy_manager.is_some() { 100 } else { 0 },
            "features": {
                "bulletproof_amounts": self.privacy_manager.is_some(),
                "address_encryption": self.privacy_manager.is_some(),
                "timing_privacy": self.privacy_manager.is_some(),
            },
        })
    }
}

// ──────────────────────────────────────────────
// Pure functions (transaction execution, hashing)
// ──────────────────────────────────────────────

/// Execute a transaction against rollup state, returning gas used.
/// After balance updates, recomputes Pedersen commitments for affected accounts.
fn execute_transaction(state: &mut RollupState, tx: &Transaction) -> Result<u64> {
    let base_gas: u64 = 21_000;
    let data_gas: u64 = (tx.data.len() as u64) * 68;
    let total_gas = base_gas + data_gas;
    let gas_cost = tx.gas_price * total_gas;

    // Get or create sender account
    let sender_addr = tx.from.clone();
    let sender = state.accounts
        .entry(sender_addr.clone())
        .or_insert_with(|| AccountState {
            balance_commitment: compute_balance_commitment(&sender_addr, 0, 0),
            balance: 0,
            nonce: 0,
            wallet_type: aa::types::WalletType::LegacyEOA,
            owner_pubkey: None,
        });

    // Verify nonce
    if tx.nonce != sender.nonce {
        return Err(anyhow!("Invalid nonce: expected {}, got {}", sender.nonce, tx.nonce));
    }

    // Verify sufficient balance
    let total_cost = tx.value + gas_cost;
    if sender.balance < total_cost {
        return Err(anyhow!("Insufficient balance: {} < {}", sender.balance, total_cost));
    }

    // Deduct from sender and recompute commitment
    sender.balance -= total_cost;
    sender.nonce += 1;
    sender.balance_commitment = compute_balance_commitment(&sender_addr, sender.balance, sender.nonce);

    // Credit recipient and recompute commitment
    let recipient_addr = tx.to.clone();
    let recipient = state.accounts
        .entry(recipient_addr.clone())
        .or_insert_with(|| AccountState {
            balance_commitment: compute_balance_commitment(&recipient_addr, 0, 0),
            balance: 0,
            nonce: 0,
            wallet_type: aa::types::WalletType::LegacyEOA,
            owner_pubkey: None,
        });
    recipient.balance += tx.value;
    recipient.balance_commitment = compute_balance_commitment(&recipient_addr, recipient.balance, recipient.nonce);

    // Record account execution nullifier (Mode 1 commitment binding).
    // H("C0DL3:acct:" || sender || nonce_le) — binds this (sender, nonce) execution
    // to the block's committed state. Nonce already prevents replay; this adds
    // a cryptographic anchor linking the account state transition to the proof.
    let mut n_input = Vec::new();
    n_input.extend_from_slice(b"C0DL3:acct:");
    n_input.extend_from_slice(tx.from.as_bytes());
    n_input.extend_from_slice(&tx.nonce.to_le_bytes());
    let acct_nullifier: [u8; 32] = Sha256::digest(&n_input).into();
    state.account_nullifiers.insert(acct_nullifier);

    Ok(total_gas)
}

/// Compute merkle root from transaction hashes
fn compute_merkle_root(transactions: &[Transaction]) -> String {
    if transactions.is_empty() {
        return "0x0000000000000000000000000000000000000000000000000000000000000000".to_string();
    }

    let mut hashes: Vec<String> = transactions.iter().map(|tx| tx.hash.clone()).collect();

    while hashes.len() > 1 {
        let mut new_hashes = Vec::new();
        for chunk in hashes.chunks(2) {
            let mut hasher = Sha256::new();
            hasher.update(chunk[0].as_bytes());
            if chunk.len() > 1 {
                hasher.update(chunk[1].as_bytes());
            } else {
                hasher.update(chunk[0].as_bytes());
            }
            let result = hasher.finalize();
            new_hashes.push(format!("0x{}", hex::encode(result)));
        }
        hashes = new_hashes;
    }

    hashes[0].clone()
}

/// Compute block hash from header
fn compute_block_hash(block: &Block) -> String {
    let mut hasher = Sha256::new();
    hasher.update(block.header.height.to_le_bytes());
    hasher.update(block.header.parent_hash.as_bytes());
    hasher.update(block.header.timestamp.to_le_bytes());
    hasher.update(block.header.merkle_root.as_bytes());
    hasher.update(block.header.state_root.as_bytes());
    hasher.update(block.header.validator.as_bytes());
    hasher.update(block.header.gas_used.to_le_bytes());
    let result = hasher.finalize();
    format!("0x{}", hex::encode(result))
}

// ──────────────────────────────────────────────
// RPC Handlers
// ──────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    node: Arc<Mutex<C0DL3ZkSyncNode>>,
}

async fn root() -> &'static str {
    "C0DL3 zkSync Era Hyperchain Node API — Settlement: zkSync Era -> Ethereum L1"
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let ns = state.node.lock().unwrap().get_node_state();
    Json(json!({
        "status": "healthy",
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "node_state": ns,
    }))
}

async fn get_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let stats = state.node.lock().unwrap().get_network_stats();
    match stats {
        Ok(s) => Ok(Json(s)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_block(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    match rollup.blocks.get(&height) {
        Some(block) => Ok(Json(json!(block))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_transaction(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();

    // Check pending transactions
    let pending = node.pending_transactions.lock().unwrap();
    if let Some(tx) = pending.get(&hash) {
        return Ok(Json(json!(tx)));
    }

    // Search confirmed blocks
    let rollup = node.rollup_state.lock().unwrap();
    for block in rollup.blocks.values() {
        for tx in &block.transactions {
            if tx.hash == hash {
                return Ok(Json(json!(tx)));
            }
        }
    }

    Err(StatusCode::NOT_FOUND)
}

async fn submit_transaction(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = state.node.lock().unwrap().add_transaction(tx);
    match result {
        Ok(_) => Ok(Json(json!({"status": "success", "message": "Transaction submitted"}))),
        Err(e) => Ok(Json(json!({"status": "error", "message": e.to_string()}))),
    }
}

async fn get_hyperchain_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    Json(json!({
        "chain_id": node.hyperchain_config.chain_id,
        "name": node.hyperchain_config.name,
        "settlement_layer": "zkSync Era -> Ethereum L1",
        "era_rpc_url": node.config.settlement.era_rpc_url,
        "bridge_address": node.hyperchain_config.bridge_address,
        "validator_address": node.hyperchain_config.validator_address,
    }))
}

async fn get_l1_batches(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let batches: Vec<_> = node.l1_batches.lock().unwrap().values().cloned().collect();
    Json(json!({"batches": batches, "total_count": batches.len()}))
}

async fn get_settlement_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let batches = node.l1_batches.lock().unwrap();
    let latest = batches.values().max_by_key(|b| b.batch_number);

    Json(json!({
        "settlement_layer": "zkSync Era -> Ethereum L1",
        "era_rpc_url": node.config.settlement.era_rpc_url,
        "eth_l1_rpc_url": node.config.settlement.eth_l1_rpc_url,
        "total_batches": batches.len(),
        "latest_batch": latest.map(|b| json!({
            "batch_number": b.batch_number,
            "state_root": b.state_root,
            "timestamp": b.timestamp,
        })),
    }))
}

// Validator endpoints

async fn get_validators(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    Json(node.validator_registry.get_status_json())
}

async fn register_validator(
    State(state): State<AppState>,
    Json(registration): Json<ValidatorRegistration>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = Arc::clone(&state.node.lock().unwrap().validator_registry);
    match registry.register_validator(registration).await {
        Ok(info) => Ok(Json(json!({
            "status": "registered",
            "validator": info,
        }))),
        Err(e) => Ok(Json(json!({
            "status": "error",
            "message": e.to_string(),
        }))),
    }
}

async fn get_validator(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    match node.validator_registry.get_validator(&id) {
        Some(info) => Ok(Json(json!(info))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// Fuego endpoints (proof verification, NOT settlement)

async fn get_fuego_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let fuego = Arc::clone(&state.node.lock().unwrap().fuego_client);
    let status = fuego.get_status().await;
    Json(json!({
        "role": "proof verification + validator qualification (NOT settlement)",
        "connection": status,
    }))
}

async fn get_fuego_commitment(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let fuego = Arc::clone(&state.node.lock().unwrap().fuego_client);
    match fuego.get_commitment(&hash).await {
        Ok(commitment) => Ok(Json(json!(commitment))),
        Err(e) => Ok(Json(json!({"error": e.to_string()}))),
    }
}

// Privacy endpoints

async fn get_privacy_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    Json(node.get_privacy_status())
}

#[derive(Deserialize)]
struct CreatePrivateTransactionRequest {
    sender: String,
    recipient: String,
    amount: u64,
    sender_balance: u64,
}

async fn create_private_transaction(
    State(state): State<AppState>,
    Json(request): Json<CreatePrivateTransactionRequest>,
) -> Result<Json<privacy::PrivateTransaction>, StatusCode> {
    let mut node = state.node.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match node.create_private_transaction_rpc(
        &request.sender, &request.recipient, request.amount, request.sender_balance,
    ) {
        Ok(tx) => Ok(Json(tx)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn submit_private_transaction(
    State(state): State<AppState>,
    Json(tx): Json<privacy::PrivateTransaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    match node.process_private_transaction(tx) {
        Ok(_) => Ok(Json(json!({"status": "success", "privacy_level": "maximum"}))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn get_private_transaction(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<privacy::PrivateTransaction>, StatusCode> {
    let node = state.node.lock().unwrap();
    match node.get_private_transaction(&hash) {
        Ok(Some(tx)) => Ok(Json(tx)),
        _ => Err(StatusCode::NOT_FOUND),
    }
}

async fn verify_private_transaction(
    State(state): State<AppState>,
    Json(tx): Json<privacy::PrivateTransaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    if let Some(ref pm) = node.privacy_manager {
        match pm.verify_private_transaction(&tx) {
            Ok(valid) => Ok(Json(json!({"valid": valid}))),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

// ──────────────────────────────────────────────
// Shielded pool — client-side partial proving
// ──────────────────────────────────────────────
//
// These endpoints accept opaque commitments + proofs from wallets.
// The sequencer NEVER sees plaintext amounts, blinding factors, or spend keys.
// It only verifies that the cryptographic proofs are well-formed (early rejection).
// The SP1 guest re-verifies everything inside the ZK circuit for trustless finality.

#[derive(Debug, Deserialize)]
struct CreateWalletRequest {
    owner_pubkey: String,  // hex-encoded 32-byte pubkey
    salt: Option<String>,  // optional hex salt for deterministic address
}

/// Shield request — deposit from EVM into shielded pool.
/// The wallet generates the commitment and proofs client-side.
#[derive(Debug, Clone, Deserialize)]
struct ShieldRequestBody {
    /// Note commitment = SHA-256("C0DL3:note:" || value_commitment || recipient_pubkey)
    note_commitment: [u8; 32],
    /// Pedersen commitment to the shielded amount
    value_commitment: [u8; 32],
    /// Recipient's one-time public key (stealth address)
    recipient_pubkey: [u8; 32],
    /// Proof of knowledge of (amount, blinding) for value_commitment
    knowledge_proof: privacy::CommitmentKnowledgeProof,
    /// Bulletproofs range proof: amount in [0, 2^64)
    range_proof: Vec<u8>,
}

/// Unshield request — withdraw from shielded pool to EVM.
#[derive(Debug, Clone, Deserialize)]
struct UnshieldRequestBody {
    /// Nullifier proving this note is being spent
    nullifier: [u8; 32],
    /// The note commitment being spent
    note_commitment: [u8; 32],
    /// Pedersen commitment to the withdrawn amount
    value_commitment: [u8; 32],
    /// Proof of knowledge of (amount, blinding)
    knowledge_proof: privacy::CommitmentKnowledgeProof,
    /// Merkle proof that note_commitment exists in the note tree
    merkle_proof: Vec<([u8; 32], bool)>,
}

/// POST /shield — accept a shielded deposit (sequencer-blind).
async fn accept_shield(
    State(state): State<AppState>,
    Json(request): Json<ShieldRequestBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 1. Verify knowledge proof
    if !privacy::verify_commitment_knowledge(&request.knowledge_proof) {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "invalid knowledge proof"
        })));
    }

    // 2. Verify knowledge proof commitment matches request
    if request.knowledge_proof.commitment != request.value_commitment {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "knowledge proof commitment mismatch"
        })));
    }

    // 3. Verify note commitment derivation
    let expected_note: [u8; 32] = {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        h.update(b"C0DL3:note:");
        h.update(&request.value_commitment);
        h.update(&request.recipient_pubkey);
        h.finalize().into()
    };
    if expected_note != request.note_commitment {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "note commitment derivation mismatch"
        })));
    }

    // 4. Verify range proof
    {
        use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
        use curve25519_dalek_ng::ristretto::CompressedRistretto;
        use merlin::Transcript;

        let pc_gens = PedersenGens::default();
        let bp_gens = BulletproofGens::new(64, 1);
        let rp = RangeProof::from_bytes(&request.range_proof).map_err(|_| StatusCode::BAD_REQUEST)?;
        let mut transcript = Transcript::new(b"C0DL3-ShieldedPool-RangeProof");
        let committed = CompressedRistretto(request.value_commitment);
        if rp.verify_single(&bp_gens, &pc_gens, &mut transcript, &committed, 64).is_err() {
            return Ok(Json(json!({
                "status": "rejected",
                "reason": "invalid range proof"
            })));
        }
    }

    // All proofs valid — add note to the shielded pool
    let node = state.node.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut rollup = node.rollup_state.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let position = rollup.shielded_pool.add_note(request.note_commitment, request.recipient_pubkey);

    info!(
        "Shield accepted: note at position {} (commitment prefix: {:02x}{:02x}..)",
        position,
        request.note_commitment[0],
        request.note_commitment[1],
    );

    Ok(Json(json!({
        "status": "accepted",
        "note_position": position,
        "note_tree_root": hex::encode(rollup.shielded_pool.note_tree_root),
    })))
}

/// POST /unshield — accept a shielded withdrawal (sequencer-blind).
async fn accept_unshield(
    State(state): State<AppState>,
    Json(request): Json<UnshieldRequestBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // 1. Verify knowledge proof
    if !privacy::verify_commitment_knowledge(&request.knowledge_proof) {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "invalid knowledge proof"
        })));
    }

    // 2. Verify commitment match
    if request.knowledge_proof.commitment != request.value_commitment {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "knowledge proof commitment mismatch"
        })));
    }

    let node = state.node.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut rollup = node.rollup_state.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 3. Check nullifier freshness
    if rollup.shielded_pool.is_nullifier_spent(&request.nullifier) {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "nullifier already spent (double-spend attempt)"
        })));
    }

    // 4. Verify Merkle membership proof
    if !shielded_pool::verify_merkle_proof(
        &rollup.shielded_pool.note_tree_root,
        &request.note_commitment,
        &request.merkle_proof,
    ) {
        return Ok(Json(json!({
            "status": "rejected",
            "reason": "invalid Merkle membership proof"
        })));
    }

    // All checks pass — record nullifier as spent
    rollup.shielded_pool.nullifier_set.insert(request.nullifier);

    info!(
        "Unshield accepted: nullifier prefix {:02x}{:02x}.., nullifier_count={}",
        request.nullifier[0],
        request.nullifier[1],
        rollup.shielded_pool.nullifier_count(),
    );

    Ok(Json(json!({
        "status": "accepted",
        "nullifier_count": rollup.shielded_pool.nullifier_count(),
    })))
}

/// GET /shield/note_tree — get current note tree info for wallet Merkle proof construction.
async fn get_note_tree_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();

    let commitments: Vec<String> = rollup
        .shielded_pool
        .notes
        .iter()
        .map(|n| hex::encode(n.commitment))
        .collect();

    Json(json!({
        "note_tree_root": hex::encode(rollup.shielded_pool.note_tree_root),
        "note_count": rollup.shielded_pool.note_count(),
        "nullifier_count": rollup.shielded_pool.nullifier_count(),
        "note_commitments": commitments,
    }))
}

// ──────────────────────────────────────────────
// Prover economics RPC handlers
// ──────────────────────────────────────────────

/// POST /proof/submit — any prover submits a proof for a pending block.
/// First valid submission within the proof window wins the HEAT reward.
async fn submit_proof(
    State(state): State<AppState>,
    Json(submission): Json<ProofSubmission>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    let state_db = node.state_db.clone();
    let mut rollup = node.rollup_state.lock().unwrap();
    match rollup.accept_proof(submission) {
        Ok(proven) => {
            // Write-through: persist proven block + prover stats
            if let Err(e) = state_db.put_proven_block(proven.block_height, &proven) {
                error!("Failed to persist proven block {}: {}", proven.block_height, e);
            }
            if let Some(stats) = rollup.prover_registry.provers.get(&proven.prover_address) {
                if let Err(e) = state_db.put_prover_stats(&proven.prover_address, stats) {
                    error!("Failed to persist prover stats: {}", e);
                }
            }
            Ok(Json(json!({
                "status": "accepted",
                "proven_block": proven,
            })))
        }
        Err(e) => Ok(Json(json!({
            "status": "error",
            "message": e.to_string(),
        }))),
    }
}

/// GET /proof/pending — list all blocks currently awaiting proof (within window).
async fn get_pending_proofs(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    let current_height = rollup.block_height;

    let mut pending_blocks: Vec<serde_json::Value> = rollup
        .pending_proofs
        .keys()
        .map(|&h| {
            let status = rollup.proof_status(h, DEFAULT_PROOF_WINDOW_SECS);
            let status_str = match status {
                BlockProofStatus::Pending => "pending",
                BlockProofStatus::Proven => "proven",
                BlockProofStatus::TimedOut => "timed_out",
            };
            let submissions = rollup.pending_proofs.get(&h).map(|v| v.len()).unwrap_or(0);
            json!({
                "block_height": h,
                "proof_status": status_str,
                "submissions_received": submissions,
            })
        })
        .collect();

    pending_blocks.sort_by_key(|v| v["block_height"].as_u64().unwrap_or(0));

    Json(json!({
        "current_height": current_height,
        "proof_window_secs": DEFAULT_PROOF_WINDOW_SECS,
        "pending_blocks": pending_blocks,
    }))
}

/// GET /proof/block_input/{height} — download block input data for SP1 guest execution.
///
/// Returns the full GuestBlockInput needed by the prover service to prove a block:
///   - All transactions converted to guest format
///   - Pre-state account witnesses (plaintext balances for EVM execution accounts only)
///   - Shielded pool data (spend proofs, new notes, nullifiers, shield/unshield requests)
///   - Previous state root and note tree root
///
/// PRIVACY: Shielded pool operations use opaque commitments + proofs.
/// The prover sees only commitments, not plaintext amounts for shielded activity.
/// Plaintext balances are provided ONLY for accounts involved in EVM transactions.
async fn get_block_input(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();

    // Block must exist
    let block = match rollup.blocks.get(&height) {
        Some(b) => b.clone(),
        None => return Err(StatusCode::NOT_FOUND),
    };

    // Compute prev_state_root from parent block
    let prev_state_root = if height > 1 {
        if let Some(parent) = rollup.blocks.get(&(height - 1)) {
            hex_to_bytes32(&parent.header.state_root)
        } else {
            [0u8; 32]
        }
    } else {
        [0u8; 32]
    };

    // Convert transactions to guest format
    let transactions: Vec<serde_json::Value> = block.transactions.iter().map(|tx| {
        let from_bytes = address_str_to_bytes20(&tx.from);
        let to_bytes = if tx.to.is_empty() || tx.to == "0x" {
            None
        } else {
            Some(address_str_to_bytes20(&tx.to))
        };

        json!({
            "from": from_bytes,
            "to": to_bytes,
            "value": tx.value,
            "gas_limit": tx.gas_limit,
            "gas_price": tx.gas_price,
            "nonce": tx.nonce,
            "data": tx.data,
        })
    }).collect();

    // Build account witnesses for EVM re-execution.
    //
    // PROVER PRIVACY: Accounts involved in EVM transactions get plaintext balances
    // (unavoidable — revm needs them). All other accounts get ONLY their pre-computed
    // Pedersen commitment. The prover can verify the state root using the commitment
    // directly, without knowing the underlying balance.
    //
    // The guest program's AccountWitness.precomputed_commitment field controls this:
    //   - null: guest computes commitment from plaintext balance (EVM participants)
    //   - [u8;32]: guest uses this commitment directly (non-participants)
    //
    // Security: If a prover provides a wrong commitment, the state root check
    // in the guest fails → SP1 proof is invalid → rejected by the node.
    let evm_participants: HashSet<String> = block.transactions.iter()
        .flat_map(|tx| vec![tx.from.clone(), tx.to.clone()])
        .filter(|addr| !addr.is_empty())
        .collect();

    let accounts: Vec<serde_json::Value> = rollup.accounts.iter().map(|(addr, acct)| {
        if evm_participants.contains(addr) {
            // EVM participant: prover needs plaintext for revm execution
            json!({
                "address": addr,
                "balance": acct.balance,
                "nonce": acct.nonce,
                "code": Vec::<u8>::new(),
                "storage": Vec::<([u8; 32], [u8; 32])>::new(),
            })
        } else {
            // Non-participant: prover gets ONLY the commitment, not plaintext balance.
            // balance=0 is a dummy — the guest ignores it when precomputed_commitment is set.
            let commitment = compute_balance_commitment(addr, acct.balance, acct.nonce);
            json!({
                "address": addr,
                "balance": 0,
                "nonce": acct.nonce,
                "code": Vec::<u8>::new(),
                "storage": Vec::<([u8; 32], [u8; 32])>::new(),
                "precomputed_commitment": commitment,
            })
        }
    }).collect();

    // Build shielded pool data
    let prev_note_commitments: Vec<[u8; 32]> = rollup.shielded_pool.notes.iter()
        .map(|n| n.commitment)
        .collect();
    let prev_nullifiers: Vec<[u8; 32]> = rollup.shielded_pool.nullifier_set.iter()
        .copied()
        .collect();

    let shielded = json!({
        "spend_proofs": [],
        "new_notes": [],
        "nullifiers": [],
        "prev_nullifiers": prev_nullifiers,
        "prev_note_commitments": prev_note_commitments,
        "shield_requests": [],
        "unshield_requests": [],
    });

    // Convert AA UserOperations to guest format
    let user_operations: Vec<serde_json::Value> = block.user_operations.iter().map(|op| {
        // Look up sender and recipient commitments at block time
        let sender_old_commit = rollup.accounts.get(&op.sender)
            .map(|a| a.balance_commitment)
            .unwrap_or_else(|| compute_balance_commitment(&op.sender, 0, 0));
        let recipient_old_commit = rollup.accounts.get(&op.to)
            .map(|a| a.balance_commitment)
            .unwrap_or_else(|| compute_balance_commitment(&op.to, 0, 0));

        json!({
            "sender": op.sender,
            "nonce": op.nonce,
            "to": op.to,
            "sender_old_commitment": sender_old_commit,
            "sender_new_commitment": op.sender_new_commitment,
            "recipient_old_commitment": recipient_old_commit,
            "recipient_new_commitment": op.recipient_new_commitment,
            "amount_commitment": op.amount_commitment,
            "knowledge_proof": {
                "commitment": op.knowledge_proof.commitment,
                "announcement": op.knowledge_proof.announcement,
                "response_v": op.knowledge_proof.response_v,
                "response_r": op.knowledge_proof.response_r,
            },
            "auth_signature_r": op.auth_signature.r_point,
            "auth_signature_s": op.auth_signature.s_scalar,
            "sender_pubkey": rollup.accounts.get(&op.sender)
                .and_then(|a| a.owner_pubkey)
                .unwrap_or([0u8; 32]),
            "gas_limit": op.gas_limit,
            "gas_price": op.gas_price,
            "paymaster": op.paymaster,
        })
    }).collect();

    let block_input = json!({
        "block_height": height,
        "prev_state_root": prev_state_root,
        "transactions": transactions,
        "accounts": accounts,
        "prev_note_tree_root": rollup.shielded_pool.note_tree_root,
        "shielded": shielded,
        "block_gas_limit": block.header.gas_limit,
        "timestamp": block.header.timestamp,
        "user_operations": user_operations,
    });

    // Expected claim for the prover to verify locally
    let new_state_root = hex_to_bytes32(&block.header.state_root);
    let tx_merkle_root = hex_to_bytes32(&block.header.merkle_root);

    let expected_claim = json!({
        "block_height": height,
        "prev_state_root": format!("0x{}", hex::encode(prev_state_root)),
        "new_state_root": format!("0x{}", hex::encode(new_state_root)),
        "tx_merkle_root": format!("0x{}", hex::encode(tx_merkle_root)),
        "tx_count": block.transactions.len(),
        "total_gas_used": block.header.gas_used,
        "note_tree_root": format!("0x{}", hex::encode(rollup.shielded_pool.note_tree_root)),
        "nullifier_count": rollup.shielded_pool.nullifier_count(),
    });

    Ok(Json(json!({
        "block_height": height,
        "block_input": block_input,
        "expected_claim": expected_claim,
    })))
}

/// Convert hex state root string to [u8; 32].
fn hex_to_bytes32(hex_str: &str) -> [u8; 32] {
    let decoded = hex::decode(hex_str.trim_start_matches("0x"))
        .unwrap_or_else(|_| vec![0u8; 32]);
    let mut arr = [0u8; 32];
    let len = decoded.len().min(32);
    arr[..len].copy_from_slice(&decoded[..len]);
    arr
}

/// Convert address string to [u8; 20] (for guest transaction format).
fn address_str_to_bytes20(addr: &str) -> [u8; 20] {
    let hex_str = addr.trim_start_matches("0x");
    let decoded = hex::decode(hex_str).unwrap_or_else(|_| vec![0u8; 20]);
    let mut arr = [0u8; 20];
    let len = decoded.len().min(20);
    arr[..len].copy_from_slice(&decoded[..len]);
    arr
}

/// GET /provers — prover leaderboard sorted by blocks proved.
async fn get_provers(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    let leaderboard = rollup.prover_registry.leaderboard();

    let entries: Vec<serde_json::Value> = leaderboard
        .into_iter()
        .map(|(addr, stats)| {
            json!({
                "address": addr,
                "blocks_proved": stats.blocks_proved,
                "total_heat_earned_fwei": stats.total_heat_earned,
                "total_heat_earned": format!("{:.6}", stats.total_heat_earned as f64 / 1_000_000.0),
                "last_proved_block": stats.last_proved_block,
            })
        })
        .collect();

    Json(json!({
        "total_provers": entries.len(),
        "leaderboard": entries,
    }))
}

/// GET /economics — HEAT subsidy schedule and prover economics summary.
async fn get_economics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    let current_height = rollup.block_height;
    let summary = EconomicsSummary::build(&rollup.prover_registry, current_height);

    Json(json!({
        "economics": summary,
        "proof_system": "sp1-sovereign",
        "proof_verification": {
            "system": "SP1 (Succinct RISC-V zkVM)",
            "verification_compiled": ProverConfig::verification_available(),
            "has_vkey": !rollup.prover_config.vkey.is_empty(),
            "program_elf_hash": rollup.prover_config.program_elf_hash,
        },
        "heat_unit": "fwei (1 HEAT = 1_000_000 fwei on L3)",
        "heat_supply": "proof-of-burn only — XFG burned on Fuego → HEAT minted via Ethereal_XFG accounting (no inflation)",
        "prover_reward_model": {
            "gas_fees": "100% of base fee per block routes to winning prover",
            "proposer": "Gas priority tip routes to block proposer",
            "note": "No block subsidy. HEAT is not inflationary.",
        },
        "coldao_model": {
            "description": "CD earned by locking XFG on Fuego — governance token, NOT prover reward",
            "tiers": 8,
            "term_options": ["3 months", "12 months"],
            "apy_range": "8%–69% (staggered per tier)",
        },
    }))
}

// ──────────────────────────────────────────────
// Settlement endpoints (L3 → zkSync Era)
// ──────────────────────────────────────────────

/// GET /settlement — settlement pipeline status.
async fn get_settlement_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let sm = node.settlement_manager.lock().unwrap();
    let summary = sm.summary();
    Json(json!({
        "settlement": {
            "path": "C0DL3 (L3) → zkSync Era (L2) → Ethereum (L1)",
            "contract": summary.settlement_contract,
            "era_rpc_url": summary.era_rpc_url,
            "total_batches": summary.total_batches,
            "pending": summary.pending,
            "submitted": summary.submitted,
            "settled": summary.settled,
            "failed": summary.failed,
            "last_settled_height": summary.last_settled_height,
        }
    }))
}

/// GET /settlement/batches — list all settlement batches.
async fn get_settlement_batches(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let sm = node.settlement_manager.lock().unwrap();

    let pending = sm.batches_by_status(settlement::SettlementStatus::Pending);
    let submitted = sm.batches_by_status(settlement::SettlementStatus::Submitted);
    let settled = sm.batches_by_status(settlement::SettlementStatus::Settled);

    let format_batch = |b: &&settlement::SettlementBatch| json!({
        "batch_id": b.batch_id,
        "block_heights": b.block_heights,
        "status": b.status.to_string(),
        "proof_size_bytes": b.proof_bytes.len(),
        "l2_tx_hash": b.l2_tx_hash,
        "created_at": b.created_at,
        "settled_at": b.settled_at,
    });

    Json(json!({
        "pending": pending.iter().map(format_batch).collect::<Vec<_>>(),
        "submitted": submitted.iter().map(format_batch).collect::<Vec<_>>(),
        "settled": settled.iter().map(format_batch).collect::<Vec<_>>(),
    }))
}

// ──────────────────────────────────────────────
// Storage status endpoint
// ──────────────────────────────────────────────

/// GET /storage/status — persistent state database info
async fn get_storage_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    let db_size = node.state_db.size_on_disk();

    Json(json!({
        "engine": "sled",
        "size_bytes": db_size,
        "size_mb": format!("{:.2}", db_size as f64 / 1_048_576.0),
        "persisted_height": rollup.block_height,
        "persisted_accounts": rollup.accounts.len(),
        "persisted_blocks": rollup.blocks.len(),
        "persisted_proven": rollup.proven_blocks.len(),
        "shielded_notes": rollup.shielded_pool.notes.len(),
        "shielded_nullifiers": rollup.shielded_pool.nullifier_set.len(),
    }))
}

// ──────────────────────────────────────────────
// Faucet (testnet only)
// ──────────────────────────────────────────────

/// Faucet request body.
#[derive(Debug, Deserialize)]
struct FaucetRequest {
    address: String,
    #[serde(default = "default_faucet_amount")]
    amount: u64,
}

fn default_faucet_amount() -> u64 {
    1_000_000_000_000 // 1e12 fwei = 1000 HEAT
}

const FAUCET_ADDRESS: &str = "0xC0DL3_FAUCET_0000000000000000000001";
const MAX_FAUCET_DRIP: u64 = 10_000_000_000_000; // 10e12 fwei = 10,000 HEAT

/// POST /faucet — drip testnet HEAT to an address
async fn faucet_drip(
    State(state): State<AppState>,
    Json(req): Json<FaucetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if req.address.is_empty() || req.address.len() < 10 {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "invalid address"}))));
    }
    if req.amount > MAX_FAUCET_DRIP {
        return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": format!("max drip is {} fwei", MAX_FAUCET_DRIP)
        }))));
    }

    let node = state.node.lock().unwrap();
    let state_db = node.state_db.clone();
    let mut rollup = node.rollup_state.lock().unwrap();

    // Check faucet balance
    let faucet = match rollup.accounts.get(FAUCET_ADDRESS) {
        Some(f) if f.balance >= req.amount => f.clone(),
        Some(f) => return Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "faucet depleted",
            "remaining": f.balance,
        })))),
        None => return Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "faucet account not initialized (missing genesis?)",
        })))),
    };

    // Debit faucet
    rollup.accounts.entry(FAUCET_ADDRESS.to_string()).and_modify(|a| {
        a.balance -= req.amount;
        a.balance_commitment = compute_balance_commitment(FAUCET_ADDRESS, a.balance, a.nonce);
    });

    // Credit recipient (create if needed)
    let recipient_addr = req.address.clone();
    let recipient = rollup.accounts
        .entry(recipient_addr.clone())
        .or_insert_with(|| AccountState {
            balance_commitment: compute_balance_commitment(&recipient_addr, 0, 0),
            balance: 0,
            nonce: 0,
            wallet_type: aa::types::WalletType::LegacyEOA,
            owner_pubkey: None,
        });
    recipient.balance += req.amount;
    recipient.balance_commitment = compute_balance_commitment(&recipient_addr, recipient.balance, recipient.nonce);

    rollup.compute_state_root();

    // Persist both accounts
    if let Some(f) = rollup.accounts.get(FAUCET_ADDRESS) {
        let _ = state_db.put_account(FAUCET_ADDRESS, f);
    }
    if let Some(r) = rollup.accounts.get(&req.address) {
        let _ = state_db.put_account(&req.address, r);
    }

    let new_balance = rollup.accounts.get(&req.address).map(|a| a.balance).unwrap_or(0);
    let faucet_remaining = rollup.accounts.get(FAUCET_ADDRESS).map(|a| a.balance).unwrap_or(0);

    Ok(Json(json!({
        "status": "ok",
        "address": req.address,
        "amount": req.amount,
        "new_balance": new_balance,
        "faucet_remaining": faucet_remaining,
    })))
}

// ──────────────────────────────────────────────
// Ethereum JSON-RPC compatibility
// ──────────────────────────────────────────────

/// Standard Ethereum JSON-RPC request envelope.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    id: serde_json::Value,
}

/// POST /rpc — Ethereum-compatible JSON-RPC endpoint
///
/// Supports the minimal set wallets need to connect:
///   eth_chainId, eth_blockNumber, eth_getBalance, eth_getTransactionCount,
///   net_version, web3_clientVersion
async fn eth_json_rpc(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<serde_json::Value> {
    let node = state.node.lock().unwrap();
    let rollup = node.rollup_state.lock().unwrap();
    let chain_id = node.genesis_config.chain_id;

    let result = match req.method.as_str() {
        "eth_chainId" => {
            json!(format!("0x{:x}", chain_id))
        }
        "net_version" => {
            json!(chain_id.to_string())
        }
        "web3_clientVersion" => {
            json!("C0DL3/0.1.0/rust")
        }
        "eth_blockNumber" => {
            json!(format!("0x{:x}", rollup.block_height))
        }
        "eth_getBalance" => {
            let addr = req.params.as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let balance = rollup.accounts.get(addr).map(|a| a.balance).unwrap_or(0);
            json!(format!("0x{:x}", balance))
        }
        "eth_getTransactionCount" => {
            let addr = req.params.as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let nonce = rollup.accounts.get(addr).map(|a| a.nonce).unwrap_or(0);
            json!(format!("0x{:x}", nonce))
        }
        "eth_getBlockByNumber" => {
            let block_param = req.params.as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("latest");

            let height = if block_param == "latest" || block_param == "pending" {
                rollup.block_height
            } else {
                u64::from_str_radix(block_param.trim_start_matches("0x"), 16).unwrap_or(0)
            };

            match rollup.blocks.get(&height) {
                Some(block) => json!({
                    "number": format!("0x{:x}", block.header.height),
                    "hash": block.header.parent_hash,
                    "parentHash": block.header.parent_hash,
                    "timestamp": format!("0x{:x}", block.header.timestamp),
                    "gasUsed": format!("0x{:x}", block.header.gas_used),
                    "gasLimit": format!("0x{:x}", block.header.gas_limit),
                    "stateRoot": block.header.state_root,
                    "transactionsRoot": block.header.merkle_root,
                    "transactions": block.transactions.iter().map(|tx| &tx.hash).collect::<Vec<_>>(),
                }),
                None => json!(null),
            }
        }
        "eth_gasPrice" => {
            json!("0x3b9aca00") // 1 gwei default
        }
        _ => {
            return Json(json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "error": {
                    "code": -32601,
                    "message": format!("method {} not supported", req.method),
                }
            }));
        }
    };

    Json(json!({
        "jsonrpc": "2.0",
        "id": req.id,
        "result": result,
    }))
}

// ──────────────────────────────────────────────
// Account Abstraction endpoints
// ──────────────────────────────────────────────

/// POST /aa/create_wallet — create a new AA PrivateWallet account.
async fn create_aa_wallet(
    State(state): State<AppState>,
    Json(body): Json<CreateWalletRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Parse owner_pubkey from hex
    let pubkey_bytes = match hex::decode(body.owner_pubkey.trim_start_matches("0x")) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": "invalid owner_pubkey: expected 64 hex chars"
        })))),
    };

    let salt = match &body.salt {
        Some(s) => {
            match hex::decode(s.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    arr
                }
                _ => return Err((StatusCode::BAD_REQUEST, Json(json!({
                    "error": "invalid salt: expected 64 hex chars"
                })))),
            }
        }
        None => [0u8; 32],
    };

    let address = aa::wallet_factory::derive_wallet_address(&pubkey_bytes, &salt);
    let account = aa::wallet_factory::create_wallet_account(&address, pubkey_bytes);

    let node = state.node.lock().unwrap();
    let state_db = node.state_db.clone();
    let mut rollup = node.rollup_state.lock().unwrap();
    let balance_commitment = account.balance_commitment;
    rollup.accounts.insert(address.clone(), account.clone());
    rollup.compute_state_root();

    // Persist new account
    if let Err(e) = state_db.put_account(&address, &account) {
        error!("Failed to persist new wallet {}: {}", address, e);
    }

    Ok(Json(json!({
        "address": address,
        "wallet_type": "PrivateWallet",
        "balance_commitment": hex::encode(balance_commitment),
    })))
}

/// POST /aa/send — submit and execute a UserOperation (AA private transfer).
async fn aa_send(
    State(state): State<AppState>,
    Json(op): Json<aa::types::UserOperation>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let node = state.node.lock().unwrap();
    let state_db = node.state_db.clone();
    let mut rollup = node.rollup_state.lock().unwrap();

    // Verify sender exists and is a PrivateWallet
    let sender = match rollup.accounts.get(&op.sender) {
        Some(a) => a.clone(),
        None => return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": "sender account not found"
        })))),
    };

    if sender.wallet_type != aa::types::WalletType::PrivateWallet {
        return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": "sender is not a PrivateWallet (use /submit_transaction for legacy EOA)"
        }))));
    }

    let sender_pubkey = match sender.owner_pubkey {
        Some(pk) => pk,
        None => return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": "sender has no owner_pubkey"
        })))),
    };

    // Get or create recipient
    let recipient_commit = rollup.accounts
        .get(&op.to)
        .map(|a| a.balance_commitment)
        .unwrap_or_else(|| compute_balance_commitment(&op.to, 0, 0));

    // Validate the UserOperation
    match aa::validation::validate_user_operation(
        &op,
        &sender_pubkey,
        &sender.balance_commitment,
        &recipient_commit,
    ) {
        Ok(()) => {},
        Err(e) => return Err((StatusCode::BAD_REQUEST, Json(json!({
            "error": format!("validation failed: {}", e)
        })))),
    }

    // Apply: update commitments
    rollup.accounts
        .entry(op.sender.clone())
        .and_modify(|a| {
            a.balance_commitment = op.sender_new_commitment;
            a.nonce += 1;
        });

    // Create/update recipient
    rollup.accounts
        .entry(op.to.clone())
        .or_insert_with(|| AccountState {
            balance_commitment: compute_balance_commitment(&op.to, 0, 0),
            balance: 0,
            nonce: 0,
            wallet_type: aa::types::WalletType::PrivateWallet,
            owner_pubkey: None,
        })
        .balance_commitment = op.recipient_new_commitment;

    rollup.compute_state_root();

    let sender_new = op.sender_new_commitment;
    let recipient_new = op.recipient_new_commitment;

    // Persist updated sender + recipient accounts
    if let Some(s) = rollup.accounts.get(&op.sender) {
        if let Err(e) = state_db.put_account(&op.sender, s) {
            error!("Failed to persist sender {}: {}", op.sender, e);
        }
    }
    if let Some(r) = rollup.accounts.get(&op.to) {
        if let Err(e) = state_db.put_account(&op.to, r) {
            error!("Failed to persist recipient {}: {}", op.to, e);
        }
    }

    // Queue op for inclusion in the next block (guest will re-verify inside SP1)
    node.pending_aa_operations.lock().unwrap().push(op);

    Ok(Json(json!({
        "status": "accepted",
        "sender_new_commitment": hex::encode(sender_new),
        "recipient_new_commitment": hex::encode(recipient_new),
    })))
}

// ──────────────────────────────────────────────
// CLI
// ──────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "c0dl3-zksync")]
#[command(about = "C0DL3 zkSync Era Hyperchain Node — Privacy L3 settling to Ethereum via zkSync Era")]
#[command(version)]
struct Cli {
    #[arg(long, default_value = "info")]
    log_level: String,
    #[arg(long, default_value = "./data")]
    data_dir: String,

    // Settlement (zkSync Era → Ethereum L1)
    #[arg(long, default_value = "http://localhost:3050", help = "zkSync Era RPC URL (settlement layer)")]
    era_rpc_url: String,
    #[arg(long, default_value = "http://localhost:8545", help = "Ethereum L1 RPC URL (finality monitoring)")]
    eth_rpc_url: String,

    // Fuego (proof verification only)
    #[arg(long, default_value = "http://127.0.0.1:28280", help = "Fuego RPC URL (proof verification + validator qualification)")]
    fuego_rpc_url: String,
    #[arg(long, default_value = "true", help = "Connect to Fuego testnet")]
    fuego_testnet: bool,

    // Network
    #[arg(long, default_value = "30333")]
    p2p_port: u16,
    #[arg(long, default_value = "9944")]
    rpc_port: u16,

    // Hyperchain
    #[arg(long, default_value = "789779", help = "Chain ID (0xC0D13 = C0DL3 testnet)")]
    hyperchain_id: u64,
    #[arg(long, default_value = "0x2233445566778899001122334455667788990011")]
    validator_address: String,
    #[arg(long, default_value = "0x3344556677889900112233445566778899001122")]
    bridge_address: String,
    #[arg(long, default_value = "0x4455667788990011223344556677889900112233")]
    l1_contract_address: String,

    // Sequencer
    #[arg(long, default_value = "2", help = "Block production interval in seconds")]
    block_time: u64,
    #[arg(long, default_value = "100", help = "Max transactions per block")]
    max_txs_per_block: u32,
    #[arg(long, default_value = "10", help = "Blocks per L1 batch")]
    batch_size: u32,
    #[arg(long, default_value = "300", help = "Max seconds before batch submission")]
    batch_timeout: u64,

    // Proof verification (SP1 sovereign prover)
    #[arg(long, help = "Path to SP1 verifying key file (required for proof verification)")]
    prover_vkey: Option<String>,
}

fn create_node_config(cli: &Cli) -> NodeConfig {
    NodeConfig {
        network: NetworkConfig {
            data_dir: cli.data_dir.clone(),
            p2p_port: cli.p2p_port,
            listen_addr: "0.0.0.0".to_string(),
            bootstrap_peers: vec![
                "/ip4/127.0.0.1/tcp/30334/p2p/QmBootstrap1".to_string(),
            ],
            max_peers: 50,
        },
        rpc: RPCConfig {
            port: cli.rpc_port,
            host: "127.0.0.1".to_string(),
            cors_origins: vec!["*".to_string()],
            max_connections: 100,
        },
        sequencer: SequencerConfig {
            enabled: true,
            block_time_secs: cli.block_time,
            max_txs_per_block: cli.max_txs_per_block,
            batch_size: cli.batch_size,
            batch_timeout_secs: cli.batch_timeout,
        },
        zksync: ZkSyncConfig {
            hyperchain_id: cli.hyperchain_id,
            validator_address: cli.validator_address.clone(),
            bridge_address: cli.bridge_address.clone(),
            l1_contract_address: cli.l1_contract_address.clone(),
            l1_batch_commitment: true,
            zk_proof_generation: true,
        },
        fuego: FuegoL1Config {
            rpc_url: cli.fuego_rpc_url.clone(),
            network: if cli.fuego_testnet { FuegoNetwork::Testnet } else { FuegoNetwork::Mainnet },
            poll_interval_secs: 30,
        },
        settlement: SettlementConfig {
            era_rpc_url: cli.era_rpc_url.clone(),
            eth_l1_rpc_url: cli.eth_rpc_url.clone(),
        },
    }
}

// ──────────────────────────────────────────────
// Main
// ──────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = create_node_config(&cli);

    // Run security checks
    info!("Running security checks...");
    let mut security_manager = security::SecurityFixManager::new()?;
    security_manager.run_security_tests()?;
    let metrics = security_manager.get_metrics();
    info!("Security score: {}%", metrics.security_score);

    info!("C0DL3 zkSync Era Hyperchain Node starting...");
    info!("Settlement: C0DL3 -> zkSync Era ({}) -> Ethereum L1 ({})",
        config.settlement.era_rpc_url, config.settlement.eth_l1_rpc_url);
    info!("Fuego (proof verification): {}", config.fuego.rpc_url);
    info!("P2P port: {}, RPC port: {}", cli.p2p_port, cli.rpc_port);
    info!("Hyperchain ID: {}", cli.hyperchain_id);
    info!("Sequencer: {}s blocks, {} txs/block, {} blocks/batch",
        cli.block_time, cli.max_txs_per_block, cli.batch_size);

    // Build prover configuration from CLI flags
    let prover_vkey = if let Some(ref path) = cli.prover_vkey {
        std::fs::read(path)
            .map_err(|e| anyhow!("Failed to read SP1 verifying key from {}: {}", path, e))?
    } else {
        vec![]
    };
    let prover_config = ProverConfig::new(prover_vkey);
    info!("Proof verification: {}", ProverConfig::label());

    let mut node = C0DL3ZkSyncNode::new(config, prover_config);

    if let Err(e) = node.start().await {
        error!("Node failed to start: {}", e);
        process::exit(1);
    }

    Ok(())
}
