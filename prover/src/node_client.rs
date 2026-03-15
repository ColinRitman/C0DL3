// HTTP client for the COLDL3 node RPC.
//
// The prover service communicates with the node via REST endpoints:
//   GET  /proof/pending         — list blocks awaiting proof
//   GET  /proof/block_input/{h} — download block data + pre-state witness
//   POST /proof/submit          — submit generated proof
//   GET  /health                — check node availability

use anyhow::{Result, Context};
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::types::{BlockInputResponse, GuestBlockInput, PendingBlock, ProofSubmission};

/// Client for the COLDL3 node RPC.
pub struct NodeClient {
    base_url: String,
    client: Client,
}

impl NodeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Check if the node is reachable.
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(e) => {
                warn!("Node health check failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get list of blocks pending proof.
    pub async fn get_pending_blocks(&self) -> Result<Vec<PendingBlock>> {
        let url = format!("{}/proof/pending", self.base_url);
        let resp = self.client.get(&url).send().await
            .context("failed to fetch pending blocks")?;

        if !resp.status().is_success() {
            anyhow::bail!("GET /proof/pending returned {}", resp.status());
        }

        let body: serde_json::Value = resp.json().await
            .context("failed to parse pending blocks response")?;

        // The node returns { "pending_blocks": [...], "current_height": N }
        let blocks = body.get("pending_blocks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        Some(PendingBlock {
                            block_height: v.get("block_height")?.as_u64()?,
                            status: v.get("status")?.as_str()?.to_string(),
                            tx_count: v.get("tx_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                            submissions: v.get("submissions").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(blocks)
    }

    /// Download block input data for SP1 guest execution.
    ///
    /// Returns the full GuestBlockInput (transactions, pre-state witnesses,
    /// shielded pool data) needed to prove a block.
    pub async fn get_block_input(&self, height: u64) -> Result<BlockInputResponse> {
        let url = format!("{}/proof/block_input/{}", self.base_url, height);
        info!("Fetching block input for height {} from {}", height, url);

        let resp = self.client.get(&url).send().await
            .context(format!("failed to fetch block input for height {}", height))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "GET /proof/block_input/{} returned {}: {}",
                height, status, body
            );
        }

        let input: BlockInputResponse = resp.json().await
            .context(format!("failed to parse block input for height {}", height))?;

        debug!(
            "Block {} input: {} txs, {} accounts, {} shielded ops",
            height,
            input.block_input.transactions.len(),
            input.block_input.accounts.len(),
            input.block_input.shielded.spend_proofs.len()
                + input.block_input.shielded.new_notes.len(),
        );

        Ok(input)
    }

    /// Submit a proof to the node.
    pub async fn submit_proof(&self, submission: &ProofSubmission) -> Result<serde_json::Value> {
        let url = format!("{}/proof/submit", self.base_url);
        info!(
            "Submitting proof for block {} ({} bytes)",
            submission.block_height,
            submission.proof_bytes.len(),
        );

        let resp = self.client.post(&url)
            .json(submission)
            .send()
            .await
            .context("failed to submit proof")?;

        let body: serde_json::Value = resp.json().await
            .context("failed to parse proof submission response")?;

        if body.get("status").and_then(|v| v.as_str()) == Some("error") {
            let msg = body.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            anyhow::bail!("Proof rejected: {}", msg);
        }

        Ok(body)
    }

    /// Get current block height from the node.
    pub async fn get_block_height(&self) -> Result<u64> {
        let url = format!("{}/stats", self.base_url);
        let resp = self.client.get(&url).send().await
            .context("failed to fetch stats")?;

        let body: serde_json::Value = resp.json().await
            .context("failed to parse stats response")?;

        body.get("block_height")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow::anyhow!("block_height not found in stats response"))
    }
}
