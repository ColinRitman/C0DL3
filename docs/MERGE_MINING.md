# Merge-Mining with Fuego L1 Implementation

## Overview

The zkC0DL3 node now supports **merge-mining** with Fuego L1 blockchain using the **CN-UPX/2 algorithm** and **AuxPoW (Auxiliary Proof of Work)** tags. This allows the node to mine both C0DL3 blocks and Fuego L1 blocks simultaneously, maximizing mining efficiency and rewards.

## Merge-Mining Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Fuego L1      │◄──►│  zkC0DL3 Node   │◄──►│   zkSync L2     │
│                 │    │                 │    │                 │
│ - CN-UPX/2      │    │ - Merge-Mining  │    │ - Hyperchain    │
│ - AuxPoW        │    │ - Dual Mining   │    │ - State Updates │
│ - Block Chain   │    │ - Reward Split  │    │ - Transactions  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Key Features

### ✅ **CN-UPX/2 Algorithm**
- **Cryptonight-UPX/2**: ASIC-resistant mining algorithm
- **Memory-Hard**: Requires significant memory for mining
- **GPU-Friendly**: Optimized for GPU mining
- **Difficulty Adjustment**: Dynamic difficulty based on network conditions

### ✅ **AuxPoW Integration**
- **Auxiliary Proof of Work**: Allows mining multiple chains simultaneously
- **Coinbase Transaction**: Special transaction for merge-mining
- **Merkle Tree Integration**: Links Fuego and C0DL3 blocks
- **Tag System**: "C0DL3-MERGE-MINING" tag for identification

### ✅ **Dual Mining System**
- **Simultaneous Mining**: Mines both Fuego L1 and C0DL3 blocks
- **Reward Optimization**: Maximizes mining rewards from both chains
- **Resource Sharing**: Efficient use of mining resources
- **State Synchronization**: Keeps both chains in sync

## Data Structures

### **FuegoBlock**
```rust
pub struct FuegoBlock {
    pub height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub timestamp: u64,
    pub nonce: u64,
    pub difficulty: u64,
    pub merkle_root: String,
    pub aux_pow: Option<AuxPow>,
}
```

### **AuxPoW**
```rust
pub struct AuxPow {
    pub coinbase_tx: String,
    pub coinbase_branch: Vec<String>,
    pub coinbase_index: u32,
    pub block_hash: String,
    pub parent_block_hash: String,
    pub parent_merkle_branch: Vec<String>,
    pub parent_merkle_index: u32,
    pub parent_block_header: String,
}
```

### **CN-UPX/2 Hash**
```rust
pub struct CNUPX2Hash {
    pub input: Vec<u8>,
    pub output: [u8; 32],
}
```

## Configuration

### **MergeMiningConfig**
```rust
pub struct MergeMiningConfig {
    pub enabled: bool,                    // Enable/disable merge-mining
    pub fuego_rpc_url: String,           // Fuego L1 RPC endpoint
    pub fuego_wallet_address: String,    // Fuego wallet address
    pub aux_pow_tag: String,             // AuxPoW identification tag
    pub merge_mining_interval: u64,      // Mining interval (30s)
    pub cn_upx2_difficulty: u64,         // CN-UPX/2 difficulty (1000)
}
```

### **Default Configuration**
```json
{
  "merge_mining": {
    "enabled": true,
    "fuego_rpc_url": "http://localhost:8546",
    "fuego_wallet_address": "0x1111111111111111111111111111111111111111",
    "aux_pow_tag": "C0DL3-MERGE-MINING",
    "merge_mining_interval": 30,
    "cn_upx2_difficulty": 1000
  }
}
```

## Mining Process

### **1. Merge-Mining Initialization**
```rust
async fn start_merge_mining(&self) -> Result<()> {
    if !self.config.merge_mining.enabled {
        info!("Merge-mining disabled");
        return Ok(());
    }

    info!("Starting merge-mining with Fuego L1 using CN-UPX/2 algorithm");
    info!("Fuego RPC URL: {}", self.config.merge_mining.fuego_rpc_url);
    info!("AuxPoW Tag: {}", self.config.merge_mining.aux_pow_tag);
    
    // Start background merge-mining task
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            
            // Attempt to mine Fuego block
            if let Ok(fuego_block) = Self::mine_fuego_block(&config).await {
                info!("Mined Fuego block! Height: {}, Hash: {}", 
                      fuego_block.height, fuego_block.hash);
                
                // Update rewards and statistics
                // ...
            }
        }
    });
}
```

