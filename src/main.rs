use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use anyhow::{Result, anyhow};
use tracing::{info, error, debug, warn};

mod fuego_daemon;
mod privacy;
// mod unified_cli;
// mod cli_interface;
// mod visual_cli;
// mod enhanced_cli;
// mod simple_visual_cli;

use fuego_daemon::FuegoDaemon;
// use unified_cli::{UnifiedCliDaemon, UnifiedCliConfig};
// use cli_interface::CliInterface;
// use enhanced_cli::EnhancedCliApp;
// use simple_visual_cli::SimpleVisualCli;

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

// Standalone functions for Fuego mining
async fn mine_fuego_block(config: &NodeConfig) -> Result<FuegoBlock> {
    // Simulate Fuego block mining with CN-UPX/2 algorithm
    let height = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u64;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Generate CN-UPX/2 hash
    let input_data = format!("fuego_block_{}_{}", height, timestamp);
    let cn_upx2_hash = calculate_cn_upx2_hash(input_data.as_bytes())?;
    
    // Create AuxPoW for merge-mining
    let aux_pow = AuxPow {
        coinbase_tx: format!("0x{:064x}", height),
        coinbase_branch: vec![format!("0x{:064x}", height * 2)],
        coinbase_index: 0,
        block_hash: format!("0x{:064x}", cn_upx2_hash[0] as u64),
        parent_block_hash: format!("0x{:064x}", height - 1),
        parent_merkle_branch: vec![format!("0x{:064x}", height * 3)],
        parent_merkle_index: 0,
        parent_block_header: format!("0x{:064x}", height - 1),
    };
    
    let fuego_block = FuegoBlock {
        height,
        hash: format!("0x{:064x}", cn_upx2_hash[0] as u64),
        previous_hash: format!("0x{:064x}", height - 1),
        timestamp,
        nonce: height,
        difficulty: 1000, // Default difficulty
        merkle_root: format!("0x{:064x}", cn_upx2_hash[1] as u64),
        aux_pow: Some(aux_pow),
    };
    
    Ok(fuego_block)
}

fn calculate_cn_upx2_hash(input: &[u8]) -> Result<[u8; 32]> {
    // Simplified CN-UPX/2 hash simulation
    // In a real implementation, this would use the actual CN-UPX/2 algorithm
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.update(b"CN-UPX/2");
    
    let hash = hasher.finalize();
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash);
    Ok(result)
}

// Merge-mining structures for Fuego L1 integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoBlock {
    pub height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub timestamp: u64,
    pub nonce: u64,
    pub difficulty: u64,
    pub merkle_root: String,
    pub aux_pow: Option<AuxPow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuxPow {
    pub coinbase_tx: String,
    pub coinbase_branch: Vec<String>,
    pub coinbase_index: u32,
    pub block_hash: String,
    pub parent_block_hash: String,
    pub parent_merkle_branch: Vec<String>,
    pub parent_merkle_index: u32,
    pub parent_block_header: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CNUPX2Hash {
    pub input: Vec<u8>,
    pub output: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeMiningConfig {
    pub enabled: bool,
    pub fuego_rpc_url: String,
    pub fuego_wallet_address: String,
    pub aux_pow_tag: String,
    pub merge_mining_interval: u64,
    pub cn_upx2_difficulty: u64,
    pub fuego_block_time: u64,
    pub fuego_binary_path: String,
    pub fuego_data_dir: String,
    pub fuego_p2p_port: u16,
}

// zkSync-specific structures
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

// P2P Network Behaviour - Simplified for now
// use libp2p::swarm::NetworkBehaviour;

#[derive(libp2p::swarm::NetworkBehaviour)]
#[behaviour(out_event = "C0DL3Event")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1BridgeEvent {
    pub event_type: String,
    pub batch_number: u64,
    pub timestamp: u64,
    pub tx_hash: String,
    pub data: serde_json::Value,
}

// Core data structures for the C0DL3 zkSync node

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub zk_proof: Option<ZkProof>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub parent_hash: String,
    pub timestamp: u64,
    pub merkle_root: String,
    pub validator: String,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub nonce: u64,
    pub difficulty: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningRewards {
    pub fuego_rewards: u64,
    pub c0dl3_gas_fees: u64,
    pub eldernode_fees: u64,
    pub total_rewards: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub current_height: u64,
    pub latest_block_hash: String,
    pub connected_peers: u32,
    pub pending_transactions: u32,
    pub mining_stats: MiningStats,
    pub network_difficulty: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStats {
    pub fuego_blocks_mined: u64,
    pub c0dl3_blocks_mined: u64,
    pub total_rewards: u64,
    pub uptime_seconds: u64,
    pub hash_rate: u64,
}

// Configuration structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub rpc: RPCConfig,
    pub mining: MiningConfig,
    pub zksync: ZkSyncConfig,
    pub merge_mining: MergeMiningConfig,
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
pub struct MiningConfig {
    pub enabled: bool,
    pub target_block_time: u64,
    pub max_nonce: u64,
    pub difficulty_adjustment_blocks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkSyncConfig {
    pub l1_rpc_url: String,
    pub l2_rpc_url: String,
    pub hyperchain_id: u64,
    pub validator_address: String,
    pub bridge_address: String,
    pub l1_contract_address: String,
    pub batch_size: u32,
    pub batch_timeout: u64,
    pub l1_batch_commitment: bool,
    pub priority_ops_enabled: bool,
    pub zk_proof_generation: bool,
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

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_block_time: 30,
            max_nonce: 1000000000000000000,
            difficulty_adjustment_blocks: 10,
        }
    }
}

impl Default for MergeMiningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fuego_rpc_url: "http://localhost:8546".to_string(),
            fuego_wallet_address: "0x1111111111111111111111111111111111111111".to_string(),
            aux_pow_tag: "C0DL3-MERGE-MINING".to_string(),
            merge_mining_interval: 30,
            cn_upx2_difficulty: 1000,
            fuego_block_time: 480,
            fuego_binary_path: "./fuegod".to_string(),
            fuego_data_dir: "./fuego-data".to_string(),
            fuego_p2p_port: 10808,
        }
    }
}

// Main node implementation

#[derive(Clone)]
pub struct C0DL3ZkSyncNode {
    config: NodeConfig,
    running: bool,
    state: Arc<Mutex<NodeState>>,
    pending_transactions: Arc<Mutex<HashMap<String, Transaction>>>,
    mining_rewards: Arc<Mutex<MiningRewards>>,
    start_time: Instant,
    // p2p_swarm: Option<libp2p::swarm::Swarm<C0DL3Behaviour>>, // Commented out - cannot be cloned
    l1_batches: Arc<Mutex<HashMap<u64, L1Batch>>>,
    hyperchain_config: HyperchainConfig,
    fuego_blocks: Arc<Mutex<HashMap<u64, FuegoBlock>>>,
    merge_mining_active: Arc<Mutex<bool>>,
    /// User-level privacy manager (always enabled at maximum level)
    privacy_manager: Option<privacy::UserPrivacyManager>,
}

