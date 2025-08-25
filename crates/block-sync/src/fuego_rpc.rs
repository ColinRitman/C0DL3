use crate::{Block, BlockHeader, Transaction};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Fuego RPC client for communicating with Fuego nodes
pub struct FuegoRPC {
    endpoint: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FuegoBlock {
    pub hash: String,
    pub height: u64,
    pub timestamp: u64,
    pub transactions: Vec<FuegoTransaction>,
    pub previous_hash: String,
    pub nonce: u64,
    pub difficulty: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FuegoTransaction {
    pub hash: String,
    pub amount: u64,
    pub fee: u64,
    pub from: String,
    pub to: String,
    pub timestamp: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FuegoRPCResponse<T> {
    pub result: Option<T>,
    pub error: Option<String>,
    pub id: u64,
}

impl FuegoRPC {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            client: reqwest::Client::new(),
        }
    }

    /// Get the latest block from Fuego
    pub async fn get_latest_block(&self) -> Result<FuegoBlock> {
        let response: FuegoRPCResponse<FuegoBlock> = self
            .client
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "get_latest_block",
                "params": [],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;

        response.result.ok_or_else(|| {
            anyhow!("Fuego RPC error: {}", response.error.unwrap_or_default())
        })
    }

    /// Get block by height
    pub async fn get_block_by_height(&self, height: u64) -> Result<FuegoBlock> {
        let response: FuegoRPCResponse<FuegoBlock> = self
            .client
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "get_block_by_height",
                "params": [height],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;

        response.result.ok_or_else(|| {
            anyhow!("Fuego RPC error: {}", response.error.unwrap_or_default())
        })
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<FuegoBlock> {
        let response: FuegoRPCResponse<FuegoBlock> = self
            .client
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "get_block_by_hash",
                "params": [hash],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;

        response.result.ok_or_else(|| {
            anyhow!("Fuego RPC error: {}", response.error.unwrap_or_default())
        })
    }

    /// Verify PoW for a Fuego block
    pub async fn verify_pow(&self, block: &FuegoBlock) -> Result<bool> {
        // This would typically call Fuego's PoW verification
        // For now, we'll implement a basic check
        let response: FuegoRPCResponse<bool> = self
            .client
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "verify_pow",
                "params": [{
                    "hash": block.hash,
                    "nonce": block.nonce,
                    "difficulty": block.difficulty
                }],
                "id": 1
            }))
            .send()
            .await?
            .json()
            .await?;

        response.result.ok_or_else(|| {
            anyhow!("Fuego RPC error: {}", response.error.unwrap_or_default())
        })
    }

    /// Convert Fuego block to COLD L3 block format
    pub fn convert_block(&self, fuego_block: FuegoBlock) -> Result<Block> {
        // Convert previous hash to bytes
        let prev_hash_bytes = hex::decode(&fuego_block.previous_hash)?;
        let prev_hash: [u8; 32] = prev_hash_bytes.try_into()
            .map_err(|_| anyhow!("Invalid previous hash length"))?;
        
        // Convert fuego block hash to bytes
        let fuego_hash_bytes = hex::decode(&fuego_block.hash)?;
        let fuego_hash: [u8; 32] = fuego_hash_bytes.try_into()
            .map_err(|_| anyhow!("Invalid fuego hash length"))?;
        
        // Calculate merkle root (placeholder - would be calculated from transactions)
        let merkle_root = [0u8; 32];
        
        let header = BlockHeader {
            height: fuego_block.height,
            timestamp: fuego_block.timestamp,
            prev_hash,
            merkle_root,
            fuego_block_hash: fuego_hash,
            validator_signature: None, // Will be set by validators
        };

        let transactions: Result<Vec<Transaction>> = fuego_block
            .transactions
            .into_iter()
            .map(|tx| {
                // Convert hash to bytes
                let hash_bytes = hex::decode(&tx.hash)?;
                let hash: [u8; 32] = hash_bytes.try_into()
                    .map_err(|_| anyhow!("Invalid transaction hash length"))?;
                
                // Convert addresses (placeholder - would parse actual addresses)
                let from_bytes = hex::decode(&tx.from).unwrap_or_else(|_| vec![0u8; 32]);
                let from: [u8; 32] = from_bytes.try_into().unwrap_or([0u8; 32]);
                
                let to_bytes = hex::decode(&tx.to).unwrap_or_else(|_| vec![0u8; 32]);
                let to: [u8; 32] = to_bytes.try_into().unwrap_or([0u8; 32]);
                
                // Convert signature
                let signature = hex::decode(&tx.signature)?;
                
                Ok(Transaction {
                    hash,
                    from,
                    to,
                    amount: tx.amount,
                    fee: tx.fee,
                    nonce: 0, // Fuego transactions don't have nonce in this format
                    signature,
                    data: vec![], // No additional data
                })
            })
            .collect();

        Ok(Block {
            header,
            transactions: transactions?,
        })
    }
}
