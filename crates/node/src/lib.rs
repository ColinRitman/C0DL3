use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use zksync_client::{ZkSyncClient, ZkSyncConfig};
use zk_proofs::ZkProofProver;
use fuego_integration::{ZkSyncDualMiner, FuegoConfig};

pub mod config;

// Core modules
use block_sync::BlockSync;
use bridge::Bridge;
use consensus::Consensus;
use net_p2p::P2PNetwork;
use rpc::RPCServer;
use state_db::StateDB;
use txpool::TxPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub network: config::NetworkConfig,
    pub rpc: config::RPCConfig,
    pub bridge: config::BridgeConfig,
    pub consensus: config::ConsensusConfig,
    pub logging: config::LoggingConfig,
    pub zksync: config::ZkSyncConfig,
    pub fuego: config::FuegoConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            network: config::NetworkConfig::default(),
            rpc: config::RPCConfig::default(),
            bridge: config::BridgeConfig::default(),
            consensus: config::ConsensusConfig::default(),
            logging: config::LoggingConfig::default(),
            zksync: config::ZkSyncConfig::default(),
            fuego: config::FuegoConfig::default(),
        }
    }
}

pub struct CODL3ZkSyncNode {
    config: NodeConfig,
    shutdown_flag: Arc<AtomicBool>,
    
    // Core components
    state_db: Arc<StateDB>,
    txpool: Arc<TxPool>,
    block_sync: Arc<BlockSync>,
    consensus: Arc<RwLock<Consensus>>,
    bridge: Arc<Bridge>,
    p2p_network: Arc<P2PNetwork>,
    rpc_server: Arc<RPCServer>,
    
    // zkSync specific components
    zksync_client: Arc<ZkSyncClient>,
    zk_prover: Arc<ZkProofProver>,
    dual_miner: Arc<ZkSyncDualMiner>,
}