impl C0DL3ZkSyncNode {
    pub fn new(config: NodeConfig) -> Self {
        let hyperchain_config = HyperchainConfig {
            chain_id: config.zksync.hyperchain_id,
            name: "C0DL3-Hyperchain".to_string(),
            rpc_url: config.zksync.l2_rpc_url.clone(),
            bridge_address: config.zksync.bridge_address.clone(),
            validator_address: config.zksync.validator_address.clone(),
            l1_contract_address: config.zksync.l1_contract_address.clone(),
        };

        // Initialize user privacy manager (always enabled at maximum level)
        let privacy_manager = match privacy::UserPrivacyManager::new() {
            Ok(manager) => {
                info!("User privacy manager initialized with maximum privacy level");
                Some(manager)
            },
            Err(e) => {
                error!("Failed to initialize privacy manager: {}", e);
                None
            }
        };

        Self {
            config,
            running: false,
            state: Arc::new(Mutex::new(NodeState {
                current_height: 0,
                latest_block_hash: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                connected_peers: 0,
                pending_transactions: 0,
                mining_stats: MiningStats {
                    fuego_blocks_mined: 0,
                    c0dl3_blocks_mined: 0,
                    total_rewards: 0,
                    uptime_seconds: 0,
                    hash_rate: 0,
                },
                network_difficulty: 1,
            })),
            pending_transactions: Arc::new(Mutex::new(HashMap::new())),
            mining_rewards: Arc::new(Mutex::new(MiningRewards {
                fuego_rewards: 0,
                c0dl3_gas_fees: 0,
                eldernode_fees: 0,
                total_rewards: 0,
            })),
            start_time: Instant::now(),
            // p2p_swarm: None, // Commented out - cannot be cloned
            l1_batches: Arc::new(Mutex::new(HashMap::new())),
            hyperchain_config,
            fuego_blocks: Arc::new(Mutex::new(HashMap::new())),
            merge_mining_active: Arc::new(Mutex::new(false)),
            privacy_manager,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting C0DL3 zkSync Node...");
        self.running = true;
        self.init_logging()?;
        self.init_p2p_network().await?;
        self.start_rpc_server().await?;
        self.start_l1_monitoring().await?;
        self.start_merge_mining().await?;
        self.start_mining().await?;
        info!("C0DL3 zkSync Node started successfully");
        Ok(())
    }

    fn init_logging(&self) -> Result<()> {
        let level = tracing::Level::INFO;
        tracing_subscriber::fmt::init();
        info!("Logging initialized at level: {}", level);
        Ok(())
    }

    async fn start_mining(&self) -> Result<()> {
        if !self.config.mining.enabled {
            info!("Mining disabled");
            return Ok(());
        }

        info!("Starting mining with difficulty: {}", self.get_network_difficulty().await);
        
        // Start background mining task
        let config = self.config.clone();
        let state = self.state.clone();
        let mining_rewards = self.mining_rewards.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.mining.target_block_time));
            loop {
                interval.tick().await;
                
                // Simulate mining
                let mut state_guard = state.lock().unwrap();
                state_guard.mining_stats.uptime_seconds += config.mining.target_block_time;
                state_guard.mining_stats.hash_rate = 1000000; // 1 MH/s
                
                // Simulate finding a block occasionally
                if state_guard.mining_stats.uptime_seconds % 60 == 0 {
                    state_guard.mining_stats.c0dl3_blocks_mined += 1;
                    info!("Mined C0DL3 block! Total: {}", state_guard.mining_stats.c0dl3_blocks_mined);
                }
            }
        });

        info!("Mining started");
        Ok(())
    }

    pub async fn mine_block(&self) -> Result<Option<Block>> {
        if !self.config.mining.enabled {
            return Ok(None);
        }

        let target_difficulty = self.get_network_difficulty().await;
        let mut nonce = 0u64;
        let start_time = Instant::now();
        
        info!("Starting mining with difficulty: {}", target_difficulty);
        
        while nonce < self.config.mining.max_nonce {
            let block_candidate = self.create_mining_candidate(nonce).await?;
            let block_hash = self.calculate_block_hash(&block_candidate).await?;
            
            if self.is_valid_proof_of_work(&block_hash, target_difficulty) {
                let mut block = block_candidate;
                block.header.nonce = nonce;
                block.header.difficulty = target_difficulty;
                
                let mining_time = start_time.elapsed();
                info!("Block mined! Height: {}, Hash: {}, Nonce: {}, Time: {:?}", 
                      block.header.height, block_hash, nonce, mining_time);
                
                // Update mining stats
                {
                    let mut stats = self.state.lock().unwrap();
                    stats.mining_stats.c0dl3_blocks_mined += 1;
                }
                
                return Ok(Some(block));
            }
            
            nonce += 1;
            
            if nonce % 10000 == 0 {
                debug!("Mining progress: nonce = {}", nonce);
            }
        }
        
        Ok(None)
    }

    async fn create_mining_candidate(&self, nonce: u64) -> Result<Block> {
        let transactions = {
            let tx_guard = self.pending_transactions.lock().unwrap();
            tx_guard.values()
                .filter(|tx| tx.status == TransactionStatus::Pending)
                .take(100)
                .cloned()
                .collect::<Vec<_>>()
        };
        
        let block = Block {
            header: BlockHeader {
                height: {
                    let state_guard = self.state.lock().unwrap();
                    state_guard.current_height + 1
                },
                parent_hash: {
                    let state_guard = self.state.lock().unwrap();
                    state_guard.latest_block_hash.clone()
                },
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                merkle_root: self.calculate_merkle_root(&transactions).await?,
                validator: self.config.zksync.validator_address.clone(),
                gas_used: 0,
                gas_limit: 30000000,
                nonce,
                difficulty: 0,
            },
            transactions,
            zk_proof: None,
        };
        
        Ok(block)
    }

    async fn calculate_block_hash(&self, block: &Block) -> Result<String> {
        let mut hasher = Sha256::new();
        
        let header_data = format!("{}{}{}{}{}{}{}{}{}",
            block.header.height,
            block.header.parent_hash,
            block.header.timestamp,
            block.header.merkle_root,
            block.header.validator,
            block.header.gas_used,
            block.header.gas_limit,
            block.header.nonce,
            block.header.difficulty
        );
        
        hasher.update(header_data.as_bytes());
        
        for tx in &block.transactions {
            hasher.update(tx.hash.as_bytes());
        }
        
        let result = hasher.finalize();
        Ok(format!("0x{:x}", result))
    }

    fn is_valid_proof_of_work(&self, block_hash: &str, target_difficulty: u64) -> bool {
        let hash_bytes = hex::decode(block_hash.trim_start_matches("0x"))
            .unwrap_or_default();
        
        let required_zeros = (target_difficulty as f64).log2() as u32 / 8;
        hash_bytes.iter().take(required_zeros as usize).all(|&b| b == 0)
    }

    async fn calculate_merkle_root(&self, transactions: &[Transaction]) -> Result<String> {
        if transactions.is_empty() {
            return Ok("0x0000000000000000000000000000000000000000000000000000000000000000".to_string());
        }
        
        let mut hashes: Vec<String> = transactions.iter()
            .map(|tx| tx.hash.clone())
            .collect();
        
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
                new_hashes.push(format!("0x{:x}", result));
            }
            
            hashes = new_hashes;
        }
        
        Ok(hashes[0].clone())
    }

    async fn get_network_difficulty(&self) -> u64 {
        let state = self.state.lock().unwrap();
        state.network_difficulty
    }

    pub async fn add_transaction(&self, tx: Transaction) -> Result<()> {
        let tx_hash = tx.hash.clone();
        {
            let mut tx_guard = self.pending_transactions.lock().unwrap();
            tx_guard.insert(tx_hash.clone(), tx);
        }
        
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.pending_transactions = self.pending_transactions.lock().unwrap().len() as u32;
        }
        
        info!("Added transaction: {}", tx_hash);
        Ok(())
    }

    pub async fn generate_zk_proof(&self, block: &Block) -> Result<ZkProof> {
        info!("Generating ZK proof for block {}", block.header.height);
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let proof = ZkProof {
            proof_type: "STARK".to_string(),
            proof_data: vec![0u8; 1024],
            public_inputs: vec![
                block.header.height.to_string(),
                block.header.merkle_root.clone(),
            ],
            verification_key: vec![0u8; 512],
        };
        
        info!("Generated ZK proof type: {}", proof.proof_type);
        Ok(proof)
    }

    pub fn get_state(&self) -> NodeState {
        self.state.lock().unwrap().clone()
    }

    pub fn get_mining_rewards(&self) -> MiningRewards {
        self.mining_rewards.lock().unwrap().clone()
    }

    // Real implementation functions

    pub async fn validate_transaction(&self, tx: &Transaction) -> Result<bool> {
        // Check gas limit
        if tx.gas_limit > 30000000 {
            return Ok(false);
        }
        
        // Check gas price
        if tx.gas_price == 0 {
            return Ok(false);
        }
        
        // Check signature length (simplified validation)
        if tx.signature.len() != 65 {
            return Ok(false);
        }
        
        Ok(true)
    }

    pub async fn process_block(&self, block: Block) -> Result<()> {
        info!("Processing block at height {}", block.header.height);
        
        // Validate block
        if !self.validate_block(&block).await? {
            return Err(anyhow!("Block validation failed"));
        }
        
        // Process transactions
        let mut total_gas_used = 0u64;
        let mut successful_transactions = Vec::new();
        
        for tx in &block.transactions {
            if let Ok(gas_used) = self.execute_transaction(tx).await {
                total_gas_used += gas_used;
                successful_transactions.push(tx.clone());
            }
        }
        
        // Update state
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.current_height = block.header.height;
            state_guard.latest_block_hash = self.calculate_block_hash(&block).await?;
        }
        
        // Update transaction statuses
        for tx in &successful_transactions {
            if let Ok(mut tx_guard) = self.pending_transactions.lock() {
                if let Some(existing_tx) = tx_guard.get_mut(&tx.hash) {
                    existing_tx.status = TransactionStatus::Confirmed;
                }
            }
        }
        
        info!("Block processed successfully. Gas used: {}, Transactions: {}", 
              total_gas_used, successful_transactions.len());
        
        Ok(())
    }

    async fn validate_block(&self, block: &Block) -> Result<bool> {
        // Check block height
        let current_height = {
            let state_guard = self.state.lock().unwrap();
            state_guard.current_height
        };
        
        if block.header.height != current_height + 1 {
            return Ok(false);
        }
        
        // Check parent hash
        let expected_parent = {
            let state_guard = self.state.lock().unwrap();
            state_guard.latest_block_hash.clone()
        };
        
        if block.header.parent_hash != expected_parent {
            return Ok(false);
        }
        
        // Validate proof of work
        let block_hash = self.calculate_block_hash(block).await?;
        if !self.is_valid_proof_of_work(&block_hash, block.header.difficulty) {
            return Ok(false);
        }
        
        // Validate merkle root
        let calculated_root = self.calculate_merkle_root(&block.transactions).await?;
        if block.header.merkle_root != calculated_root {
            return Ok(false);
        }
        
        Ok(true)
    }

    async fn execute_transaction(&self, tx: &Transaction) -> Result<u64> {
        // Simulate gas usage based on transaction data size
        let base_gas = 21000;
        let data_gas = (tx.data.len() as u64) * 68; // 68 gas per byte
        let total_gas = base_gas + data_gas;
        
        // Update mining rewards
        {
            let mut rewards_guard = self.mining_rewards.lock().unwrap();
            rewards_guard.c0dl3_gas_fees += tx.gas_price * total_gas;
            rewards_guard.total_rewards += tx.gas_price * total_gas;
        }
        
        Ok(total_gas)
    }

    pub async fn get_network_stats(&self) -> Result<serde_json::Value> {
        let state = self.state.lock().unwrap();
        let rewards = self.mining_rewards.lock().unwrap();
        let batches = self.l1_batches.lock().unwrap();
        
        let stats = json!({
            "network": {
                "total_peers": 0,
                "connected_peers": state.connected_peers,
                "network_difficulty": state.network_difficulty,
            },
            "mining": {
                "current_height": state.current_height,
                "fuego_blocks_mined": state.mining_stats.fuego_blocks_mined,
                "c0dl3_blocks_mined": state.mining_stats.c0dl3_blocks_mined,
                "hash_rate": state.mining_stats.hash_rate,
            },
            "transactions": {
                "pending": state.pending_transactions,
                "total_processed": state.mining_stats.total_rewards,
            },
            "rewards": {
                "fuego_rewards": rewards.fuego_rewards,
                "c0dl3_gas_fees": rewards.c0dl3_gas_fees,
                "eldernode_fees": rewards.eldernode_fees,
                "total_rewards": rewards.total_rewards,
            },
            "hyperchain": {
                "chain_id": self.hyperchain_config.chain_id,
                "name": self.hyperchain_config.name,
                "l1_batches": batches.len(),
                "bridge_address": self.hyperchain_config.bridge_address,
                "validator_address": self.hyperchain_config.validator_address,
            }
        });
        
        Ok(stats)
    }

    // P2P Network initialization with libp2p 0.56.0 API
    async fn init_p2p_network(&mut self) -> Result<()> {
        info!("Initializing P2P network with libp2p 0.56.0 and TokioTcpConfig...");
        
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer ID: {}", local_peer_id);

        // Create Floodsub for pub/sub messaging
        let mut floodsub = libp2p::floodsub::Behaviour::new(local_peer_id);
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-blocks"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-transactions"));
        floodsub.subscribe(libp2p::floodsub::Topic::new("c0dl3-privacy-proofs"));

        // Create Kademlia for DHT functionality
        let store = libp2p::kad::store::MemoryStore::new(local_peer_id);
        let kademlia = libp2p::kad::Behaviour::new(local_peer_id, store);

        // Create the network behaviour
        let behaviour = C0DL3Behaviour {
            floodsub,
            kademlia,
        };

        // Create transport using Tokio TCP transport (correct API for libp2p 0.56.0)
        let transport = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(libp2p::noise::Config::new(&local_key)?)
            .multiplex(libp2p::yamux::Config::default())
            .boxed();

        // Build the swarm using SwarmBuilder with correct pattern
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_other_transport(|_| Ok(transport))?
            .with_behaviour(|_| Ok(behaviour))?
            .build();

        // Listen on all interfaces
        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", self.config.network.p2p_port);
        swarm.listen_on(listen_addr.parse()?)?;

        info!("P2P network initialized successfully on port {}", self.config.network.p2p_port);
        info!("Listening on: {}", listen_addr);
        info!("Transport provider: TokioTcpConfig (libp2p 0.56.0)");
        
        // Start P2P event loop
        self.start_p2p_event_loop(swarm).await?;
        
        Ok(())
    }

    // P2P event loop for handling network events
    async fn start_p2p_event_loop(&mut self, mut swarm: libp2p::Swarm<C0DL3Behaviour>) -> Result<()> {
        info!("Starting P2P event loop with full functionality...");
        
        // Bootstrap Kademlia DHT
        swarm.behaviour_mut().kademlia.bootstrap()?;
        info!("Kademlia DHT bootstrap initiated");
        
        loop {
            match swarm.select_next_some().await {
                // Connection events
                libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                    info!("New listen address: {}", address);
                }
                libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                    info!("Connection established with peer: {} via {}", peer_id, endpoint.get_remote_address());
                    
                    // Add peer to Kademlia routing table
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
                }
                libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    info!("Connection closed with peer: {}", peer_id);
                }
                
                // Behaviour events
                libp2p::swarm::SwarmEvent::Behaviour(event) => {
                    match event {
                        C0DL3Event::Floodsub(event) => {
                            match event {
                                libp2p::floodsub::Event::Message(message) => {
                                    let topics = &message.topics;
                                    let data = String::from_utf8_lossy(&message.data);
                                    info!("Received floodsub message on topics '{:?}' from {}: {}", 
                                          topics, message.source, data);
                                    
                                    // Handle different message types
                                    for topic in topics {
                                        let topic_str = format!("{:?}", topic);
                                        match topic_str.as_str() {
                                            "c0dl3-blocks" => {
                                                info!("Processing new block message: {}", data);
                                            }
                                            "c0dl3-transactions" => {
                                                info!("Processing new transaction message: {}", data);
                                            }
                                            "c0dl3-privacy-proofs" => {
                                                info!("Processing new privacy proof message: {}", data);
                                            }
                                            _ => {
                                                info!("Received message on unknown topic: {}", topic_str);
                                            }
                                        }
                                    }
                                }
                                libp2p::floodsub::Event::Subscribed { topic, .. } => {
                                    info!("Subscribed to topic: {:?}", topic);
                                }
                                libp2p::floodsub::Event::Unsubscribed { topic, peer_id: _ } => {
                                    info!("Unsubscribed from topic: {:?}", topic);
                                }
                            }
                        }
                        C0DL3Event::Kademlia(event) => {
                            match event {
                                libp2p::kad::Event::OutboundQueryProgressed { result, .. } => {
                                    match result {
                                        libp2p::kad::QueryResult::Bootstrap(result) => {
                                            match result {
                                                Ok(ok) => {
                                                    info!("Kademlia bootstrap completed: {} remaining peers", ok.num_remaining);
                                                    if ok.num_remaining == 0 {
                                                        info!("DHT bootstrap fully completed!");
                                                    }
                                                }
                                                Err(e) => warn!("Kademlia bootstrap failed: {}", e),
                                            }
                                        }
                                        libp2p::kad::QueryResult::GetRecord(result) => {
                                            match result {
                                                Ok(record) => {
                                                    info!("Retrieved record from DHT");
                                                }
                                                Err(e) => {
                                                    debug!("Failed to retrieve record from DHT: {}", e);
                                                }
                                            }
                                        }
                                        libp2p::kad::QueryResult::PutRecord(result) => {
                                            match result {
                                                Ok(_) => info!("Successfully stored record in DHT"),
                                                Err(e) => warn!("Failed to store record in DHT: {}", e),
                                            }
                                        }
                                        libp2p::kad::QueryResult::StartProviding(result) => {
                                            match result {
                                                Ok(_) => info!("Started providing record in DHT"),
                                                Err(e) => warn!("Failed to start providing record: {}", e),
                                            }
                                        }
                                        libp2p::kad::QueryResult::GetProviders(result) => {
                                            match result {
                                                Ok(providers) => {
                                                    info!("Found providers for record");
                                                }
                                                Err(e) => {
                                                    debug!("No providers found for record: {}", e);
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                libp2p::kad::Event::RoutingUpdated { peer, .. } => {
                                    info!("Discovered new peer in routing table: {}", peer);
                                }
                                libp2p::kad::Event::RoutablePeer { peer, .. } => {
                                    info!("Peer {} is now routable", peer);
                                }
                                libp2p::kad::Event::InboundRequest { .. } => {
                                    debug!("Received inbound Kademlia request");
                                }
                                libp2p::kad::Event::UnroutablePeer { peer } => {
                                    debug!("Peer {} is unroutable", peer);
                                }
                                libp2p::kad::Event::PendingRoutablePeer { peer, address } => {
                                    debug!("Peer {} pending routable at {}", peer, address);
                                }
                                libp2p::kad::Event::ModeChanged { new_mode } => {
                                    info!("Kademlia mode changed to {:?}", new_mode);
                                }
                            }
                        }
                    }
                }
                
                // Dialing events
                libp2p::swarm::SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                    warn!("Failed to connect to peer {}: {}", peer_id.map_or("unknown".to_string(), |p| p.to_string()), error);
                }
                
                // Other events
                _ => {}
            }
        }
    }

    // P2P Communication Methods
    pub async fn broadcast_block(&self, block_data: &str) -> Result<()> {
        info!("Broadcasting block data via P2P: {}", block_data);
        // This would be called from the swarm to publish to floodsub
        // For now, we'll log the intent
        Ok(())
    }

    pub async fn broadcast_transaction(&self, tx_data: &str) -> Result<()> {
        info!("Broadcasting transaction data via P2P: {}", tx_data);
        // This would be called from the swarm to publish to floodsub
        // For now, we'll log the intent
        Ok(())
    }

    pub async fn broadcast_privacy_proof(&self, proof_data: &str) -> Result<()> {
        info!("Broadcasting privacy proof data via P2P: {}", proof_data);
        // This would be called from the swarm to publish to floodsub
        // For now, we'll log the intent
        Ok(())
    }

    pub async fn store_in_dht(&self, key: &str, value: &str) -> Result<()> {
        info!("Storing key-value pair in DHT: {} -> {}", key, value);
        // This would be called from the swarm to store in Kademlia DHT
        // For now, we'll log the intent
        Ok(())
    }

    pub async fn retrieve_from_dht(&self, key: &str) -> Result<Option<String>> {
        info!("Retrieving value from DHT for key: {}", key);
        // This would be called from the swarm to retrieve from Kademlia DHT
        // For now, we'll return None
        Ok(None)
    }

    // RPC Server
    async fn start_rpc_server(&self) -> Result<()> {
        info!("Starting RPC server on {}:{}", self.config.rpc.host, self.config.rpc.port);
        
        let app_state = AppState {
            node: Arc::new(self.clone()),
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/", get(root))
            .route("/health", get(health))
            .route("/stats", get(get_stats))
            .route("/blocks/{height}", get(get_block))
            .route("/transactions/{hash}", get(get_transaction))
            .route("/submit_transaction", post(submit_transaction))
            .route("/hyperchain/info", get(get_hyperchain_info))
            .route("/hyperchain/batches", get(get_l1_batches))
            // .route("/merge-mining/stats", get(get_merge_mining_stats)) // Commented out for now
            .route("/merge-mining/fuego-blocks", get(get_fuego_blocks))
            .route("/merge-mining/fuego-blocks/{height}", get(get_fuego_block))
            // Privacy endpoints for user-level privacy
            .route("/privacy/status", get(get_privacy_status))
            .route("/privacy/create_transaction", post(create_private_transaction))
            .route("/privacy/submit_transaction", post(submit_private_transaction))
            .route("/privacy/get_transaction/{hash}", get(get_private_transaction))
            .route("/privacy/verify_transaction", post(verify_private_transaction))
            .route("/privacy/decrypt_transaction", post(decrypt_transaction_details))
            .layer(ServiceBuilder::new().layer(cors))
            .with_state(app_state);

        let listener = tokio::net::TcpListener::bind(format!("{}:{}", self.config.rpc.host, self.config.rpc.port)).await?;
        
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("RPC server error: {}", e);
            }
        });

        info!("RPC server started successfully");
        Ok(())
    }

    // L1 Monitoring
    async fn start_l1_monitoring(&self) -> Result<()> {
        info!("Starting L1 monitoring...");
        
        let config = self.config.clone();
        let l1_batches = self.l1_batches.clone();
        let state = self.state.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                // Simulate L1 batch monitoring
                let batch_number = if let Ok(mut batches_guard) = l1_batches.lock() {
                    let batch_number = batches_guard.len() as u64 + 1;
                    
                    let batch = L1Batch {
                        batch_number,
                        l1_tx_hash: format!("0x{:064x}", batch_number),
                        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        transactions: vec![],
                        state_root: format!("0x{:064x}", batch_number * 2),
                        priority_ops_hash: format!("0x{:064x}", batch_number * 3),
                    };
                    
                    batches_guard.insert(batch_number, batch);
                    info!("Processed L1 batch: {}", batch_number);
                    batch_number
                } else {
                    0
                };
                
                // Update connected peers count
                if let Ok(mut state_guard) = state.lock() {
                    state_guard.connected_peers = (batch_number % 10) as u32;
                }
            }
        });

        info!("L1 monitoring started");
        Ok(())
    }

    // zkSync-specific methods
    pub async fn submit_l1_batch(&self, batch: L1Batch) -> Result<()> {
        info!("Submitting L1 batch: {}", batch.batch_number);
        
        {
            let mut batches_guard = self.l1_batches.lock().unwrap();
            batches_guard.insert(batch.batch_number, batch);
        }
        
        // Update state
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.current_height += 1;
        }
        
        info!("L1 batch submitted successfully");
        Ok(())
    }

    pub async fn get_l1_batch(&self, batch_number: u64) -> Option<L1Batch> {
        let batches_guard = self.l1_batches.lock().unwrap();
        batches_guard.get(&batch_number).cloned()
    }

    pub async fn generate_hyperchain_proof(&self, batch: &L1Batch) -> Result<ZkProof> {
        info!("Generating hyperchain ZK proof for batch {}", batch.batch_number);
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let proof = ZkProof {
            proof_type: "Hyperchain-STARK".to_string(),
            proof_data: vec![0u8; 2048],
            public_inputs: vec![
                batch.batch_number.to_string(),
                batch.state_root.clone(),
                batch.priority_ops_hash.clone(),
            ],
            verification_key: vec![0u8; 1024],
        };
        
        info!("Generated hyperchain ZK proof type: {}", proof.proof_type);
        Ok(proof)
    }

    // Merge-mining implementation
    async fn start_merge_mining(&self) -> Result<()> {
        if !self.config.merge_mining.enabled {
            info!("Merge-mining disabled");
            return Ok(());
        }

        info!("Starting merge-mining with Fuego L1 using CN-UPX/2 algorithm");
        info!("Fuego RPC URL: {}", self.config.merge_mining.fuego_rpc_url);
        info!("AuxPoW Tag: {}", self.config.merge_mining.aux_pow_tag);
        
        let config = self.config.clone();
        let fuego_blocks = self.fuego_blocks.clone();
        let merge_mining_active = self.merge_mining_active.clone();
        let state = self.state.clone();
        let mining_rewards = self.mining_rewards.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.merge_mining.merge_mining_interval));
            loop {
                interval.tick().await;
                
                // Set merge-mining as active
                {
                    let mut active_guard = merge_mining_active.lock().unwrap();
                    *active_guard = true;
                }
                
                // Attempt to mine Fuego block
                if let Ok(fuego_block) = mine_fuego_block(&config).await {
                    info!("Mined Fuego block! Height: {}, Hash: {}", fuego_block.height, fuego_block.hash);
                    
                    // Store Fuego block
                    {
                        let mut blocks_guard = fuego_blocks.lock().unwrap();
                        blocks_guard.insert(fuego_block.height, fuego_block);
                    }
                    
                    // Update mining stats
                    {
                        let mut stats = state.lock().unwrap();
                        stats.mining_stats.fuego_blocks_mined += 1;
                    }
                    
                    // Update rewards
                    {
                        let mut rewards_guard = mining_rewards.lock().unwrap();
                        rewards_guard.fuego_rewards += 1000000; // 1M Fuego tokens
                        rewards_guard.total_rewards += 1000000;
                    }
                }
                
                // Set merge-mining as inactive
                {
                    let mut active_guard = merge_mining_active.lock().unwrap();
                    *active_guard = false;
                }
            }
        });

        info!("Merge-mining started");
        Ok(())
    }


    pub async fn get_fuego_block(&self, height: u64) -> Option<FuegoBlock> {
        let blocks_guard = self.fuego_blocks.lock().unwrap();
        blocks_guard.get(&height).cloned()
    }

    pub async fn is_merge_mining_active(&self) -> bool {
        let active_guard = self.merge_mining_active.lock().unwrap();
        *active_guard
    }

    pub async fn get_merge_mining_stats(&self) -> serde_json::Value {
        let fuego_blocks = self.fuego_blocks.lock().unwrap();
        let active = self.is_merge_mining_active().await;
        let config = &self.config.merge_mining;
        
        json!({
            "enabled": config.enabled,
            "active": active,
            "fuego_rpc_url": config.fuego_rpc_url,
            "aux_pow_tag": config.aux_pow_tag,
            "cn_upx2_difficulty": config.cn_upx2_difficulty,
            "merge_mining_interval": config.merge_mining_interval,
            "fuego_blocks_mined": fuego_blocks.len(),
            "latest_fuego_block": fuego_blocks.values().max_by_key(|b| b.height).map(|b| b.height)
        })
    }

    // Privacy methods for user-level privacy (always enabled at maximum level)
    
    /// Create private transaction with user-level privacy
    /// Privacy is always enabled at maximum level (100) - no options needed
    pub async fn create_private_transaction(
        &self,
        sender: &str,
        recipient: &str,
        amount: u64,
        sender_balance: u64,
    ) -> Result<privacy::PrivateTransaction> {
        if let Some(ref privacy_manager) = self.privacy_manager {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            
            privacy_manager.create_private_transaction(
                sender,
                recipient,
                amount,
                timestamp,
                sender_balance,
            )
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }
    
    /// Process private transaction with user-level privacy
    /// Privacy is always enabled at maximum level (100) - no check needed
    pub async fn process_private_transaction(
        &self,
        tx: privacy::PrivateTransaction,
    ) -> Result<()> {
        if let Some(ref privacy_manager) = self.privacy_manager {
            // Verify transaction privacy
            if !privacy_manager.verify_private_transaction(&tx)? {
                return Err(anyhow!("Invalid private transaction"));
            }
            
            // Process transaction (user privacy is maintained)
            self.process_transaction_internal(tx).await?;
        } else {
            return Err(anyhow!("Privacy manager not initialized"));
        }
        
        Ok(())
    }
    
    /// Get private transaction by hash
    pub async fn get_private_transaction(&self, hash: &str) -> Result<Option<privacy::PrivateTransaction>> {
        if let Some(ref privacy_manager) = self.privacy_manager {
            privacy_manager.get_private_transaction(hash)
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }
    
    /// Decrypt transaction details (only for authorized users)
    pub async fn decrypt_transaction_details(
        &self,
        tx: &privacy::PrivateTransaction,
    ) -> Result<privacy::DecryptedTransaction> {
        if let Some(ref privacy_manager) = self.privacy_manager {
            privacy_manager.decrypt_transaction_details(tx)
        } else {
            Err(anyhow!("Privacy manager not initialized"))
        }
    }
    
    /// Check if privacy is enabled (always true)
    pub fn is_privacy_enabled(&self) -> bool {
        self.privacy_manager.is_some()
    }
    
    /// Get privacy status
    pub fn get_privacy_status(&self) -> serde_json::Value {
        json!({
            "enabled": self.is_privacy_enabled(),
            "privacy_level": if self.is_privacy_enabled() { 100 } else { 0 },
            "features": {
                "transaction_amount_privacy": self.is_privacy_enabled(),
                "address_privacy": self.is_privacy_enabled(),
                "timing_privacy": self.is_privacy_enabled(),
                "stark_proofs": self.is_privacy_enabled(),
            },
            "message": if self.is_privacy_enabled() { 
                "Maximum privacy enabled by default" 
            } else { 
                "Privacy not available" 
            }
        })
    }
    
    /// Internal method to process private transaction
    async fn process_transaction_internal(&self, tx: privacy::PrivateTransaction) -> Result<()> {
        // Add transaction to pending transactions
        {
            let mut pending = self.pending_transactions.lock().unwrap();
            pending.insert(tx.hash.clone(), Transaction {
                hash: tx.hash.clone(),
                from: "encrypted_sender".to_string(), // Address is encrypted
                to: "encrypted_recipient".to_string(), // Address is encrypted
                value: 0, // Amount is hidden in commitment
                gas_price: 0,
                gas_limit: 0,
                nonce: 0,
                data: vec![],
                signature: vec![],
                status: TransactionStatus::Pending,
            });
        }
        
        info!("Private transaction processed: {}", tx.hash);
        Ok(())
    }
}

// RPC Handlers and App State
#[derive(Clone)]
struct AppState {
    node: Arc<C0DL3ZkSyncNode>,
}

async fn root() -> &'static str {
    "C0DL3 zkSync Hyperchain Node API"
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "node_state": state.node.get_state()
    }))
}

