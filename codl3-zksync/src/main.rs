use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use axum::{
    routing::{get, post},
    http::{StatusCode, HeaderValue},
    response::Json,
    extract::{State, Path},
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};
use libp2p::{
    core::{upgrade, transport, identity},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, noise, yamux, kad, mdns,
    PeerId, Multiaddr,
};
use futures::StreamExt;
use serde_json::Value;
use uuid::Uuid;

// Core data structures for the CODL3 zkSync node

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkProof {
    pub proof_type: String,
    pub proof_data: Vec<u8>,
    pub public_inputs: Vec<String>,
    pub verification_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoBlock {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub transactions: Vec<FuegoTransaction>,
    pub fees: BlockFees,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoTransaction {
    pub txid: String,
    pub amount: u64,
    pub fee: u64,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockFees {
    pub transaction_fees: u64,
    pub deposit_fees: u64,
    pub burn_fees: u64,
    pub total_fees: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningRewards {
    pub fuego_rewards: u64,
    pub codl3_gas_fees: u64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiningStats {
    pub fuego_blocks_mined: u64,
    pub codl3_blocks_mined: u64,
    pub total_rewards: u64,
    pub uptime_seconds: u64,
}

// Network-related structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub address: String,
    pub last_seen: u64,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub message_id: String,
    pub message_type: String,
    pub payload: Value,
    pub timestamp: u64,
    pub sender: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSyncRequest {
    pub from_height: u64,
    pub to_height: u64,
    pub requester: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSyncResponse {
    pub blocks: Vec<Block>,
    pub total_blocks: u64,
    pub response_id: String,
}

// Configuration structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub network: NetworkConfig,
    pub rpc: RPCConfig,
    pub bridge: BridgeConfig,
    pub consensus: ConsensusConfig,
    pub logging: LoggingConfig,
    pub zksync: ZkSyncConfig,
    pub fuego: FuegoConfig,
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
pub struct BridgeConfig {
    pub enabled: bool,
    pub poll_interval: u64,
    pub l1_confirmations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub validator_count: u32,
    pub block_time: u64,
    pub finality_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkSyncConfig {
    pub l1_rpc_url: String,
    pub l2_rpc_url: String,
    pub hyperchain_id: u64,
    pub gas_token_address: String,
    pub bridge_address: String,
    pub staking_address: String,
    pub validator_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoConfig {
    pub rpc_url: String,
    pub wallet_address: String,
    pub mining_threads: u32,
    pub block_time: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            p2p_port: 30333,
            listen_addr: "0.0.0.0".to_string(),
            bootstrap_peers: vec![
                "/ip4/127.0.0.1/tcp/30334/p2p/QmBootstrap1".to_string(),
                "/ip4/127.0.0.1/tcp/30335/p2p/QmBootstrap2".to_string(),
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

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval: 30,
            l1_confirmations: 12,
        }
    }
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            validator_count: 21,
            block_time: 12,
            finality_threshold: 15,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
}

impl Default for ZkSyncConfig {
    fn default() -> Self {
        Self {
            l1_rpc_url: "http://localhost:8545".to_string(),
            l2_rpc_url: "http://localhost:3050".to_string(),
            hyperchain_id: 324,
            gas_token_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            bridge_address: "0xabcdef1234567890abcdef1234567890abcdef1234".to_string(),
            staking_address: "0x1122334455667788990011223344556677889900".to_string(),
            validator_address: "0x2233445566778899001122334455667788990011".to_string(),
        }
    }
}

impl Default for FuegoConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8080".to_string(),
            wallet_address: "0x333344556677889900112233445566778899001122".to_string(),
            mining_threads: 4,
            block_time: 12,
        }
    }
}

// P2P Network Behaviour

#[derive(NetworkBehaviour)]
struct CODL3Behaviour {
    kad: kad::Kademlia<kad::store::MemoryStore>,
    mdns: mdns::Mdns,
    #[behaviour(ignore)]
    events: mpsc::UnboundedSender<NetworkEvent>,
}

#[derive(Debug)]
enum NetworkEvent {
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    MessageReceived(PeerId, Vec<u8>),
}

// Main node implementation

pub struct CODL3ZkSyncNode {
    config: NodeConfig,
    running: bool,
    state: Arc<Mutex<NodeState>>,
    pending_transactions: Arc<Mutex<HashMap<String, Transaction>>>,
    latest_blocks: Arc<Mutex<Vec<Block>>>,
    mining_rewards: Arc<Mutex<MiningRewards>>,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    swarm: Option<Swarm<CODL3Behaviour>>,
    rpc_server: Option<axum::Server<axum::Router, axum::serve::AddrIncoming>>,
    start_time: Instant,
}

impl CODL3ZkSyncNode {
    pub fn new(config: NodeConfig) -> Self {
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
                    codl3_blocks_mined: 0,
                    total_rewards: 0,
                    uptime_seconds: 0,
                },
            })),
            pending_transactions: Arc::new(Mutex::new(HashMap::new())),
            latest_blocks: Arc::new(Mutex::new(Vec::new())),
            mining_rewards: Arc::new(Mutex::new(MiningRewards {
                fuego_rewards: 0,
                codl3_gas_fees: 0,
                eldernode_fees: 0,
                total_rewards: 0,
            })),
            peers: Arc::new(Mutex::new(HashMap::new())),
            swarm: None,
            rpc_server: None,
            start_time: Instant::now(),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting CODL3 zkSync Node...");

        self.running = true;

        // Initialize logging
        self.init_logging()?;

        // Start various components
        self.start_p2p_network().await?;
        self.start_rpc_server().await?;
        self.start_block_sync().await?;
        self.start_consensus().await?;
        self.start_bridge_monitoring().await?;
        self.start_dual_mining().await?;

        // Main event loop
        self.run_event_loop().await?;

        info!("CODL3 zkSync Node stopped");
        Ok(())
    }

    fn init_logging(&self) -> Result<()> {
        let level = self.config.logging.level.parse::<tracing::Level>()
            .unwrap_or(tracing::Level::INFO);

        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                format!("codl3_zksync={}", level)
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();

        info!("Logging initialized at level: {}", level);
        Ok(())
    }

    async fn start_p2p_network(&mut self) -> Result<()> {
        info!("Starting P2P network on port {}", self.config.network.p2p_port);

        // Create a random PeerId
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer ID: {}", local_peer_id);

        // Create a transport
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&local_key)
            .expect("Signing libp2p-noise static DH keypair failed.");

        let transport = transport::TokioTcpConfig::new()
            .nodelay(true)
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(yamux::YamuxConfig::default())
            .boxed();

        // Create a Swarm to manage peers and events
        let mut swarm = {
            let mut behaviour = CODL3Behaviour {
                kad: kad::Kademlia::new(PeerId::from(local_key.public()), kad::store::MemoryStore::new(local_peer_id)),
                mdns: mdns::Mdns::new(Default::default()).await?,
                events: mpsc::unbounded_channel().0,
            };

            // Add bootstrap peers
            for peer_addr in &self.config.network.bootstrap_peers {
                if let Ok(addr) = peer_addr.parse::<Multiaddr>() {
                    if let Ok((peer_id, _)) = addr.extract_peer_id() {
                        behaviour.kad.add_address(&peer_id, addr);
                        info!("Added bootstrap peer: {}", peer_id);
                    }
                }
            }

            Swarm::new(transport, behaviour, local_peer_id)
        };

        // Listen on all interfaces and whatever port the OS assigns
        let addr = format!("/ip4/{}/tcp/{}", self.config.network.listen_addr, self.config.network.p2p_port);
        let addr: Multiaddr = addr.parse()?;
        swarm.listen_on(addr)?;

        self.swarm = Some(swarm);
        info!("P2P network started successfully");
        Ok(())
    }

    async fn start_rpc_server(&mut self) -> Result<()> {
        info!("Starting RPC server on {}:{}", self.config.rpc.host, self.config.rpc.port);

        // Create CORS layer
        let cors = CorsLayer::new()
            .allow_methods([Method::GET, Method::POST])
            .allow_origin(Any)
            .allow_headers(Any);

        // Create the application state
        let app_state = AppState {
            node_state: self.state.clone(),
            mining_rewards: self.mining_rewards.clone(),
            peers: self.peers.clone(),
            pending_transactions: self.pending_transactions.clone(),
            latest_blocks: self.latest_blocks.clone(),
        };

        // Build the application
        let app = axum::Router::new()
            .route("/", get(root))
            .route("/health", get(health))
            .route("/status", get(status))
            .route("/peers", get(get_peers))
            .route("/transactions", get(get_transactions))
            .route("/transactions", post(add_transaction))
            .route("/blocks", get(get_blocks))
            .route("/blocks/:height", get(get_block_by_height))
            .route("/mining/rewards", get(get_mining_rewards))
            .route("/mining/stats", get(get_mining_stats))
            .route("/network/info", get(get_network_info))
            .with_state(app_state)
            .layer(cors);

        // Start the server
        let addr = format!("{}:{}", self.config.rpc.host, self.config.rpc.port);
        let addr: SocketAddr = addr.parse()?;
        
        let server = axum::Server::bind(&addr)
            .serve(app.into_make_service());

        info!("RPC server started on {}", addr);
        self.rpc_server = Some(server);
        Ok(())
    }

    async fn start_block_sync(&self) -> Result<()> {
        info!("Starting block synchronization");
        
        // Start background task for block synchronization
        let state = self.state.clone();
        let peers = self.peers.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                // Check for new blocks from peers
                let peer_count = {
                    let peers_guard = peers.lock().unwrap();
                    peers_guard.len() as u32
                };
                
                if peer_count > 0 {
                    debug!("Checking for new blocks from {} peers", peer_count);
                    // TODO: Implement actual block sync logic
                }
            }
        });

        info!("Block synchronization started");
        Ok(())
    }

    async fn start_consensus(&self) -> Result<()> {
        info!("Starting consensus mechanism");
        
        // Start background task for consensus
        let state = self.state.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(self.config.consensus.block_time));
            loop {
                interval.tick().await;
                
                // Consensus logic here
                let mut state_guard = state.lock().unwrap();
                state_guard.uptime_seconds = state_guard.start_time.elapsed().as_secs();
            }
        });

        info!("Consensus mechanism started");
        Ok(())
    }

    async fn start_bridge_monitoring(&self) -> Result<()> {
        info!("Starting bridge monitoring");
        
        if !self.config.bridge.enabled {
            info!("Bridge monitoring disabled");
            return Ok(());
        }

        // Start background task for bridge monitoring
        let bridge_config = self.config.bridge.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(bridge_config.poll_interval));
            loop {
                interval.tick().await;
                
                // Bridge monitoring logic here
                debug!("Checking bridge status...");
                // TODO: Implement actual bridge monitoring
            }
        });

        info!("Bridge monitoring started");
        Ok(())
    }

    async fn start_dual_mining(&self) -> Result<()> {
        info!("Starting dual mining (Fuego + CODL3)");
        
        // Start background task for dual mining
        let mining_rewards = self.mining_rewards.clone();
        let state = self.state.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                
                // Mining logic here
                debug!("Processing mining rewards...");
                // TODO: Implement actual mining logic
            }
        });

        info!("Dual mining started");
        Ok(())
    }

    async fn run_event_loop(&mut self) -> Result<()> {
        info!("Starting main event loop");
        
        let mut swarm_events = if let Some(swarm) = &mut self.swarm {
            Box::pin(swarm.next())
        } else {
            return Err(anyhow::anyhow!("Swarm not initialized"));
        };

        let mut rpc_server = if let Some(server) = &mut self.rpc_server {
            Box::pin(server)
        } else {
            return Err(anyhow::anyhow!("RPC server not initialized"));
        };

        loop {
            tokio::select! {
                swarm_event = swarm_events.next() => {
                    if let Some(event) = swarm_event {
                        self.handle_swarm_event(event).await?;
                    }
                }
                _ = &mut rpc_server => {
                    info!("RPC server stopped");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<CODL3BehaviourEvent, impl std::error::Error>) -> Result<()> {
        match event {
            SwarmEvent::Behaviour(CODL3BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, _) in list {
                    info!("mDNS discovered peer: {}", peer_id);
                    self.add_peer(peer_id).await?;
                }
            }
            SwarmEvent::Behaviour(mdns::Event::Expired(list)) => {
                for (peer_id, _) in list {
                    info!("mDNS expired peer: {}", peer_id);
                    self.remove_peer(peer_id).await?;
                }
            }
            SwarmEvent::Behaviour(CODL3BehaviourEvent::Kad(kad::Event::OutboundQueryCompleted { result, .. })) => {
                match result {
                    kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { peers, .. })) => {
                        info!("Bootstrap completed with {} peers", peers.len());
                        for (peer_id, _) in peers {
                            self.add_peer(peer_id).await?;
                        }
                    }
                    kad::QueryResult::Bootstrap(Err(e)) => {
                        warn!("Bootstrap failed: {}", e);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn add_peer(&mut self, peer_id: PeerId) -> Result<()> {
        let peer_info = PeerInfo {
            peer_id: peer_id.to_string(),
            address: "".to_string(), // Will be filled when we get more info
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: "1.0.0".to_string(),
        };

        {
            let mut peers_guard = self.peers.lock().unwrap();
            peers_guard.insert(peer_id.to_string(), peer_info);
        }

        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.connected_peers = self.peers.lock().unwrap().len() as u32;
        }

        info!("Added peer: {} (total: {})", peer_id, self.peers.lock().unwrap().len());
        Ok(())
    }

    async fn remove_peer(&mut self, peer_id: PeerId) -> Result<()> {
        {
            let mut peers_guard = self.peers.lock().unwrap();
            peers_guard.remove(&peer_id.to_string());
        }

        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.connected_peers = self.peers.lock().unwrap().len() as u32;
        }

        info!("Removed peer: {} (total: {})", peer_id, self.peers.lock().unwrap().len());
        Ok(())
    }

    // Basic utility methods
    pub fn get_state(&self) -> NodeState {
        self.state.lock().unwrap().clone()
    }

    pub fn get_mining_rewards(&self) -> MiningRewards {
        self.mining_rewards.lock().unwrap().clone()
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

    pub async fn create_block(&self, transactions: Vec<Transaction>) -> Result<Block> {
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
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                merkle_root: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                validator: self.config.zksync.validator_address.clone(),
                gas_used: 0,
                gas_limit: 30000000,
            },
            transactions,
            zk_proof: None,
        };
        
        info!("Created block at height {}", block.header.height);
        
        // Update state
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.current_height = block.header.height;
            state_guard.latest_block_hash = format!("0x{:x}", Uuid::new_v4());
        }
        
        // Add to latest blocks
        {
            let mut blocks_guard = self.latest_blocks.lock().unwrap();
            blocks_guard.push(block.clone());
            if blocks_guard.len() > 100 {
                blocks_guard.remove(0);
            }
        }
        
        Ok(block)
    }

    pub async fn process_fuego_block(&self, fuego_block: FuegoBlock) -> Result<()> {
        info!("Processing Fuego block at height {}", fuego_block.height);
        
        // Update mining rewards
        {
            let mut rewards_guard = self.mining_rewards.lock().unwrap();
            rewards_guard.fuego_rewards += fuego_block.fees.total_fees;
            rewards_guard.total_rewards += fuego_block.fees.total_fees;
        }
        
        {
            let mut state_guard = self.state.lock().unwrap();
            state_guard.mining_stats.fuego_blocks_mined += 1;
        }
        
        info!("Updated mining rewards: {} total", self.mining_rewards.lock().unwrap().total_rewards);
        Ok(())
    }

    pub async fn generate_zk_proof(&self, block: &Block) -> Result<ZkProof> {
        info!("Generating ZK proof for block {}", block.header.height);
        
        // Simulate ZK proof generation time
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let proof = ZkProof {
            proof_type: "STARK".to_string(),
            proof_data: vec![0u8; 1024], // Placeholder proof data
            public_inputs: vec![
                block.header.height.to_string(),
                block.header.merkle_root.clone(),
            ],
            verification_key: vec![0u8; 512], // Placeholder verification key
        };
        
        info!("Generated ZK proof for block {}", block.header.height);
        Ok(proof)
    }
}

