# Fuego Daemon Integration

This document describes how the unified C0DL3-Fuego daemon integrates with the actual Fuego daemon from the fuego-fresh repository.

## 🎯 **Overview**

The unified daemon now supports integration with the **actual Fuego daemon** from fuego-fresh, providing real CN-UPX/2 mining capabilities alongside C0DL3 zkSync operations.

## 🔧 **Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                Unified C0DL3-Fuego Daemon                   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   C0DL3 Node    │  │ Actual Fuego    │  │   Monitor   │ │
│  │                 │  │    Daemon       │  │             │ │
│  │ • zkSync ops    │  │ • Real CN-UPX/2 │  │ • Stats     │ │
│  │ • P2P network   │  │ • Actual mining │  │ • Rewards   │ │
│  │ • RPC server    │  │ • Real blocks   │  │ • Health    │ │
│  │ • L1 monitoring │  │ • RPC interface │  │ • Logging   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│           │                     │                 │        │
│           └─────────────────────┼─────────────────┘        │
│                                 │                         │
│                    ┌─────────────────┐                    │
│                    │  Shared State   │                    │
│                    │                 │                    │
│                    │ • Block counts  │                    │
│                    │ • Real rewards  │                    │
│                    │ • Statistics    │                    │
│                    └─────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 **Getting Started**

### **1. Build Fuego Daemon**

```bash
# Build the actual Fuego daemon from fuego-fresh
./scripts/build-fuego-daemon.sh
```

This script will:
- Check dependencies (CMake, Make, Boost)
- Install Boost if needed
- Build the Fuego daemon from source
- Copy the binary to `./bin/fuegod`
- Create a symlink at `./fuegod`

### **2. Start Unified Daemon**

```bash
# Method 1: Using startup script
./scripts/start-unified-daemon.sh

# Method 2: Direct CLI
cargo run -- --unified-daemon true --fuego-binary-path ./fuegod

# Method 3: Docker Compose
docker-compose -f docker-compose-unified.yml up unified-daemon
```

## ⚙️ **Configuration**

### **CLI Options**

```bash
# Fuego daemon configuration
--fuego-binary-path ./fuegod          # Path to Fuego daemon binary
--fuego-data-dir ./fuego-data         # Fuego data directory
--fuego-p2p-port 10808                # Fuego P2P port
--fuego-rpc-url http://localhost:8546 # Fuego RPC URL
--fuego-block-time 480                # Fuego block time (seconds)

# Merge mining configuration
--merge-mining-enabled true            # Enable merge mining
--aux-pow-tag C0DL3-MERGE-MINING      # AuxPoW tag
--cn-upx2-difficulty 1000             # CN-UPX/2 difficulty
```

### **Environment Variables**

```bash
# Fuego daemon settings
FUEGO_BINARY_PATH=/app/bin/fuegod
FUEGO_DATA_DIR=/app/fuego-data
FUEGO_P2P_PORT=10808
FUEGO_RPC_URL=http://localhost:8546
FUEGO_BLOCK_TIME=480

# Merge mining settings
MERGE_MINING_ENABLED=true
AUX_POW_TAG=C0DL3-MERGE-MINING
CN_UPX2_DIFFICULTY=1000
```

## 🔄 **Integration Process**

### **1. Daemon Startup**

```rust
// Create Fuego daemon from merge mining config
let fuego_daemon = FuegoDaemon::from_merge_mining_config(&config.merge_mining);

// Start the actual Fuego daemon process
fuego_daemon.start().await?;
```

### **2. Process Management**

```rust
// Start Fuego daemon as subprocess
let mut cmd = Command::new(&config.fuego_binary_path);
cmd.arg("--data-dir").arg(&config.fuego_data_dir)
   .arg("--rpc-bind-port").arg(config.fuego_rpc_port.to_string())
   .arg("--p2p-bind-port").arg(config.fuego_p2p_port.to_string())
   .arg("--testnet"); // Use testnet mode

let child = cmd.spawn()?;
```

### **3. RPC Communication**

```rust
// Get block height from Fuego RPC
let payload = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "getblockcount",
    "params": [],
    "id": 1
});

let response = client.post(&config.fuego_rpc_url)
    .json(&payload)
    .send()
    .await?;
```

### **4. State Synchronization**

```rust
// Monitor Fuego daemon and update shared state
let mut interval = tokio::time::interval(Duration::from_secs(30));

loop {
    interval.tick().await;
    
    // Get Fuego daemon stats
    let fuego_stats = fuego_daemon.get_stats();
    
    // Update shared state
    let mut state_guard = state.lock().unwrap();
    state_guard.fuego_blocks_mined = fuego_stats.blocks_mined;
    state_guard.fuego_rewards = fuego_stats.total_rewards;
    state_guard.total_rewards = state_guard.c0dl3_rewards + fuego_stats.total_rewards;
}
```

## 📊 **Real vs Simulation Mode**