impl CODL3ZkSyncNode {
    pub async fn new(config: NodeConfig) -> Result<Self> {
        info!("Initializing CODL3 zkSync Hyperchains node");
        
        // Initialize shutdown flag
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        
        // Initialize core components
        let state_db = Arc::new(StateDB::new(&config.network.data_dir)?);
        let txpool = Arc::new(TxPool::new());
        let block_sync = Arc::new(BlockSync::new());
        let consensus = Arc::new(RwLock::new(Consensus::new()));
        let bridge = Arc::new(Bridge::new());
        let p2p_network = Arc::new(P2PNetwork::new(&config.network));
        let rpc_server = Arc::new(RPCServer::new(&config.rpc));
        
        // Initialize zkSync components
        let zksync_config = ZkSyncConfig {
            l1_rpc_url: config.zksync.l1_rpc_url.clone(),
            l2_rpc_url: config.zksync.l2_rpc_url.clone(),
            hyperchain_id: config.zksync.hyperchain_id,
            gas_token_address: config.zksync.gas_token_address,
            bridge_address: config.zksync.bridge_address,
            staking_address: config.zksync.staking_address,
            validator_address: config.zksync.validator_address,
        };
        
        let zksync_client = Arc::new(ZkSyncClient::new(zksync_config).await?);
        let zk_prover = Arc::new(ZkProofProver::new().await?);
        
        // Initialize Fuego integration
        let fuego_config = FuegoConfig {
            rpc_url: config.fuego.rpc_url.clone(),
            wallet_address: config.fuego.wallet_address.clone(),
            mining_threads: config.fuego.mining_threads,
            block_time: config.fuego.block_time,
        };
        
        let dual_miner = Arc::new(ZkSyncDualMiner::new(
            fuego_config,
            zksync_config,
        ).await?);
        
        Ok(Self {
            config,
            shutdown_flag,
            state_db,
            txpool,
            block_sync,
            consensus,
            bridge,
            p2p_network,
            rpc_server,
            zksync_client,
            zk_prover,
            dual_miner,
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        info!("Starting CODL3 zkSync Hyperchains node");
        
        // Start all components
        self.start_p2p_network().await?;
        self.start_rpc_server().await?;
        self.start_block_sync().await?;
        self.start_consensus().await?;
        self.start_bridge_monitoring().await?;
        self.start_dual_mining().await?;
        
        info!("CODL3 zkSync node started successfully");
        
        // Keep the node running
        self.wait_for_shutdown().await;
        
        Ok(())
    }
    
    async fn start_p2p_network(&self) -> Result<()> {
        info!("Starting P2P network");
        
        let shutdown_flag = self.shutdown_flag.clone();
        let p2p_network = self.p2p_network.clone();
        
        tokio::spawn(async move {
            if let Err(e) = p2p_network.start().await {
                error!("P2P network error: {}", e);
            }
            
            // Keep running until shutdown
            while !shutdown_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_rpc_server(&self) -> Result<()> {
        info!("Starting RPC server on {}", self.config.rpc.bind_address);
        
        let shutdown_flag = self.shutdown_flag.clone();
        let rpc_server = self.rpc_server.clone();
        
        tokio::spawn(async move {
            if let Err(e) = rpc_server.start().await {
                error!("RPC server error: {}", e);
            }
            
            // Keep running until shutdown
            while !shutdown_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_block_sync(&self) -> Result<()> {
        info!("Starting block synchronization");
        
        let shutdown_flag = self.shutdown_flag.clone();
        let block_sync = self.block_sync.clone();
        let state_db = self.state_db.clone();
        
        tokio::spawn(async move {
            loop {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                
                // Sync blocks from Fuego
                if let Err(e) = block_sync.sync_blocks(&state_db).await {
                    error!("Block sync error: {}", e);
                }
                
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_consensus(&self) -> Result<()> {
        info!("Starting consensus mechanism");
        
        let shutdown_flag = self.shutdown_flag.clone();
        let consensus = self.consensus.clone();
        
        tokio::spawn(async move {
            loop {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                
                // Run consensus
                {
                    let mut consensus_guard = consensus.write().await;
                    if let Err(e) = consensus_guard.run_consensus().await {
                        error!("Consensus error: {}", e);
                    }
                }
                
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_bridge_monitoring(&self) -> Result<()> {
        info!("Starting bridge monitoring");
        
        let shutdown_flag = self.shutdown_flag.clone();
        let bridge = self.bridge.clone();
        let zksync_client = self.zksync_client.clone();
        
        tokio::spawn(async move {
            loop {
                if shutdown_flag.load(Ordering::SeqCst) {
                    break;
                }
                
                // Monitor bridge events
                if let Err(e) = bridge.monitor_events(&zksync_client).await {
                    error!("Bridge monitoring error: {}", e);
                }
                
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });
        
        Ok(())
    }
    
    async fn start_dual_mining(&self) -> Result<()> {
        info!("Starting dual mining (Fuego + CODL3)");
        
        let shutdown_flag = self.shutdown_flag.clone();
        let dual_miner = self.dual_miner.clone();
        
        tokio::spawn(async move {
            if let Err(e) = dual_miner.start_dual_mining().await {
                error!("Dual mining error: {}", e);
            }
            
            // Keep running until shutdown
            while !shutdown_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
        
        Ok(())
    }
    
    async fn wait_for_shutdown(&self) {
        info!("Waiting for shutdown signal");
        
        // Wait for shutdown flag
        while !self.shutdown_flag.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        
        info!("Shutdown signal received");
    }
    
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping CODL3 zkSync node");
        
        // Set shutdown flag
        self.shutdown_flag.store(true, Ordering::SeqCst);
        
        // Stop dual mining
        self.dual_miner.stop_mining().await?;
        
        info!("CODL3 zkSync node stopped");
        Ok(())
    }
    
    // Getter methods for components
    pub fn get_zksync_client(&self) -> Arc<ZkSyncClient> {
        self.zksync_client.clone()
    }
    
    pub fn get_zk_prover(&self) -> Arc<ZkProofProver> {
        self.zk_prover.clone()
    }
    
    pub fn get_dual_miner(&self) -> Arc<ZkSyncDualMiner> {
        self.dual_miner.clone()
    }
    
    pub fn get_state_db(&self) -> Arc<StateDB> {
        self.state_db.clone()
    }
    
    pub fn get_txpool(&self) -> Arc<TxPool> {
        self.txpool.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_node_creation() {
        let config = NodeConfig::default();
        let node = CODL3ZkSyncNode::new(config).await;
        assert!(node.is_ok());
    }
}
