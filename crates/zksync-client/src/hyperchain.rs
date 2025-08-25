use anyhow::Result;
use ethers::types::{Address, U256, H256};
use crate::{HyperchainInfo, L2ChainId};

pub struct HyperchainClient;

impl HyperchainClient {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn get_info(&self, chain_id: L2ChainId) -> Result<HyperchainInfo> {
        Ok(HyperchainInfo {
            chain_id,
            name: "CODL3 Hyperchain".to_string(),
            gas_token: Address::random(),
            validator: Address::random(),
            max_tx_per_block: 1000,
            max_gas_per_block: 1_000_000,
            allowlist_enabled: true,
        })
    }
    
    pub async fn get_gas_price(&self) -> Result<U256> {
        Ok(U256::from(1_000_000_000u64)) // 1 gwei
    }
}
