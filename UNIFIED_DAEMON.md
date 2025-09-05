# Unified C0DL3-Fuego Daemon

The Unified Daemon combines both C0DL3 zkSync node and Fuego L1 mining into a single process, providing efficient dual-chain mining with shared resources and unified monitoring.

## 🚀 **Overview**

The unified daemon runs three main components simultaneously:
1. **C0DL3 zkSync Node** - Handles C0DL3 blockchain operations
2. **Fuego Mining Daemon** - Mines Fuego L1 blocks using CN-UPX/2
3. **Unified Monitor** - Tracks statistics across both chains

## ⚙️ **Configuration**

### **CLI Options**

```bash
# Unified daemon mode
--unified-daemon true

# C0DL3 Configuration
--target-block-time 30          # C0DL3 block time (seconds)
--p2p-port 30333               # P2P networking port
--rpc-port 9944                # RPC API port

# Fuego Configuration  
--fuego-block-time 480         # Fuego block time (seconds)
--fuego-rpc-url http://localhost:8546
--fuego-wallet-address 0x1111111111111111111111111111111111111111

# Merge Mining
--merge-mining-enabled true
--aux-pow-tag C0DL3-MERGE-MINING
--cn-upx2-difficulty 1000
```

### **Environment Variables**

```bash
# Unified Daemon Mode
UNIFIED_DAEMON=true

# C0DL3 Settings
TARGET_BLOCK_TIME=30
P2P_PORT=30333
RPC_PORT=9944

# Fuego Settings
FUEGO_BLOCK_TIME=480
FUEGO_RPC_URL=http://localhost:8546
FUEGO_WALLET_ADDRESS=0x1111111111111111111111111111111111111111

# Merge Mining
MERGE_MINING_ENABLED=true
AUX_POW_TAG=C0DL3-MERGE-MINING
CN_UPX2_DIFFICULTY=1000
```

## 🏃 **Running the Unified Daemon**

### **Method 1: Direct CLI**

```bash
cargo run -- \
    --unified-daemon true \
    --merge-mining-enabled true \
    --target-block-time 30 \
    --fuego-block-time 480 \
    --fuego-rpc-url http://localhost:8546
```

### **Method 2: Startup Script**

```bash
./scripts/start-unified-daemon.sh
```

### **Method 3: Docker Compose**

```bash
# Start unified daemon only
docker-compose -f docker-compose-unified.yml up unified-daemon

# Start with Fuego L1 node
docker-compose -f docker-compose-unified.yml --profile fuego up
```

## 📊 **Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                    Unified Daemon Process                   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   C0DL3 Node    │  │ Fuego Mining    │  │   Monitor   │ │
│  │                 │  │    Daemon       │  │             │ │
│  │ • zkSync ops    │  │ • CN-UPX/2      │  │ • Stats     │ │
│  │ • P2P network   │  │ • AuxPoW        │  │ • Rewards   │ │
│  │ • RPC server    │  │ • Block mining  │  │ • Health    │ │
│  │ • L1 monitoring │  │ • 480s blocks   │  │ • Logging   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│           │                     │                 │        │
│           └─────────────────────┼─────────────────┘        │
│                                 │                         │
│                    ┌─────────────────┐                    │
│                    │  Shared State   │                    │
│                    │                 │                    │
│                    │ • Block counts  │                    │
│                    │ • Rewards       │                    │
│                    │ • Statistics    │                    │
│                    └─────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

## 🔄 **Mining Flow**

### **Dual Chain Mining**

```rust
// C0DL3 Mining (30s blocks)
async fn mine_c0dl3_block() -> Block {
    // Mine C0DL3 block using CNUPX2-MM
    // Update C0DL3 state
    // Generate ZK proof
}

// Fuego Mining (480s blocks)  
async fn mine_fuego_block() -> FuegoBlock {
    // Mine Fuego block using CN-UPX/2
    // Create AuxPoW link
    // Update Fuego state
}

// Unified Mining Cycle
async fn unified_mining_cycle() {
    // Mine both blocks simultaneously
    let (c0dl3_block, fuego_block) = tokio::join!(
        mine_c0dl3_block(),
        mine_fuego_block()
    );
    
    // Create AuxPoW link between blocks
    let aux_pow = create_auxpow_link(&c0dl3_block, &fuego_block);
    
    // Update shared rewards
    update_dual_rewards(&c0dl3_block, &fuego_block);
}
```

## 💰 **Reward System**

### **Dual Rewards**

