use anyhow::Result;
use async_trait::async_trait;
use merkle_light::hash::Algorithm;
use merkle_light::merkle::MerkleTree;
use rocksdb::{DBWithThreadMode, MultiThreaded, Options};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

pub type MerkleRoot = [u8; 32];

/// State database trait for COLD L3
#[async_trait]
pub trait StateDB: Send + Sync {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &[u8]) -> Result<()>;
    async fn commit(&self, version: u64) -> Result<MerkleRoot>;
    async fn get_merkle_root(&self) -> Result<MerkleRoot>;
    async fn get_version(&self) -> Result<u64>;
}

/// RocksDB implementation of StateDB
pub struct RocksStateDB {
    db: Arc<DBWithThreadMode<MultiThreaded>>,
    current_version: Arc<RwLock<u64>>,
    pending_changes: Arc<RwLock<Vec<(Vec<u8>, Option<Vec<u8>>)>>>,
}

impl RocksStateDB {
    /// Create a new RocksDB state database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(10000);
        opts.set_use_fsync(true);
        
        let db = Arc::new(DBWithThreadMode::open(&opts, path)?);
        
        Ok(Self {
            db,
            current_version: Arc::new(RwLock::new(0)),
            pending_changes: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Get a value from the database
    async fn get_internal(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let db = self.db.clone();
        let key = key.to_vec();
        
        tokio::task::spawn_blocking(move || {
            db.get(&key).map_err(|e| anyhow::anyhow!("RocksDB error: {}", e))
        })
        .await?
    }

    /// Put a value in the database
    async fn put_internal(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();
        let value = value.to_vec();
        
        tokio::task::spawn_blocking(move || {
            db.put(&key, &value).map_err(|e| anyhow::anyhow!("RocksDB error: {}", e))
        })
        .await?
    }

    /// Delete a value from the database
    async fn delete_internal(&self, key: &[u8]) -> Result<()> {
        let db = self.db.clone();
        let key = key.to_vec();
        
        tokio::task::spawn_blocking(move || {
            db.delete(&key).map_err(|e| anyhow::anyhow!("RocksDB error: {}", e))
        })
        .await?
    }

    /// Calculate Merkle root from all key-value pairs
    async fn calculate_merkle_root(&self) -> Result<MerkleRoot> {
        let db = self.db.clone();
        
        tokio::task::spawn_blocking(move || {
            let mut hasher = Sha256::new();
            let mut iter = db.iterator(rocksdb::IteratorMode::Start);
            
            while let Some(result) = iter.next() {
                match result {
                    Ok((key, value)) => {
                        hasher.update(&key);
                        hasher.update(&value);
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("RocksDB iteration error: {}", e));
                    }
                }
            }
            
            let result = hasher.finalize();
            let mut root = [0u8; 32];
            root.copy_from_slice(&result);
            Ok(root)
        })
        .await?
    }
}

#[async_trait]
impl StateDB for RocksStateDB {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        debug!("Getting key: {:?}", hex::encode(key));
        self.get_internal(key).await
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        debug!("Putting key: {:?}, value: {:?}", hex::encode(key), hex::encode(value));
        
        // Add to pending changes
        {
            let mut changes = self.pending_changes.write().await;
            changes.push((key.to_vec(), Some(value.to_vec())));
        }
        
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        debug!("Deleting key: {:?}", hex::encode(key));
        
        // Add to pending changes
        {
            let mut changes = self.pending_changes.write().await;
            changes.push((key.to_vec(), None));
        }
        
        Ok(())
    }

    async fn commit(&self, version: u64) -> Result<MerkleRoot> {
        info!("Committing version {}", version);
        
        // Apply pending changes
        {
            let changes = self.pending_changes.read().await;
            for (key, value) in changes.iter() {
                match value {
                    Some(val) => self.put_internal(key, val).await?,
                    None => self.delete_internal(key).await?,
                }
            }
        }
        
        // Clear pending changes
        {
            let mut changes = self.pending_changes.write().await;
            changes.clear();
        }
        
        // Update version
        {
            let mut current_version = self.current_version.write().await;
            *current_version = version;
        }
        
        // Calculate and return Merkle root
        let root = self.calculate_merkle_root().await?;
        info!("Committed version {} with root: {:?}", version, hex::encode(root));
        
        Ok(root)
    }

    async fn get_merkle_root(&self) -> Result<MerkleRoot> {
        self.calculate_merkle_root().await
    }

    async fn get_version(&self) -> Result<u64> {
        let version = self.current_version.read().await;
        Ok(*version)
    }
}

/// Block state management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockState {
    pub height: u64,
    pub merkle_root: MerkleRoot,
    pub timestamp: u64,
    pub transaction_count: u64,
}

impl RocksStateDB {
    /// Store block state
    pub async fn store_block_state(&self, state: &BlockState) -> Result<()> {
        let key = format!("block:{}", state.height).into_bytes();
        let value = serde_json::to_vec(state)?;
        self.put(&key, &value).await
    }

    /// Get block state
    pub async fn get_block_state(&self, height: u64) -> Result<Option<BlockState>> {
        let key = format!("block:{}", height).into_bytes();
        if let Some(data) = self.get(&key).await? {
            let state: BlockState = serde_json::from_slice(&data)?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    /// Store commitment
    pub async fn store_commitment(&self, commitment_type: &str, data: &[u8]) -> Result<()> {
        let key = format!("commitment:{}", commitment_type).into_bytes();
        self.put(&key, data).await
    }

    /// Get commitment
    pub async fn get_commitment(&self, commitment_type: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("commitment:{}", commitment_type).into_bytes();
        self.get(&key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_state_db_operations() {
        let temp_dir = tempdir().unwrap();
        let db = RocksStateDB::new(temp_dir.path()).unwrap();
        
        // Test put and get
        let key = b"test_key";
        let value = b"test_value";
        
        db.put(key, value).await.unwrap();
        let retrieved = db.get(key).await.unwrap().unwrap();
        assert_eq!(retrieved, value);
        
        // Test commit
        let root = db.commit(1).await.unwrap();
        assert_ne!(root, [0u8; 32]);
        
        // Test version
        let version = db.get_version().await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn test_block_state_storage() {
        let temp_dir = tempdir().unwrap();
        let db = RocksStateDB::new(temp_dir.path()).unwrap();
        
        let state = BlockState {
            height: 1,
            merkle_root: [1u8; 32],
            timestamp: 1234567890,
            transaction_count: 10,
        };
        
        db.store_block_state(&state).await.unwrap();
        db.commit(1).await.unwrap();
        
        let retrieved = db.get_block_state(1).await.unwrap().unwrap();
        assert_eq!(retrieved.height, state.height);
        assert_eq!(retrieved.merkle_root, state.merkle_root);
    }
}
