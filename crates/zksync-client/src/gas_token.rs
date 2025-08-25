use anyhow::Result;
use ethers::types::{Address, U256};

pub struct GasTokenClient;

impl GasTokenClient {
    pub fn new(token_address: Address) -> Self {
        Self
    }
    
    pub async fn get_balance(&self, address: Address) -> Result<U256> {
        // Placeholder implementation
        Ok(U256::from(1_000_000_000_000_000_000u128)) // 1 HEAT
    }
    
    pub async fn transfer(&self, to: Address, amount: U256) -> Result<bool> {
        // Placeholder implementation
        Ok(true)
    }
}
