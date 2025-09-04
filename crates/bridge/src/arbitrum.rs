use crate::{BridgeState, Proof};
use anyhow::Result;
use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{Address, Bytes, U256},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Arbitrum bridge contract interface
#[derive(Debug, Clone)]
pub struct ArbitrumBridge {
    provider: Arc<Provider<Http>>,
    contract_address: Address,
    wallet: LocalWallet,
    contract: Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>,
}

/// Arbitrum bridge contract events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEvent {
    pub block_number: u64,
    pub transaction_hash: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

/// Arbitrum submission result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionResult {
    pub success: bool,
    pub transaction_hash: String,
    pub gas_used: u64,
    pub block_number: u64,
}

impl ArbitrumBridge {
    /// Create a new Arbitrum bridge instance
    pub async fn new(
        rpc_url: String,
        contract_address: Address,
        private_key: String,
    ) -> Result<Self> {
        let provider = Arc::new(Provider::<Http>::try_from(rpc_url)?);
        let wallet = private_key.parse::<LocalWallet>()?;
        let client = SignerMiddleware::new(provider.clone(), wallet.clone());

        // Bridge contract ABI (simplified)
        let contract_abi = include_bytes!("../abi/bridge.json");
        let abi: ethers::abi::Abi = serde_json::from_slice(contract_abi)?;
        let contract = Contract::new(contract_address, abi, client.into());

        Ok(Self {
            provider,
            contract_address,
            wallet,
            contract,
        })
    }

    /// Submit a batch of proofs to Arbitrum
    pub async fn submit_batch(&self, proofs: Vec<Proof>) -> Result<SubmissionResult> {
        // Convert proofs to the format expected by the contract
        let proof_data: Vec<Bytes> = proofs
            .into_iter()
            .map(|proof| {
                // Serialize proof to bytes
                let proof_bytes = serde_json::to_vec(&proof)?;
                Ok(Bytes::from(proof_bytes))
            })
            .collect::<Result<Vec<Bytes>>>()?;

        // Call the bridge contract's submitBatch function
        let call = self.contract.method::<Vec<ethers::types::Bytes>, ()>("submitBatch", proof_data)?;
        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;

        let receipt = receipt.unwrap();
        Ok(SubmissionResult {
            success: receipt.status == Some(U64::from(1)),
            transaction_hash: format!("{:?}", receipt.transaction_hash),
            gas_used: receipt.gas_used.unwrap_or_default().as_u64(),
            block_number: receipt.block_number.unwrap_or_default().as_u64(),
        })
    }

    /// Get the latest bridge state from Arbitrum
    pub async fn get_bridge_state(&self) -> Result<BridgeState> {
        // Call the bridge contract's getState function
        let call = self.contract.method("getState", ())?;
        let state_data: (u64, u64, u64) = call.call().await?;

        Ok(BridgeState {
            last_fuego_height: state_data.0,
            last_arbitrum_height: state_data.1,
            total_submitted: state_data.2,
            pending_proofs: 0,
            last_submission: 0,
        })
    }

    /// Verify a proof on Arbitrum
    pub async fn verify_proof(&self, proof: &Proof) -> Result<bool> {
        // Call the bridge contract's verifyProof function
        let proof_bytes = serde_json::to_vec(proof)?;
        let call = self.contract.method("verifyProof", Bytes::from(proof_bytes))?;
        let is_valid: bool = call.call().await?;

        Ok(is_valid)
    }

    /// Get bridge events from a specific block
    pub async fn get_events(&self, from_block: u64, to_block: u64) -> Result<Vec<BridgeEvent>> {
        // Get events from the bridge contract
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(self.contract_address);

        let logs = self.provider.get_logs(&filter).await?;

        let events: Vec<BridgeEvent> = logs
            .into_iter()
            .map(|log| BridgeEvent {
                block_number: log.block_number.unwrap_or_default().as_u64(),
                transaction_hash: format!("{:?}", log.transaction_hash),
                event_type: "BridgeEvent".to_string(),
                data: serde_json::to_value(log).unwrap_or_default(),
            })
            .collect();

        Ok(events)
    }

    /// Check if a block has been submitted to Arbitrum
    pub async fn is_block_submitted(&self, height: u64) -> Result<bool> {
        let state = self.get_bridge_state().await?;
        Ok(height <= state.last_fuego_height)
    }

    /// Get the gas price for submissions
    pub async fn get_gas_price(&self) -> Result<U256> {
        let gas_price = self.provider.get_gas_price().await?;
        Ok(gas_price)
    }

    /// Estimate gas for batch submission
    pub async fn estimate_gas(&self, proofs: Vec<Proof>) -> Result<u64> {
        let proof_data: Vec<Bytes> = proofs
            .into_iter()
            .map(|proof| {
                let proof_bytes = serde_json::to_vec(&proof)?;
                Ok(Bytes::from(proof_bytes))
            })
            .collect::<Result<Vec<Bytes>>>()?;

        let call = self.contract.method::<Vec<ethers::types::Bytes>, ()>("submitBatch", proof_data)?;
        let gas_estimate = call.estimate_gas().await?;

        Ok(gas_estimate.as_u64())
    }
}

/// Bridge contract ABI (placeholder - would be the actual ABI)
pub const BRIDGE_ABI: &str = r#"
[
    {
        "inputs": [
            {
                "internalType": "bytes[]",
                "name": "proofs",
                "type": "bytes[]"
            }
        ],
        "name": "submitBatch",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getState",
        "outputs": [
            {
                "internalType": "uint256",
                "name": "lastSubmittedHeight",
                "type": "uint256"
            },
            {
                "internalType": "uint256",
                "name": "lastFinalizedHeight",
                "type": "uint256"
            },
            {
                "internalType": "uint256",
                "name": "totalSubmissions",
                "type": "uint256"
            }
        ],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {
                "internalType": "bytes",
                "name": "proof",
                "type": "bytes"
            }
        ],
        "name": "verifyProof",
        "outputs": [
            {
                "internalType": "bool",
                "name": "",
                "type": "bool"
            }
        ],
        "stateMutability": "view",
        "type": "function"
    }
]
"#;