// RPC Server State and Handlers

#[derive(Clone)]
struct AppState {
    node_state: Arc<Mutex<NodeState>>,
    mining_rewards: Arc<Mutex<MiningRewards>>,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    pending_transactions: Arc<Mutex<HashMap<String, Transaction>>>,
    latest_blocks: Arc<Mutex<Vec<Block>>>,
}

async fn root() -> &'static str {
    "CODL3 zkSync Node RPC Server"
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn status(State(state): State<AppState>) -> Json<NodeState> {
    let node_state = state.node_state.lock().unwrap().clone();
    Json(node_state)
}

async fn get_peers(State(state): State<AppState>) -> Json<Vec<PeerInfo>> {
    let peers = state.peers.lock().unwrap();
    let peers_vec: Vec<PeerInfo> = peers.values().cloned().collect();
    Json(peers_vec)
}

async fn get_transactions(State(state): State<AppState>) -> Json<Vec<Transaction>> {
    let transactions = state.pending_transactions.lock().unwrap();
    let transactions_vec: Vec<Transaction> = transactions.values().cloned().collect();
    Json(transactions_vec)
}

async fn add_transaction(
    State(state): State<AppState>,
    Json(transaction): Json<Transaction>,
) -> StatusCode {
    let mut transactions = state.pending_transactions.lock().unwrap();
    transactions.insert(transaction.hash.clone(), transaction);
    StatusCode::CREATED
}

