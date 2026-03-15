// Fuego L1 RPC Client for C0DL3 Rollup
// Async client that queries a running Fuego testnet/mainnet node
// for commitment data, merkle proofs, Elderfier stake info, and block data.
//
// Uses the same RPC endpoints and response types as xfg-stark/src/fuego_rpc.rs
// but with async reqwest instead of blocking.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn, debug, error};

// ──────────────────────────────────────────────
// Network configuration
// ──────────────────────────────────────────────

/// Default Fuego daemon RPC port (mainnet)
pub const DEFAULT_RPC_PORT: u16 = 18180;
/// Default Fuego daemon RPC port (testnet)
pub const DEFAULT_TESTNET_RPC_PORT: u16 = 28280;

/// Testnet seed nodes (from CryptoNoteConfig.h)
pub const TESTNET_SEED_RPC_NODES: &[(&str, u16)] = &[
    ("103.101.201.136", 28280),
    ("216.145.84.248", 28280),
    ("80.89.228.157", 28280),
    ("207.244.247.64", 28280),
    ("216.145.66.224", 28280),
];

/// Mainnet seed nodes
pub const MAINNET_SEED_RPC_NODES: &[(&str, u16)] = &[
    ("3.16.217.33", 18180),
    ("80.89.228.157", 18180),
    ("207.244.247.64", 18180),
    ("216.145.66.224", 18180),
];

/// Which Fuego network to target
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FuegoNetwork {
    Mainnet,
    Testnet,
}

// ──────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoL1Config {
    /// Fuego RPC URL (e.g., http://127.0.0.1:28280)
    pub rpc_url: String,
    /// Network type
    pub network: FuegoNetwork,
    /// How often to poll L1 for new data (seconds)
    pub poll_interval_secs: u64,
}

impl Default for FuegoL1Config {
    fn default() -> Self {
        Self {
            rpc_url: format!("http://127.0.0.1:{}", DEFAULT_TESTNET_RPC_PORT),
            network: FuegoNetwork::Testnet,
            poll_interval_secs: 30,
        }
    }
}

// ──────────────────────────────────────────────
// RPC Response types (mirrors xfg-stark/fuego_rpc.rs)
// ──────────────────────────────────────────────

/// Generic JSON-RPC response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: Option<String>,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Response from /get_commitment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentResponse {
    /// Whether the commitment was found
    pub found: bool,
    /// Hex commitment hash
    #[serde(default)]
    pub commitment_hash: String,
    /// Hex transaction hash
    #[serde(default)]
    pub tx_hash: String,
    /// Block height of the commitment
    #[serde(default)]
    pub block_height: u32,
    /// Amount in atomic units
    #[serde(default)]
    pub amount: u64,
    /// Deposit term (0xFFFFFFFF for HEAT, blocks for COLD)
    #[serde(default)]
    pub term: u32,
    /// Type: 0=HEAT, 1=YIELD/COLD, 2=ELDERFIER_STAKING
    #[serde(default, rename = "type")]
    pub commitment_type: u8,
    /// Target chain ID
    #[serde(default)]
    pub target_chain_id: u32,
    /// Leaf index in merkle tree
    #[serde(default)]
    pub leaf_index: u32,
    /// RPC status
    #[serde(default)]
    pub status: String,
}

/// Response from /get_commitment_merkle_root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleRootResponse {
    /// Current merkle root hash (hex)
    pub merkle_root: String,
    /// Total number of leaves
    pub total_leaves: u64,
    /// Block height when this root was computed
    pub block_height: u32,
    /// RPC status
    #[serde(default)]
    pub status: String,
}

/// Response from /get_commitment_merkle_proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProofResponse {
    /// Whether the proof was generated
    pub found: bool,
    /// Commitment hash this proof is for
    #[serde(default)]
    pub commitment_hash: String,
    /// Merkle proof path (hex hashes)
    #[serde(default)]
    pub proof: Vec<String>,
    /// Leaf index
    #[serde(default)]
    pub leaf_index: u32,
    /// Merkle root this proof verifies against
    #[serde(default)]
    pub merkle_root: String,
    /// RPC status
    #[serde(default)]
    pub status: String,
}