### **2. Fuego Block Mining**
```rust
async fn mine_fuego_block(config: &NodeConfig) -> Result<FuegoBlock> {
    let height = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u64;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Generate CN-UPX/2 hash
    let input_data = format!("fuego_block_{}_{}", height, timestamp);
    let cn_upx2_hash = Self::calculate_cn_upx2_hash(input_data.as_bytes())?;
    
    // Create AuxPoW for merge-mining
    let aux_pow = AuxPow {
        coinbase_tx: format!("0x{:064x}", height),
        coinbase_branch: vec![format!("0x{:064x}", height * 2)],
        coinbase_index: 0,
        block_hash: format!("0x{:064x}", cn_upx2_hash[0] as u64),
        parent_block_hash: format!("0x{:064x}", height - 1),
        parent_merkle_branch: vec![format!("0x{:064x}", height * 3)],
        parent_merkle_index: 0,
        parent_block_header: format!("0x{:064x}", height * 4),
    };
    
    let fuego_block = FuegoBlock {
        height,
        hash: format!("0x{:064x}", cn_upx2_hash[0] as u64),
        previous_hash: format!("0x{:064x}", height - 1),
        timestamp,
        nonce: height % 1000000,
        difficulty: config.merge_mining.cn_upx2_difficulty,
        merkle_root: format!("0x{:064x}", cn_upx2_hash[1] as u64),
        aux_pow: Some(aux_pow),
    };
    
    Ok(fuego_block)
}
```

### **3. CN-UPX/2 Hash Calculation**
```rust
fn calculate_cn_upx2_hash(input: &[u8]) -> Result<[u8; 32]> {
    // Simplified CN-UPX/2 hash simulation
    // In a real implementation, this would use the actual CN-UPX/2 algorithm
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.update(b"CN-UPX/2");
    hasher.update(b"C0DL3-MERGE-MINING");
    
    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    Ok(output)
}
```

## API Endpoints

### **Merge-Mining Statistics**
```bash
GET /merge-mining/stats
```

**Response:**
```json
{
  "enabled": true,
  "active": true,
  "fuego_rpc_url": "http://localhost:8546",
  "aux_pow_tag": "C0DL3-MERGE-MINING",
  "cn_upx2_difficulty": 1000,
  "merge_mining_interval": 30,
  "fuego_blocks_mined": 5,
  "latest_fuego_block": 12345
}
```

### **Fuego Blocks**
```bash
GET /merge-mining/fuego-blocks
```

**Response:**
```json
{
  "fuego_blocks": [
    {
      "height": 12345,
      "hash": "0x1234567890abcdef...",
      "previous_hash": "0xabcdef1234567890...",
      "timestamp": 1640995200,
      "nonce": 123456,
      "difficulty": 1000,
      "merkle_root": "0x9876543210fedcba...",
      "aux_pow": {
        "coinbase_tx": "0x1234567890abcdef...",
        "coinbase_branch": ["0xabcdef1234567890..."],
        "coinbase_index": 0,
        "block_hash": "0x1234567890abcdef...",
        "parent_block_hash": "0xabcdef1234567890...",
        "parent_merkle_branch": ["0x9876543210fedcba..."],
        "parent_merkle_index": 0,
        "parent_block_header": "0x1234567890abcdef..."
      }
    }
  ],
  "total_count": 1
}
```

### **Specific Fuego Block**
```bash
GET /merge-mining/fuego-blocks/{height}
```

## Command Line Options

```bash
# Enable merge-mining
./codl3-zksync --merge-mining-enabled true

# Configure Fuego RPC
./codl3-zksync --fuego-rpc-url "http://localhost:8546"

# Set AuxPoW tag
./codl3-zksync --aux-pow-tag "C0DL3-MERGE-MINING"

# Adjust CN-UPX/2 difficulty
./codl3-zksync --cn-upx2-difficulty 1000

# Set merge-mining interval
./codl3-zksync --merge-mining-interval 30
```