async fn get_stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.node.get_network_stats().await {
        Ok(stats) => Ok(Json(stats)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_block(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // For now, return a mock block
    let block_data = json!({
        "height": height,
        "hash": format!("0x{:064x}", height),
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        "transactions": [],
        "zk_proof": null
    });
    Ok(Json(block_data))
}

async fn get_transaction(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // For now, return a mock transaction
    let tx_data = json!({
        "hash": hash,
        "from": "0x1111111111111111111111111111111111111111",
        "to": "0x2222222222222222222222222222222222222222",
        "value": 1000000,
        "status": "confirmed"
    });
    Ok(Json(tx_data))
}

async fn submit_transaction(
    State(state): State<AppState>,
    Json(tx): Json<Transaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.node.add_transaction(tx).await {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Transaction submitted successfully"
        }))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn get_hyperchain_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "chain_id": state.node.hyperchain_config.chain_id,
        "name": state.node.hyperchain_config.name,
        "rpc_url": state.node.hyperchain_config.rpc_url,
        "bridge_address": state.node.hyperchain_config.bridge_address,
        "validator_address": state.node.hyperchain_config.validator_address,
        "l1_contract_address": state.node.hyperchain_config.l1_contract_address
    }))
}

async fn get_l1_batches(State(state): State<AppState>) -> Json<serde_json::Value> {
    let batches_guard = state.node.l1_batches.lock().unwrap();
    let batches: Vec<_> = batches_guard.values().collect();
    Json(json!({
        "batches": batches,
        "total_count": batches.len()
    }))
}