```json
{
  "c0dl3_rewards": {
    "gas_fees": 12000000,      // C0DL3 transaction fees
    "eldernode_fees": 6000000,  // ElderNode service fees
    "total": 18000000
  },
  "fuego_rewards": {
    "block_reward": 1000000,    // 1M Fuego tokens per block
    "total": 1000000
  },
  "combined_rewards": {
    "total": 19000000,          // Combined rewards
    "efficiency": 0.75          // Resource efficiency
  }
}
```

## 📈 **Monitoring & Statistics**

### **Unified Stats API**

```bash
# Get unified daemon statistics
curl http://localhost:9944/stats

# Response
{
  "unified_daemon": {
    "c0dl3_blocks_mined": 150,
    "fuego_blocks_mined": 12,
    "total_rewards": 19000000,
    "uptime_seconds": 3600,
    "mining_efficiency": 0.75
  },
  "c0dl3": {
    "current_height": 150,
    "hash_rate": 1000000,
    "pending_transactions": 5
  },
  "fuego": {
    "current_height": 12,
    "cn_upx2_difficulty": 1000,
    "aux_pow_links": 12
  }
}
```

### **Health Monitoring**

```bash
# Health check
curl http://localhost:9944/health

# Response
{
  "status": "healthy",
  "timestamp": 1703123456,
  "components": {
    "c0dl3_node": "running",
    "fuego_mining": "active", 
    "unified_monitor": "running"
  },
  "uptime": 3600
}
```

## 🔧 **Advanced Configuration**

### **Resource Optimization**

```bash
# Optimize for dual mining
--target-block-time 30          # Fast C0DL3 blocks
--fuego-block-time 480          # Standard Fuego blocks
--cn-upx2-difficulty 1000      # Balanced difficulty
--merge-mining-interval 30      # Frequent merge attempts
```

### **Network Configuration**

```bash
# P2P networking
--p2p-port 30333               # C0DL3 P2P port
--max-peers 50                 # Maximum connections

# RPC API
--rpc-port 9944                # Unified RPC port
--cors-origins "*"             # Allow all origins
```

## 🐳 **Docker Deployment**

### **Docker Compose**

```yaml
version: '3.8'
services:
  unified-daemon:
    build: .
    ports:
      - "9944:9944"  # RPC
      - "30333:30333"  # P2P
    environment:
      - UNIFIED_DAEMON=true
      - MERGE_MINING_ENABLED=true
      - TARGET_BLOCK_TIME=30
      - FUEGO_BLOCK_TIME=480
    volumes:
      - ./data:/app/data
    restart: unless-stopped
```

### **Docker Run**

```bash
docker run -d \
  --name c0dl3-unified-daemon \
  -p 9944:9944 \
  -p 30333:30333 \
  -e UNIFIED_DAEMON=true \
  -e MERGE_MINING_ENABLED=true \
  -v ./data:/app/data \
  c0dl3-unified-daemon:latest
```

## 📋 **Benefits**

### **For Miners**
- **Dual Revenue**: Earn from both C0DL3 and Fuego
- **Resource Efficiency**: Single process for dual mining
- **Simplified Management**: One daemon to manage
- **Higher Rewards**: Combined rewards from both chains

### **For Networks**
- **Increased Security**: More miners securing both networks
- **Better Distribution**: Decentralized mining across chains
- **Enhanced Stability**: Shared security model
- **Cross-Chain Synergy**: Mutual benefit for both networks

## 🚨 **Troubleshooting**

### **Common Issues**

1. **Port Conflicts**
   ```bash
   # Check port usage
   netstat -tulpn | grep :9944
   netstat -tulpn | grep :30333
   ```

2. **Fuego RPC Connection**
   ```bash
   # Test Fuego RPC
   curl -X POST http://localhost:8546 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
   ```

3. **Resource Usage**
   ```bash
   # Monitor resource usage
   docker stats c0dl3-unified-daemon
   ```

### **Logs**

```bash
# View unified daemon logs
docker logs -f c0dl3-unified-daemon

# Filter for specific components
docker logs c0dl3-unified-daemon | grep "C0DL3"
docker logs c0dl3-unified-daemon | grep "Fuego"
docker logs c0dl3-unified-daemon | grep "Unified"
```

## 🔮 **Future Enhancements**

- **Dynamic Difficulty Adjustment**: Auto-adjust based on network conditions
- **Advanced Monitoring**: Prometheus metrics integration
- **Load Balancing**: Distribute mining across multiple instances
- **Cross-Chain Transactions**: Direct C0DL3 ↔ Fuego transfers
- **Governance Integration**: Unified voting across both chains

---

**The Unified Daemon provides a powerful solution for dual-chain mining, combining the efficiency of shared resources with the security benefits of merge-mining across C0DL3 and Fuego networks.**