### **Real Mode (Default)**
- Uses actual Fuego daemon binary
- Real CN-UPX/2 mining
- Actual blockchain data
- Real RPC communication
- Production-ready

### **Simulation Mode (Fallback)**
- Used when Fuego binary not found
- Simulated CN-UPX/2 mining
- Mock blockchain data
- Development/testing only

### **Mode Detection**

```rust
// Check if Fuego binary exists
if !std::path::Path::new(&self.config.fuego_binary_path).exists() {
    warn!("Fuego binary not found, using simulation mode");
    return self.start_simulation_mode().await;
}
```

## 🔧 **Fuego Daemon Features**

### **Supported RPC Methods**

```json
{
  "getblockcount": "Get current block height",
  "getblock": "Get block by height",
  "getblockhash": "Get block hash by height",
  "getdifficulty": "Get current difficulty",
  "getinfo": "Get daemon information",
  "getpeerinfo": "Get peer information"
}
```

### **Block Structure**

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
    pub reward: u64,
}
```

### **AuxPoW Integration**

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

## 📈 **Performance Monitoring**

### **Real-time Statistics**

```bash
# Get unified daemon stats
curl http://localhost:9944/stats

# Response includes real Fuego data
{
  "unified_daemon": {
    "c0dl3_blocks_mined": 150,
    "fuego_blocks_mined": 12,        # Real Fuego blocks
    "total_rewards": 19000000,       # Real combined rewards
    "uptime_seconds": 3600,
    "mining_efficiency": 0.75
  },
  "fuego": {
    "current_height": 12,            # Real Fuego height
    "cn_upx2_difficulty": 1000,      # Real difficulty
    "aux_pow_links": 12,             # Real AuxPoW links
    "hash_rate": 1000000,            # Real hash rate
    "daemon_status": "running"       # Real daemon status
  }
}
```

### **Health Monitoring**

```bash
# Health check includes Fuego daemon status
curl http://localhost:9944/health

# Response
{
  "status": "healthy",
  "components": {
    "c0dl3_node": "running",
    "fuego_daemon": "running",       # Real Fuego daemon status
    "unified_monitor": "running"
  },
  "fuego_daemon": {
    "process_id": 12345,
    "uptime": 3600,
    "last_block": 12,
    "rpc_connected": true
  }
}
```

## 🐳 **Docker Integration**

### **Dockerfile Updates**

```dockerfile
# Copy Fuego daemon binary
COPY bin/fuegod /app/bin/fuegod
RUN chmod +x /app/bin/fuegod

# Create Fuego data directory
RUN mkdir -p /app/fuego-data
```

### **Docker Compose**

```yaml
services:
  unified-daemon:
    volumes:
      - ./fuego-data:/app/fuego-data
      - ./bin:/app/bin
    environment:
      - FUEGO_BINARY_PATH=/app/bin/fuegod
      - FUEGO_DATA_DIR=/app/fuego-data
      - FUEGO_P2P_PORT=10808
```

## 🔍 **Troubleshooting**

### **Common Issues**

1. **Fuego Binary Not Found**
   ```bash
   # Build Fuego daemon
   ./scripts/build-fuego-daemon.sh
   
   # Verify binary exists
   ls -la ./fuegod
   ```

2. **Boost Dependencies Missing**
   ```bash
   # Install Boost on macOS
   brew install boost
   
   # Install Boost on Ubuntu
   sudo apt-get install libboost-all-dev
   ```

3. **RPC Connection Failed**
   ```bash
   # Test Fuego RPC
   curl -X POST http://localhost:8546 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}'
   ```

4. **Port Conflicts**
   ```bash
   # Check port usage
   netstat -tulpn | grep :8546
   netstat -tulpn | grep :10808
   ```

### **Logs**

```bash
# View unified daemon logs
docker logs -f c0dl3-unified-daemon

# Filter for Fuego-specific logs
docker logs c0dl3-unified-daemon | grep "Fuego"
```

## 🎉 **Benefits**

### **Real Mining**
- ✅ **Actual CN-UPX/2 Algorithm**: Real Fuego mining
- ✅ **Real Blockchain Data**: Actual block heights and hashes
- ✅ **Real Rewards**: Actual Fuego token rewards
- ✅ **Real Network**: Connected to Fuego network

### **Production Ready**
- ✅ **Process Management**: Proper daemon lifecycle
- ✅ **Error Handling**: Robust error recovery
- ✅ **Monitoring**: Real-time health checks
- ✅ **Scalability**: Production-grade architecture

### **Developer Friendly**
- ✅ **Fallback Mode**: Simulation when binary unavailable
- ✅ **Easy Setup**: Automated build scripts
- ✅ **Comprehensive Logging**: Detailed debugging info
- ✅ **Docker Support**: Containerized deployment

---

**The Fuego daemon integration provides a production-ready solution for real dual-chain mining, combining the efficiency of unified management with the power of actual Fuego blockchain operations.**
