use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub data_dir: String,
    pub p2p_port: u16,
    pub listen_addr: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            p2p_port: 30333,
            listen_addr: "0.0.0.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPCConfig {
    pub port: u16,
    pub host: String,
    pub cors_origins: Vec<String>,
}

impl Default for RPCConfig {
    fn default() -> Self {
        Self {
            port: 9944,
            host: "127.0.0.1".to_string(),
            cors_origins: vec!["*".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub enabled: bool,
    pub poll_interval: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub validator_count: u32,
    pub block_time: u64,
    pub finality_threshold: u32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuegoConfig {
    pub rpc_url: String,
    pub wallet_address: String,
    pub mining_threads: u32,
    pub block_time: u64,
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