/// Response from /get_commitment_stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitmentStatsResponse {
    /// Total number of commitments indexed
    pub total_commitments: u64,
    /// Number of HEAT burn commitments
    pub heat_commitments: u64,
    /// Number of COLD deposit commitments
    pub cold_commitments: u64,
    /// Highest block with commitments
    pub highest_block: u32,
    /// Current merkle root (hex)
    pub merkle_root: String,
    /// Elderfier consensus percentage
    #[serde(default)]
    pub consensus_percentage: u64,
    /// Signed Elderfier IDs
    #[serde(default)]
    pub signed_elderfier_ids: Vec<u8>,
    /// RPC status
    #[serde(default)]
    pub status: String,
}

/// Elderfier stake information queried from Fuego L1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElderfierStakeInfo {
    /// Fuego address
    pub address: String,
    /// Number of 0xEF deposits found
    pub deposit_count: u32,
    /// Total staked amount in atomic units
    pub total_stake: u64,
    /// Individual deposit commitment hashes
    pub deposit_commitments: Vec<String>,
    /// Whether this address qualifies as Elderfier (>= 5 deposits, >= 4000 XFG)
    pub is_qualified: bool,
    /// Elderfier status if registered
    pub status: Option<ElderfierStatus>,
}

/// Elderfier registration status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ElderfierStatus {
    Void,
    Active,
    Unstaking,
}

// ──────────────────────────────────────────────
// Fuego L1 Client
// ──────────────────────────────────────────────

pub struct FuegoL1Client {
    /// Base URL for RPC calls
    rpc_url: String,
    /// Async HTTP client
    client: reqwest::Client,
    /// Network type
    network: FuegoNetwork,
    /// Whether connected (last check succeeded)
    connected: std::sync::atomic::AtomicBool,
}