async fn get_blocks(State(state): State<AppState>) -> Json<Vec<Block>> {
    let blocks = state.latest_blocks.lock().unwrap();
    let blocks_vec: Vec<Block> = blocks.clone();
    Json(blocks_vec)
}

async fn get_block_by_height(
    State(state): State<AppState>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, StatusCode> {
    let blocks = state.latest_blocks.lock().unwrap();
    if let Some(block) = blocks.iter().find(|b| b.header.height == height) {
        Ok(Json(block.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_mining_rewards(State(state): State<AppState>) -> Json<MiningRewards> {
    let rewards = state.mining_rewards.lock().unwrap().clone();
    Json(rewards)
}

async fn get_mining_stats(State(state): State<AppState>) -> Json<MiningStats> {
    let node_state = state.node_state.lock().unwrap();
    Json(node_state.mining_stats.clone())
}

async fn get_network_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let peers = state.peers.lock().unwrap();
    let node_state = state.node_state.lock().unwrap();
    
    let info = serde_json::json!({
        "total_peers": peers.len(),
        "connected_peers": node_state.connected_peers,
        "pending_transactions": node_state.pending_transactions,
        "current_height": node_state.current_height,
        "uptime_seconds": node_state.mining_stats.uptime_seconds,
    });
    
    Json(info)
}

// CLI structure

#[derive(Parser)]
#[command(name = "codl3-zksync")]
#[command(about = "CODL3 zkSync Hyperchains Node")]
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
    #[arg(long, default_value = "http://localhost:8080")]
    fuego_rpc_url: String,
    #[arg(long, default_value = "30333")]
    p2p_port: u16,
    #[arg(long, default_value = "9944")]
    rpc_port: u16,
    #[arg(long)]
    validator_address: Option<String>,
    #[arg(long)]
    gas_token_address: Option<String>,
}

// Utility functions

fn create_node_config(cli: &Cli) -> NodeConfig {
    NodeConfig {
        network: NetworkConfig {
            data_dir: cli.data_dir.clone(),
            p2p_port: cli.p2p_port,
            listen_addr: "0.0.0.0".to_string(),
            bootstrap_peers: vec![
                "/ip4/127.0.0.1/tcp/30334/p2p/QmBootstrap1".to_string(),
                "/ip4/127.0.0.1/tcp/30335/p2p/QmBootstrap2".to_string(),
            ],
            max_peers: 50,
        },
        rpc: RPCConfig {
            port: cli.rpc_port,
            host: "127.0.0.1".to_string(),
            cors_origins: vec!["*".to_string()],
            max_connections: 100,
        },
        bridge: BridgeConfig::default(),
        consensus: ConsensusConfig::default(),
        logging: LoggingConfig {
            level: cli.log_level.clone(),
            format: "json".to_string(),
        },
        zksync: ZkSyncConfig {
            l1_rpc_url: cli.l1_rpc_url.clone(),
            l2_rpc_url: cli.l2_rpc_url.clone(),
            hyperchain_id: 324,
            gas_token_address: cli.gas_token_address.clone().unwrap_or_else(|| "0x1234567890abcdef1234567890abcdef12345678".to_string()),
            bridge_address: "0xabcdef1234567890abcdef1234567890abcdef1234".to_string(),
            staking_address: "0x1122334455667788990011223344556677889900".to_string(),
            validator_address: cli.validator_address.clone().unwrap_or_else(|| "0x2233445566778899001122334455667788990011".to_string()),
        },
        fuego: FuegoConfig {
            rpc_url: cli.fuego_rpc_url.clone(),
            wallet_address: "0x333344556677889900112233445566778899001122".to_string(),
            mining_threads: 4,
            block_time: 12,
        },
    }
}

async fn test_node_functionality(node: &CODL3ZkSyncNode) -> Result<()> {
    info!("=== Testing CODL3 zkSync Node Functionality ===");
    
    // Test adding a transaction
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
    };
    
    node.add_transaction(test_tx).await?;
    
    // Test creating a block
    let transactions = {
        let tx_guard = node.pending_transactions.lock().unwrap();
        tx_guard.values().cloned().collect()
    };
    let block = node.create_block(transactions).await?;
    
    // Test generating ZK proof
    let proof = node.generate_zk_proof(&block).await?;
    info!("Generated ZK proof type: {}", proof.proof_type);
    
    // Test processing a Fuego block
    let fuego_block = FuegoBlock {
        height: 12345,
        hash: "0xfuego1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        transactions: vec![],
        fees: BlockFees {
            transaction_fees: 50000,
            deposit_fees: 10000,
            burn_fees: 5000,
            total_fees: 65000,
        },
    };
    
    node.process_fuego_block(fuego_block).await?;
    
    // Display node state
    let state = node.get_state();
    let rewards = node.get_mining_rewards();
    
    info!("=== CODL3 zkSync Node State ===");
    info!("Current height: {}", state.current_height);
    info!("Connected peers: {}", state.connected_peers);
    info!("Pending transactions: {}", state.pending_transactions);
    info!("Fuego blocks mined: {}", state.mining_stats.fuego_blocks_mined);
    info!("Total rewards: {}", rewards.total_rewards);
    
    info!("=== Test Completed Successfully ===");
    Ok(())
}

// Main function

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    let config = create_node_config(&cli);
    let mut node = CODL3ZkSyncNode::new(config);

    info!("CODL3 zkSync Node starting...");
    info!("Log level: {}", cli.log_level);
    info!("Data directory: {}", cli.data_dir);
    info!("L1 RPC URL: {}", cli.l1_rpc_url);
    info!("L2 RPC URL: {}", cli.l2_rpc_url);
    info!("Fuego RPC URL: {}", cli.fuego_rpc_url);
    info!("P2P port: {}", cli.p2p_port);
    info!("RPC port: {}", cli.rpc_port);
    
    if let Some(addr) = &cli.validator_address {
        info!("Validator address: {}", addr);
    }
    
    if let Some(token) = &cli.gas_token_address {
        info!("Gas token address: {}", token);
    }

    // Test the node functionality
    if let Err(e) = test_node_functionality(&node).await {
        error!("Node functionality test failed: {}", e);
        process::exit(1);
    }

    // Start the node
    if let Err(e) = node.start().await {
        error!("Node failed to start: {}", e);
        process::exit(1);
    }

    Ok(())
}