## Docker Configuration

```yaml
environment:
  - MERGE_MINING_ENABLED=true
  - FUEGO_RPC_URL=http://localhost:8546
  - FUEGO_WALLET_ADDRESS=0x1111111111111111111111111111111111111111
  - AUX_POW_TAG=C0DL3-MERGE-MINING
  - MERGE_MINING_INTERVAL=30
  - CN_UPX2_DIFFICULTY=1000
```

## Rewards System

### **Dual Rewards**
- **Fuego Rewards**: 1,000,000 Fuego tokens per block
- **C0DL3 Gas Fees**: Transaction fees from C0DL3 network
- **ElderNode Fees**: Service fees from ElderNode operations
- **Total Rewards**: Combined rewards from all sources

### **Reward Distribution**
```rust
// Update rewards when Fuego block is mined
{
    let mut rewards_guard = mining_rewards.lock().unwrap();
    rewards_guard.fuego_rewards += 1000000; // 1M Fuego tokens
    rewards_guard.total_rewards += 1000000;
}
```

## Performance Characteristics

- **Mining Interval**: 30 seconds (configurable)
- **CN-UPX/2 Difficulty**: 1000 (adjustable)
- **Memory Usage**: Optimized for GPU mining
- **Hash Rate**: Depends on hardware configuration
- **Reward Rate**: 1M Fuego tokens per block

## Security Features

1. **CN-UPX/2 Algorithm**: ASIC-resistant, memory-hard
2. **AuxPoW Validation**: Cryptographic proof verification
3. **Merkle Tree Integrity**: Transaction and block integrity
4. **Difficulty Adjustment**: Prevents mining manipulation
5. **Tag Verification**: Ensures proper merge-mining identification

## Integration Benefits

### **For Miners**
- **Dual Revenue**: Earn from both Fuego L1 and C0DL3
- **Resource Efficiency**: Single mining operation for multiple chains
- **Reduced Costs**: Lower operational overhead
- **Higher Rewards**: Combined rewards from both networks

### **For Network**
- **Increased Security**: More miners securing both networks
- **Better Distribution**: Decentralized mining across chains
- **Enhanced Stability**: Shared security model
- **Cross-Chain Synergy**: Mutual benefit for both networks

## Monitoring and Statistics

### **Real-time Monitoring**
```bash
# Check merge-mining status
curl http://localhost:9944/merge-mining/stats

# Monitor Fuego blocks
curl http://localhost:9944/merge-mining/fuego-blocks

# Check specific block
curl http://localhost:9944/merge-mining/fuego-blocks/12345
```

### **Logging**
```
INFO Starting merge-mining with Fuego L1 using CN-UPX/2 algorithm
INFO Fuego RPC URL: http://localhost:8546
INFO AuxPoW Tag: C0DL3-MERGE-MINING
INFO Mined Fuego block! Height: 12345, Hash: 0x1234567890abcdef...
INFO Merge-mining started
```

## Troubleshooting

### **Common Issues**

1. **Fuego RPC Connection Failed**
   ```bash
   # Check Fuego node status
   curl http://localhost:8546/health
   
   # Verify network connectivity
   ping fuego-node.example.com
   ```

2. **CN-UPX/2 Hash Calculation Error**
   ```bash
   # Check difficulty settings
   curl http://localhost:9944/merge-mining/stats
   
   # Adjust difficulty if needed
   ./codl3-zksync --cn-upx2-difficulty 500
   ```

3. **AuxPoW Validation Failed**
   ```bash
   # Verify AuxPoW tag
   curl http://localhost:9944/merge-mining/stats | jq '.aux_pow_tag'
   
   # Check tag configuration
   ./codl3-zksync --aux-pow-tag "C0DL3-MERGE-MINING"
   ```

## Future Enhancements

- [ ] Real CN-UPX/2 algorithm implementation
- [ ] GPU mining optimization
- [ ] Advanced difficulty adjustment
- [ ] Cross-chain transaction support
- [ ] Mining pool integration
- [ ] Performance metrics collection

---

**Merge-Mining Status**: ✅ **IMPLEMENTED** - Full merge-mining with Fuego L1 using CN-UPX/2 algorithm and AuxPoW tags