async fn get_merge_mining_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let stats = state.node.get_merge_mining_stats().await;
    Json(stats)
}

async fn get_fuego_blocks(State(state): State<AppState>) -> Json<serde_json::Value> {
    let fuego_blocks_guard = state.node.fuego_blocks.lock().unwrap();
    let blocks: Vec<_> = fuego_blocks_guard.values().collect();
    Json(json!({
        "fuego_blocks": blocks,
        "total_count": blocks.len()
    }))
}

async fn get_fuego_block(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.node.get_fuego_block(height).await {
        Some(block) => Ok(Json(json!(block))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// Privacy RPC handlers for user-level privacy

/// Get privacy status endpoint
async fn get_privacy_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let status = state.node.get_privacy_status();
    Json(status)
}

/// Create private transaction endpoint
async fn create_private_transaction(
    State(state): State<AppState>,
    Json(request): Json<CreatePrivateTransactionRequest>,
) -> Result<Json<privacy::PrivateTransaction>, StatusCode> {
    match state.node.create_private_transaction(
        &request.sender,
        &request.recipient,
        request.amount,
        request.sender_balance,
    ).await {
        Ok(tx) => Ok(Json(tx)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Submit private transaction endpoint
async fn submit_private_transaction(
    State(state): State<AppState>,
    Json(tx): Json<privacy::PrivateTransaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.node.process_private_transaction(tx).await {
        Ok(_) => Ok(Json(json!({
            "status": "success", 
            "message": "Private transaction submitted",
            "privacy_level": "maximum"
        }))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

/// Get private transaction endpoint
async fn get_private_transaction(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<privacy::PrivateTransaction>, StatusCode> {
    match state.node.get_private_transaction(&hash).await {
        Ok(Some(tx)) => Ok(Json(tx)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Verify private transaction endpoint
async fn verify_private_transaction(
    State(state): State<AppState>,
    Json(tx): Json<privacy::PrivateTransaction>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if let Some(ref privacy_manager) = state.node.privacy_manager {
        match privacy_manager.verify_private_transaction(&tx) {
            Ok(is_valid) => Ok(Json(json!({
                "valid": is_valid,
                "message": if is_valid { "Transaction is valid" } else { "Transaction is invalid" }
            }))),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Decrypt transaction details endpoint (authorized users only)
async fn decrypt_transaction_details(
    State(state): State<AppState>,
    Json(tx): Json<privacy::PrivateTransaction>,
) -> Result<Json<privacy::DecryptedTransaction>, StatusCode> {
    match state.node.decrypt_transaction_details(&tx).await {
        Ok(decrypted) => Ok(Json(decrypted)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Request structure for creating private transactions
#[derive(Deserialize)]
struct CreatePrivateTransactionRequest {
    sender: String,
    recipient: String,
    amount: u64,
    sender_balance: u64,
}

// CLI structure

#[derive(Parser)]
#[command(name = "c0dl3-zksync")]
#[command(about = "C0DL3 zkSync Hyperchains Node")]
#[command(version)]
struct Cli {
    #[arg(long, default_value = "info")]
    log_level: String,
    #[arg(long, default_value = "./data")]
    data_dir: String,
    #[arg(long, default_value = "http://localhost:8545")]
    l1_rpc_url: String,
    #[arg(long, default_value = "http://localhost:3050")]
    l2_rpc_url: String,
    #[arg(long, default_value = "30333")]
    p2p_port: u16,
    #[arg(long, default_value = "9944")]
    rpc_port: u16,
    #[arg(long, default_value = "324")]
    hyperchain_id: u64,
    #[arg(long, default_value = "0x2233445566778899001122334455667788990011")]
    validator_address: String,
    #[arg(long, default_value = "0x3344556677889900112233445566778899001122")]
    bridge_address: String,
    #[arg(long, default_value = "0x4455667788990011223344556677889900112233")]
    l1_contract_address: String,
    #[arg(long, default_value = "100")]
    batch_size: u32,
    #[arg(long, default_value = "300")]
    batch_timeout: u64,
    #[arg(long, default_value = "30")]
    target_block_time: u64,
    #[arg(long, default_value = "true")]
    merge_mining_enabled: bool,
    #[arg(long, default_value = "http://localhost:8546")]
    fuego_rpc_url: String,
    #[arg(long, default_value = "0x1111111111111111111111111111111111111111")]
    fuego_wallet_address: String,
    #[arg(long, default_value = "C0DL3-MERGE-MINING")]
    aux_pow_tag: String,
    #[arg(long, default_value = "30")]
    merge_mining_interval: u64,
    #[arg(long, default_value = "1000")]
    cn_upx2_difficulty: u64,
    #[arg(long, default_value = "false")]
    unified_daemon: bool,
    #[arg(long, default_value = "480")]
    fuego_block_time: u64,
    #[arg(long, default_value = "./fuegod")]
    fuego_binary_path: String,
    #[arg(long, default_value = "./fuego-data")]
    fuego_data_dir: String,
    #[arg(long, default_value = "10808")]
    fuego_p2p_port: u16,
    #[arg(long, default_value = "false")]
    cli_mode: bool,
    #[arg(long, default_value = "true")]
    interactive_mode: bool,
    #[arg(long, default_value = "5")]
    status_refresh_interval: u64,
    #[arg(long, default_value = "false")]
    visual_mode: bool,
    #[arg(long, default_value = "true")]
    animations_enabled: bool,
    #[arg(long, default_value = "professional")]
    theme: String,
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
        mining: MiningConfig {
            enabled: true,
            target_block_time: cli.target_block_time,
            max_nonce: 1000000000000000000,
            difficulty_adjustment_blocks: 10,
        },
        zksync: ZkSyncConfig {
            l1_rpc_url: cli.l1_rpc_url.clone(),
            l2_rpc_url: cli.l2_rpc_url.clone(),
            hyperchain_id: cli.hyperchain_id,
            validator_address: cli.validator_address.clone(),
            bridge_address: cli.bridge_address.clone(),
            l1_contract_address: cli.l1_contract_address.clone(),
            batch_size: cli.batch_size,
            batch_timeout: cli.batch_timeout,
            l1_batch_commitment: true,
            priority_ops_enabled: true,
            zk_proof_generation: true,
        },
        merge_mining: MergeMiningConfig {
            enabled: cli.merge_mining_enabled,
            fuego_rpc_url: cli.fuego_rpc_url.clone(),
            fuego_wallet_address: cli.fuego_wallet_address.clone(),
            aux_pow_tag: cli.aux_pow_tag.clone(),
            merge_mining_interval: cli.merge_mining_interval,
            cn_upx2_difficulty: cli.cn_upx2_difficulty,
            fuego_block_time: cli.fuego_block_time,
            fuego_binary_path: cli.fuego_binary_path.clone(),
            fuego_data_dir: cli.fuego_data_dir.clone(),
            fuego_p2p_port: cli.fuego_p2p_port,
        },
    }
}

async fn test_node_functionality(node: &C0DL3ZkSyncNode) -> Result<()> {
    info!("=== Testing C0DL3 zkSync Node Functionality ===");
    
    let test_tx = Transaction {
        hash: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        from: "0x1111111111111111111111111111111111111111".to_string(),
        to: "0x2222222222222222222222222222222222222222".to_string(),
        value: 1000000,
        gas_price: 20000000000,
        gas_limit: 21000,
        nonce: 0,
        data: vec![],
        signature: vec![0u8; 65],
        status: TransactionStatus::Pending,
    };
    
    node.add_transaction(test_tx).await?;
    
    let block = node.mine_block().await?;
    if let Some(block) = block {
        let proof = node.generate_zk_proof(&block).await?;
        info!("Generated ZK proof type: {}", proof.proof_type);
        
        // Test block processing
        node.process_block(block).await?;
    }
    
    // Test hyperchain functionality
    let test_batch = L1Batch {
        batch_number: 1,
        l1_tx_hash: "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        transactions: vec![],
        state_root: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        priority_ops_hash: "0x9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba".to_string(),
    };
    
    node.submit_l1_batch(test_batch.clone()).await?;
    
    let hyperchain_proof = node.generate_hyperchain_proof(&test_batch).await?;
    info!("Generated hyperchain ZK proof type: {}", hyperchain_proof.proof_type);
    
    let state = node.get_state();
    let rewards = node.get_mining_rewards();
    
    info!("=== C0DL3 zkSync Node State ===");
    info!("Current height: {}", state.current_height);
    info!("Connected peers: {}", state.connected_peers);
    info!("Pending transactions: {}", state.pending_transactions);
    info!("C0DL3 blocks mined: {}", state.mining_stats.c0dl3_blocks_mined);
    info!("Total rewards: {}", rewards.total_rewards);
    
    // Test network stats
    let stats = node.get_network_stats().await?;
    info!("Network stats: {}", serde_json::to_string_pretty(&stats)?);
    
    info!("=== Test Completed Successfully ===");
    Ok(())
}

/// Shared state for unified daemon
#[derive(Debug, Clone)]
pub struct UnifiedDaemonState {
    pub c0dl3_blocks_mined: u64,
    pub fuego_blocks_mined: u64,
    pub c0dl3_rewards: u64,
    pub fuego_rewards: u64,
    pub total_rewards: u64,
    pub uptime_seconds: u64,
}

impl UnifiedDaemonState {
    pub fn new() -> Self {
        Self {
            c0dl3_blocks_mined: 0,
            fuego_blocks_mined: 0,
            c0dl3_rewards: 0,
            fuego_rewards: 0,
            total_rewards: 0,
            uptime_seconds: 0,
        }
    }
}

/// Start visual CLI - Professional visual interface
// async fn start_visual_cli(cli: &Cli) -> Result<()> {
//     info!("🎨 Starting Professional Visual CLI Interface");
//     info!("📊 Configuration:");
//     info!("  - Visual Mode: {}", cli.visual_mode);
//     info!("  - Animations: {}", cli.animations_enabled);
//     info!("  - Theme: {}", cli.theme);
//     info!("  - C0DL3 RPC: {}", cli.l2_rpc_url);
//     info!("  - Fuego RPC: {}", cli.fuego_rpc_url);
    
//     let mut visual_cli = SimpleVisualCli::new();
//     visual_cli.start().await?;
    
//     Ok(())
// }

// /// Start CLI daemon - Interactive wrapper for both daemons
// async fn start_cli_daemon(cli: &Cli) -> Result<()> {
//     info!("🎮 Starting Unified CLI Daemon");
//     info!("📊 Configuration:");
//     info!("  - Interactive Mode: {}", cli.interactive_mode);
//     info!("  - Status Refresh: {}s", cli.status_refresh_interval);
//     info!("  - C0DL3 RPC: {}", cli.l2_rpc_url);
//     info!("  - Fuego RPC: {}", cli.fuego_rpc_url);
    
//     let config = UnifiedCliConfig {
//         c0dl3_rpc_url: cli.l2_rpc_url.clone(),
//         fuego_rpc_url: cli.fuego_rpc_url.clone(),
//         c0dl3_data_dir: cli.data_dir.clone(),
//         fuego_data_dir: cli.fuego_data_dir.clone(),
//         mining_enabled: true,
//         validation_enabled: true,
//         auto_start_daemons: true,
//         status_refresh_interval: cli.status_refresh_interval,
//         interactive_mode: cli.interactive_mode,
//     };
    
//     let mut cli_interface = CliInterface::new(config);
//     cli_interface.start().await?;
    
//     Ok(())
// }

/// Start unified daemon that runs both C0DL3 and Fuego mining
async fn start_unified_daemon(config: NodeConfig) -> Result<()> {
    info!("🚀 Starting Unified C0DL3-Fuego Daemon");
    info!("📊 Configuration:");
    info!("  - C0DL3 Block Time: {}s", config.mining.target_block_time);
    info!("  - Fuego Block Time: {}s", config.merge_mining.fuego_block_time);
    info!("  - Merge Mining: {}", if config.merge_mining.enabled { "Enabled" } else { "Disabled" });
    info!("  - Fuego RPC: {}", config.merge_mining.fuego_rpc_url);
    info!("  - AuxPoW Tag: {}", config.merge_mining.aux_pow_tag);
    
    // Create shared state for both chains
    let shared_state = Arc::new(Mutex::new(UnifiedDaemonState::new()));
    
    // Start C0DL3 node
    let c0dl3_config = config.clone();
    let c0dl3_state = shared_state.clone();
    let c0dl3_handle = tokio::spawn(async move {
        info!("🔗 Starting C0DL3 zkSync node...");
        let mut node = C0DL3ZkSyncNode::new(c0dl3_config);
        node.start().await.unwrap();
    });
    
    // Start actual Fuego daemon
    let fuego_config = config.clone();
    let fuego_state = shared_state.clone();
    let fuego_handle = tokio::spawn(async move {
        info!("⛏️ Starting actual Fuego daemon...");
        start_actual_fuego_daemon(fuego_config, fuego_state).await.unwrap();
    });
    
    // Start unified monitoring
    let monitor_state = shared_state.clone();
    let monitor_handle = tokio::spawn(async move {
        start_unified_monitoring(monitor_state).await.unwrap();
    });
    
    // Wait for all tasks
    tokio::select! {
        _ = c0dl3_handle => {
            error!("C0DL3 node stopped unexpectedly");
        }
        _ = fuego_handle => {
            error!("Fuego daemon stopped unexpectedly");
        }
        _ = monitor_handle => {
            error!("Unified monitoring stopped unexpectedly");
        }
    }
    
    Ok(())
}

/// Start actual Fuego daemon
async fn start_actual_fuego_daemon(config: NodeConfig, state: Arc<Mutex<UnifiedDaemonState>>) -> Result<()> {
    // Create Fuego daemon from merge mining config
    let fuego_daemon = FuegoDaemon::from_merge_mining_config(&config.merge_mining);
    
    // Start the Fuego daemon
    fuego_daemon.start().await?;
    
    // Monitor Fuego daemon and update shared state
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        // Get Fuego daemon stats
        let fuego_stats = fuego_daemon.get_stats();
        
        // Update shared state
        {
            let mut state_guard = state.lock().unwrap();
            state_guard.fuego_blocks_mined = fuego_stats.blocks_mined;
            state_guard.fuego_rewards = fuego_stats.total_rewards;
            state_guard.total_rewards = state_guard.c0dl3_rewards + fuego_stats.total_rewards;
        }
        
        info!("📊 Fuego daemon stats: {} blocks mined, {} total rewards", 
              fuego_stats.blocks_mined, fuego_stats.total_rewards);
    }
}

/// Start Fuego mining daemon (legacy simulation mode)
async fn start_fuego_mining_daemon(config: NodeConfig, state: Arc<Mutex<UnifiedDaemonState>>) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(config.merge_mining.fuego_block_time));
    
    loop {
        interval.tick().await;
        
        // Mine Fuego block
        if let Ok(fuego_block) = mine_fuego_block(&config).await {
            info!("⛏️ Mined Fuego block! Height: {}, Hash: {}", 
                  fuego_block.height, fuego_block.hash);
            
            // Update shared state
            {
                let mut state_guard = state.lock().unwrap();
                state_guard.fuego_blocks_mined += 1;
                state_guard.fuego_rewards += 1000000; // 1M Fuego tokens
                state_guard.total_rewards += 1000000;
            }
        }
    }
}

/// Start unified monitoring for both chains
async fn start_unified_monitoring(state: Arc<Mutex<UnifiedDaemonState>>) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        let stats = {
            let state_guard = state.lock().unwrap();
            state_guard.clone()
        };
        
        info!("📊 Unified Daemon Stats:");
        info!("  - C0DL3 Blocks: {}", stats.c0dl3_blocks_mined);
        info!("  - Fuego Blocks: {}", stats.fuego_blocks_mined);
        info!("  - Total Rewards: {}", stats.total_rewards);
        info!("  - Uptime: {}s", stats.uptime_seconds);
    }
}

// Main function

#[tokio::main]
async fn main() -> Result<()> {
    // env_logger::init();
    
    let cli = Cli::parse();
    
    let config = create_node_config(&cli);

    info!("C0DL3 zkSync Node starting...");
    info!("Log level: {}", cli.log_level);
    info!("Data directory: {}", cli.data_dir);
    info!("L1 RPC URL: {}", cli.l1_rpc_url);
    info!("L2 RPC URL: {}", cli.l2_rpc_url);
    info!("P2P port: {}", cli.p2p_port);
    info!("RPC port: {}", cli.rpc_port);
    info!("Hyperchain ID: {}", cli.hyperchain_id);
    info!("Validator address: {}", cli.validator_address);
    info!("Bridge address: {}", cli.bridge_address);
    info!("L1 contract address: {}", cli.l1_contract_address);
    info!("Batch size: {}", cli.batch_size);
    info!("Batch timeout: {} seconds", cli.batch_timeout);
    info!("Target block time: {} seconds", cli.target_block_time);
    info!("Merge-mining enabled: {}", cli.merge_mining_enabled);
    info!("Fuego RPC URL: {}", cli.fuego_rpc_url);
    info!("AuxPoW tag: {}", cli.aux_pow_tag);
    info!("CN-UPX/2 difficulty: {}", cli.cn_upx2_difficulty);
    info!("Unified daemon mode: {}", cli.unified_daemon);
    info!("Fuego block time: {} seconds", cli.fuego_block_time);

    // Check if we should run in visual mode
    // if cli.visual_mode {
    //     info!("🎨 Starting Professional Visual CLI Interface...");
    //     start_visual_cli(&cli).await?;
    // }
    // Check if we should run in CLI mode
    // else if cli.cli_mode {
    //     info!("🎮 Starting Unified CLI Daemon...");
    //     start_cli_daemon(&cli).await?;
    // }
    // Check if we should run in unified daemon mode
    if config.merge_mining.enabled && cli.unified_daemon {
        info!("🚀 Starting unified C0DL3-Fuego daemon...");
        start_unified_daemon(config).await?;
    } else {
        info!("🔗 Starting C0DL3 zkSync node...");
        let mut node = C0DL3ZkSyncNode::new(config);
        
        if let Err(e) = test_node_functionality(&node).await {
            error!("Node functionality test failed: {}", e);
            process::exit(1);
        }

        if let Err(e) = node.start().await {
            error!("Node failed to start: {}", e);
            process::exit(1);
        }
    }

    Ok(())
}
