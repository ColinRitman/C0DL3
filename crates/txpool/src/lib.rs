use anyhow::Result;
use block_sync::{Transaction, Block};
use dashmap::DashMap;
use priority_queue::PriorityQueue;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Transaction priority based on fee and age
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionPriority {
    pub fee_per_byte: u64,
    pub timestamp: u64,
    pub nonce: u64,
}

/// Transaction pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxPoolConfig {
    pub max_pool_size: usize,
    pub max_transaction_size: usize,
    pub min_fee_per_byte: u64,
    pub max_fee_per_byte: u64,
}

impl Default for TxPoolConfig {
    fn default() -> Self {
        Self {
            max_pool_size: 10000,
            max_transaction_size: 1024 * 1024, // 1MB
            min_fee_per_byte: 1,
            max_fee_per_byte: 1000000,
        }
    }
}

/// Transaction pool for COLD L3
pub struct TxPool {
    config: TxPoolConfig,
    transactions: DashMap<[u8; 32], Transaction>,
    priority_queue: Arc<RwLock<PriorityQueue<[u8; 32], TransactionPriority>>>,
    account_nonces: DashMap<[u8; 32], u64>,
    stats: Arc<RwLock<TxPoolStats>>,
}

/// Transaction pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxPoolStats {
    pub total_transactions: usize,
    pub total_fees: u64,
    pub average_fee_per_byte: f64,
    pub oldest_transaction: u64,
    pub newest_transaction: u64,
}

impl Default for TxPoolStats {
    fn default() -> Self {
        Self {
            total_transactions: 0,
            total_fees: 0,
            average_fee_per_byte: 0.0,
            oldest_transaction: 0,
            newest_transaction: 0,
        }
    }
}

impl TxPool {
    /// Create a new transaction pool
    pub fn new(config: TxPoolConfig) -> Self {
        info!("Creating transaction pool with config: {:?}", config);
        
        Self {
            config,
            transactions: DashMap::new(),
            priority_queue: Arc::new(RwLock::new(PriorityQueue::new())),
            account_nonces: DashMap::new(),
            stats: Arc::new(RwLock::new(TxPoolStats::default())),
        }
    }

    /// Add a transaction to the pool
    pub async fn add_transaction(&mut self, tx: Transaction) -> Result<()> {
        debug!("Adding transaction: {:?}", hex::encode(tx.hash));
        
        // Validate transaction
        if !self.validate_transaction(&tx).await? {
            return Err(anyhow::anyhow!("Invalid transaction"));
        }
        
        // Check pool size limit
        if self.transactions.len() >= self.config.max_pool_size {
            self.evict_lowest_priority_transaction().await?;
        }
        
        // Calculate priority
        let priority = self.calculate_priority(&tx);
        
        // Add to pool
        self.transactions.insert(tx.hash, tx.clone());
        
        // Add to priority queue
        {
            let mut queue = self.priority_queue.write().await;
            queue.push(tx.hash, priority);
        }
        
        // Update account nonce
        self.account_nonces.insert(tx.from, tx.nonce);
        
        // Update statistics
        self.update_stats(&tx).await;
        
        info!("Added transaction to pool. Pool size: {}", self.transactions.len());
        Ok(())
    }

    /// Get transactions for block inclusion
    pub async fn get_transactions(&self, limit: usize) -> Vec<Transaction> {
        debug!("Getting {} transactions for block inclusion", limit);
        
        let mut selected_transactions = Vec::new();
        let mut queue = self.priority_queue.write().await;
        
        while selected_transactions.len() < limit && !queue.is_empty() {
            if let Some((tx_hash, _priority)) = queue.pop() {
                if let Some(tx) = self.transactions.get(&tx_hash) {
                    selected_transactions.push(tx.clone());
                }
            }
        }
        
        // Re-add transactions back to queue (they weren't actually included yet)
        for tx in &selected_transactions {
            let priority = self.calculate_priority(tx);
            queue.push(tx.hash, priority);
        }
        
        info!("Selected {} transactions for block inclusion", selected_transactions.len());
        selected_transactions
    }

    /// Remove transaction from pool (e.g., after block inclusion)
    pub async fn remove_transaction(&mut self, tx_hash: &[u8; 32]) -> Result<()> {
        debug!("Removing transaction: {:?}", hex::encode(tx_hash));
        
        if let Some((_, tx)) = self.transactions.remove(tx_hash) {
            // Remove from priority queue
            {
                let mut queue = self.priority_queue.write().await;
                queue.remove(tx_hash);
            }
            
            // Update statistics
            self.remove_from_stats(&tx).await;
            
            info!("Removed transaction from pool. Pool size: {}", self.transactions.len());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Transaction not found in pool"))
        }
    }

    /// Get transaction by hash
    pub fn get_transaction(&self, tx_hash: &[u8; 32]) -> Option<Transaction> {
        self.transactions.get(tx_hash).map(|tx| tx.clone())
    }

    /// Get pool statistics
    pub async fn get_stats(&self) -> TxPoolStats {
        self.stats.read().await.clone()
    }