impl FuegoL1Client {
    /// Create a new Fuego L1 client
    pub fn new(config: &FuegoL1Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            rpc_url: config.rpc_url.clone(),
            client,
            network: config.network,
            connected: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Check if connected to Fuego node
    pub fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get the RPC URL
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Get the network type
    pub fn network(&self) -> FuegoNetwork {
        self.network
    }

    /// Test connection to Fuego node
    pub async fn check_connection(&self) -> Result<bool> {
        match self.get_block_height().await {
            Ok(height) => {
                self.connected.store(true, std::sync::atomic::Ordering::Relaxed);
                info!("Connected to Fuego {} at {} (height: {})",
                    match self.network {
                        FuegoNetwork::Testnet => "testnet",
                        FuegoNetwork::Mainnet => "mainnet",
                    },
                    self.rpc_url,
                    height
                );
                Ok(true)
            }
            Err(e) => {
                self.connected.store(false, std::sync::atomic::Ordering::Relaxed);
                warn!("Cannot connect to Fuego node at {}: {}", self.rpc_url, e);
                Ok(false)
            }
        }
    }

    /// Try connecting to seed nodes if primary connection fails
    pub async fn try_seed_nodes(&mut self) -> Result<bool> {
        let seeds = match self.network {
            FuegoNetwork::Testnet => TESTNET_SEED_RPC_NODES,
            FuegoNetwork::Mainnet => MAINNET_SEED_RPC_NODES,
        };

        for (ip, port) in seeds {
            let url = format!("http://{}:{}", ip, port);
            info!("Trying Fuego seed node: {}", url);

            let test_client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?;

            let payload = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "getblockcount",
                "params": [],
                "id": 1
            });

            match test_client.post(&url).json(&payload).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("Connected to Fuego seed node: {}", url);
                        self.rpc_url = url;
                        self.connected.store(true, std::sync::atomic::Ordering::Relaxed);
                        return Ok(true);
                    }
                }
                Err(_) => continue,
            }
        }

        warn!("Could not connect to any Fuego seed nodes");
        Ok(false)
    }

    // ──────────────────────────────────────────
    // Core RPC methods
    // ──────────────────────────────────────────

    /// Get current block height from Fuego
    pub async fn get_block_height(&self) -> Result<u64> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "getblockcount",
            "params": [],
            "id": 1
        });

        let response: JsonRpcResponse = self.client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        if let Some(error) = response.error {
            return Err(anyhow!("Fuego RPC error: {}", error.message));
        }

        response.result
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("Invalid block height response"))
    }

    /// Query a commitment by hash from the CommitmentIndex
    pub async fn get_commitment(&self, commitment_hash: &str) -> Result<CommitmentResponse> {
        let url = format!("{}/get_commitment", self.rpc_url);
        let payload = serde_json::json!({
            "commitment_hash": commitment_hash
        });

        let response: CommitmentResponse = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Get current commitment merkle root
    pub async fn get_commitment_merkle_root(&self) -> Result<MerkleRootResponse> {
        let url = format!("{}/get_commitment_merkle_root", self.rpc_url);

        let response: MerkleRootResponse = self.client
            .post(&url)
            .json(&serde_json::json!({}))
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Get merkle proof for a commitment
    pub async fn get_commitment_merkle_proof(&self, commitment_hash: &str) -> Result<MerkleProofResponse> {
        let url = format!("{}/get_commitment_merkle_proof", self.rpc_url);
        let payload = serde_json::json!({
            "commitment_hash": commitment_hash
        });

        let response: MerkleProofResponse = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Get commitment statistics
    pub async fn get_commitment_stats(&self) -> Result<CommitmentStatsResponse> {
        let url = format!("{}/get_commitment_stats", self.rpc_url);

        let response: CommitmentStatsResponse = self.client
            .post(&url)
            .json(&serde_json::json!({}))
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    /// Check Elderfier stake for an address
    /// Queries all 0xEF deposits for the given address
    pub async fn check_elderfier_stake(&self, address: &str) -> Result<ElderfierStakeInfo> {
        // Query commitment stats first to check if the index is available
        let stats = self.get_commitment_stats().await;

        // For testnet, if the Fuego node doesn't support the query,
        // return a mock response that allows validator registration
        match stats {
            Ok(s) => {
                debug!("CommitmentIndex has {} total commitments", s.total_commitments);
            }
            Err(e) => {
                warn!("Cannot query CommitmentIndex (Fuego node may not support it yet): {}", e);
                // Return unqualified stake info when we can't verify
                return Ok(ElderfierStakeInfo {
                    address: address.to_string(),
                    deposit_count: 0,
                    total_stake: 0,
                    deposit_commitments: vec![],
                    is_qualified: false,
                    status: None,
                });
            }
        }

        // TODO: Query specific 0xEF deposits for this address
        // The CommitmentIndex RPC doesn't currently support querying by address,
        // only by commitment hash. For testnet, we'll verify individual commitments
        // provided by the validator in their stake proof.
        Ok(ElderfierStakeInfo {
            address: address.to_string(),
            deposit_count: 0,
            total_stake: 0,
            deposit_commitments: vec![],
            is_qualified: false,
            status: None,
        })
    }

    /// Verify a specific commitment exists and is of type ELDERFIER_STAKING
    pub async fn verify_elderfier_commitment(&self, commitment_hash: &str) -> Result<bool> {
        let commitment = self.get_commitment(commitment_hash).await?;

        if !commitment.found {
            return Ok(false);
        }

        // Type 2 = ELDERFIER_STAKING
        if commitment.commitment_type != 2 {
            debug!("Commitment {} is type {} (expected 2 for ELDERFIER_STAKING)",
                commitment_hash, commitment.commitment_type);
            return Ok(false);
        }

        // Verify minimum stake amount (800 XFG = 8_000_000_000 atomic units per deposit)
        const ELDERFIER_MIN_STAKE_PER_DEPOSIT: u64 = 8_000_000_000;
        if commitment.amount < ELDERFIER_MIN_STAKE_PER_DEPOSIT {
            debug!("Commitment {} has amount {} (minimum {})",
                commitment_hash, commitment.amount, ELDERFIER_MIN_STAKE_PER_DEPOSIT);
            return Ok(false);
        }

        Ok(true)
    }

    /// Get connection status as JSON for RPC endpoint
    pub async fn get_status(&self) -> serde_json::Value {
        let height = self.get_block_height().await.unwrap_or(0);
        let stats = self.get_commitment_stats().await.ok();

        serde_json::json!({
            "connected": self.is_connected(),
            "rpc_url": self.rpc_url,
            "network": match self.network {
                FuegoNetwork::Testnet => "testnet",
                FuegoNetwork::Mainnet => "mainnet",
            },
            "fuego_block_height": height,
            "commitment_stats": stats.map(|s| serde_json::json!({
                "total_commitments": s.total_commitments,
                "heat_commitments": s.heat_commitments,
                "cold_commitments": s.cold_commitments,
                "merkle_root": s.merkle_root,
                "consensus_percentage": s.consensus_percentage,
            }))
        })
    }
}