    /// Validate transaction
    async fn validate_transaction(&self, tx: &Transaction) -> Result<bool> {
        // Check transaction size
        let tx_size = std::mem::size_of_val(tx);
        if tx_size > self.config.max_transaction_size {
            warn!("Transaction too large: {} bytes", tx_size);
            return Ok(false);
        }
        
        // Check fee per byte
        let fee_per_byte = tx.fee / tx_size as u64;
        if fee_per_byte < self.config.min_fee_per_byte || fee_per_byte > self.config.max_fee_per_byte {
            warn!("Invalid fee per byte: {}", fee_per_byte);
            return Ok(false);
        }
        
        // Check nonce
        if let Some(expected_nonce) = self.account_nonces.get(&tx.from) {
            if tx.nonce <= *expected_nonce {
                warn!("Invalid nonce: expected > {}, got {}", *expected_nonce, tx.nonce);
                return Ok(false);
            }
        }
        
        // TODO: Verify signature
        // TODO: Check balance
        
        Ok(true)
    }

    /// Calculate transaction priority
    fn calculate_priority(&self, tx: &Transaction) -> TransactionPriority {
        let fee_per_byte = tx.fee / std::mem::size_of_val(tx) as u64;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        TransactionPriority {
            fee_per_byte,
            timestamp,
            nonce: tx.nonce,
        }
    }

    /// Evict lowest priority transaction
    async fn evict_lowest_priority_transaction(&mut self) -> Result<()> {
        let mut queue = self.priority_queue.write().await;
        
        if let Some((tx_hash, _priority)) = queue.pop() {
            if let Some((_, tx)) = self.transactions.remove(&tx_hash) {
                self.remove_from_stats(&tx).await;
                info!("Evicted lowest priority transaction: {:?}", hex::encode(tx_hash));
            }
        }
        
        Ok(())
    }

    /// Update pool statistics
    async fn update_stats(&self, tx: &Transaction) {
        let mut stats = self.stats.write().await;
        stats.total_transactions += 1;
        stats.total_fees += tx.fee;
        stats.average_fee_per_byte = stats.total_fees as f64 / stats.total_transactions as f64;
        
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if stats.oldest_transaction == 0 || timestamp < stats.oldest_transaction {
            stats.oldest_transaction = timestamp;
        }
        if timestamp > stats.newest_transaction {
            stats.newest_transaction = timestamp;
        }
    }

    /// Remove from statistics
    async fn remove_from_stats(&self, tx: &Transaction) {
        let mut stats = self.stats.write().await;
        if stats.total_transactions > 0 {
            stats.total_transactions -= 1;
            stats.total_fees = stats.total_fees.saturating_sub(tx.fee);
            if stats.total_transactions > 0 {
                stats.average_fee_per_byte = stats.total_fees as f64 / stats.total_transactions as f64;
            } else {
                stats.average_fee_per_byte = 0.0;
            }
        }
    }

    /// Clear all transactions
    pub async fn clear(&mut self) {
        info!("Clearing transaction pool");
        self.transactions.clear();
        {
            let mut queue = self.priority_queue.write().await;
            queue.clear();
        }
        self.account_nonces.clear();
        {
            let mut stats = self.stats.write().await;
            *stats = TxPoolStats::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_txpool_creation() {
        let config = TxPoolConfig::default();
        let pool = TxPool::new(config);
        assert_eq!(pool.transactions.len(), 0);
    }

    #[tokio::test]
    async fn test_add_transaction() {
        let config = TxPoolConfig::default();
        let mut pool = TxPool::new(config);
        
        let tx = Transaction {
            hash: [1u8; 32],
            from: [2u8; 32],
            to: [3u8; 32],
            amount: 100,
            fee: 10,
            nonce: 1,
            signature: [0u8; 64],
            data: vec![],
        };
        
        pool.add_transaction(tx).await.unwrap();
        assert_eq!(pool.transactions.len(), 1);
    }

    #[tokio::test]
    async fn test_get_transactions() {
        let config = TxPoolConfig::default();
        let mut pool = TxPool::new(config);
        
        // Add multiple transactions
        for i in 0..5 {
            let tx = Transaction {
                hash: [i; 32],
                from: [i; 32],
                to: [i; 32],
                amount: 100,
                fee: 10 + i as u64,
                nonce: i as u64,
                signature: [0u8; 64],
                data: vec![],
            };
            pool.add_transaction(tx).await.unwrap();
        }
        
        let transactions = pool.get_transactions(3).await;
        assert_eq!(transactions.len(), 3);
    }

    #[tokio::test]
    async fn test_remove_transaction() {
        let config = TxPoolConfig::default();
        let mut pool = TxPool::new(config);
        
        let tx = Transaction {
            hash: [1u8; 32],
            from: [2u8; 32],
            to: [3u8; 32],
            amount: 100,
            fee: 10,
            nonce: 1,
            signature: [0u8; 64],
            data: vec![],
        };
        
        pool.add_transaction(tx.clone()).await.unwrap();
        assert_eq!(pool.transactions.len(), 1);
        
        pool.remove_transaction(&tx.hash).await.unwrap();
        assert_eq!(pool.transactions.len(), 0);
    }
}
