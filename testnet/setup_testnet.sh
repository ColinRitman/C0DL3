#!/bin/bash

# zkC0DL3 Testnet Setup Script
# This script sets up a complete testnet environment for zkC0DL3

set -e

echo "🚀 Setting up zkC0DL3 Testnet Environment"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
TESTNET_NAME="zkc0dl3-testnet"
TESTNET_PORT=8545
P2P_PORT=30303
RPC_PORT=8545
WS_PORT=8546
DATA_DIR="./testnet_data"
GENESIS_FILE="./genesis.json"
CONFIG_FILE="./testnet_config.toml"

# Create testnet directory structure
echo -e "${BLUE}📁 Creating testnet directory structure...${NC}"
mkdir -p $DATA_DIR/{nodes,logs,configs,contracts}
mkdir -p $DATA_DIR/nodes/{node1,node2,node3}

# Generate testnet configuration
echo -e "${BLUE}⚙️  Generating testnet configuration...${NC}"
cat > $CONFIG_FILE << EOF
[testnet]
name = "$TESTNET_NAME"
version = "0.1.0"
description = "zkC0DL3 Testnet - Production STARK Implementation"

[network]
p2p_port = $P2P_PORT
rpc_port = $RPC_PORT
ws_port = $WS_PORT
max_peers = 50
discovery_enabled = true

[mining]
algorithm = "cn-upx2"
target_block_time = 60
difficulty_adjustment_interval = 2016
merge_mining_enabled = true
merge_mining_interval = 60

[privacy]
stark_proofs_enabled = true
address_encryption_enabled = true
transaction_privacy_enabled = true
timing_privacy_enabled = true

[security]
vulnerability_fixes_enabled = true
secure_rng_enabled = true
input_validation_enabled = true
timing_attack_protection_enabled = true

[performance]
parallel_processing_enabled = true
proof_caching_enabled = true
memory_optimization_enabled = true
cpu_optimization_enabled = true

[xfg_integration]
winterfell_starks_enabled = true
fuego_sync_enabled = true
heat_token_bridging_enabled = true
cold_token_generation_enabled = true
EOF

# Generate genesis block configuration
echo -e "${BLUE}🏗️  Generating genesis block...${NC}"
cat > $GENESIS_FILE << EOF
{
  "config": {
    "chainId": 1337,
    "homesteadBlock": 0,
    "eip150Block": 0,
    "eip155Block": 0,
    "eip158Block": 0,
    "byzantiumBlock": 0,
    "constantinopleBlock": 0,
    "petersburgBlock": 0,
    "istanbulBlock": 0,
    "berlinBlock": 0,
    "londonBlock": 0,
    "arrowGlacierBlock": 0,
    "grayGlacierBlock": 0,
    "shanghaiTime": 0,
    "cancunTime": 0,
    "pragueTime": 0,
    "viennaTime": 0,
    "chainId": 1337,
    "networkId": 1337
  },
  "difficulty": "0x400",
  "gasLimit": "0x1C9C380",
  "alloc": {
    "0x742d35Cc6634C0532925a3b8D0C0C0C0C0C0C0C0": {
      "balance": "0x3635C9ADC5DEA00000"
    },
    "0x8ba1f109551bD432803012645Hac136c0C0C0C0C0": {
      "balance": "0x3635C9ADC5DEA00000"
    }
  },
  "extraData": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "gasUsed": "0x0",
  "number": "0x0",
  "gasUsed": "0x0",
  "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
  "timestamp": "0x0"
}
EOF

# Create node configurations
echo -e "${BLUE}🔧 Creating node configurations...${NC}"

# Node 1 Configuration
cat > $DATA_DIR/nodes/node1/config.toml << EOF
[node]
id = "node1"
name = "zkC0DL3 Testnet Node 1"
version = "0.1.0"

[network]
listen_address = "127.0.0.1:$P2P_PORT"
rpc_address = "127.0.0.1:$RPC_PORT"
ws_address = "127.0.0.1:$WS_PORT"
max_peers = 25

[mining]
enabled = true
coinbase = "0x742d35Cc6634C0532925a3b8D0C0C0C0C0C0C0C0"
target_block_time = 60

[privacy]
stark_proofs_enabled = true
address_encryption_enabled = true
transaction_privacy_enabled = true

[security]
vulnerability_fixes_enabled = true
secure_rng_enabled = true
input_validation_enabled = true

[performance]
parallel_processing_enabled = true
proof_caching_enabled = true
memory_optimization_enabled = true
EOF

# Node 2 Configuration
cat > $DATA_DIR/nodes/node2/config.toml << EOF
[node]
id = "node2"
name = "zkC0DL3 Testnet Node 2"
version = "0.1.0"

[network]
listen_address = "127.0.0.1:$((P2P_PORT + 1))"
rpc_address = "127.0.0.1:$((RPC_PORT + 1))"
ws_address = "127.0.0.1:$((WS_PORT + 1))"
max_peers = 25

[mining]
enabled = true
coinbase = "0x8ba1f109551bD432803012645Hac136c0C0C0C0C0"
target_block_time = 60

[privacy]
stark_proofs_enabled = true
address_encryption_enabled = true
transaction_privacy_enabled = true

[security]
vulnerability_fixes_enabled = true
secure_rng_enabled = true
input_validation_enabled = true

[performance]
parallel_processing_enabled = true
proof_caching_enabled = true
memory_optimization_enabled = true
EOF

# Node 3 Configuration
cat > $DATA_DIR/nodes/node3/config.toml << EOF
[node]
id = "node3"
name = "zkC0DL3 Testnet Node 3"
version = "0.1.0"

[network]
listen_address = "127.0.0.1:$((P2P_PORT + 2))"
rpc_address = "127.0.0.1:$((RPC_PORT + 2))"
ws_address = "127.0.0.1:$((WS_PORT + 2))"
max_peers = 25

[mining]
enabled = false
coinbase = "0x0000000000000000000000000000000000000000"
target_block_time = 60

[privacy]
stark_proofs_enabled = true
address_encryption_enabled = true
transaction_privacy_enabled = true

[security]
vulnerability_fixes_enabled = true
secure_rng_enabled = true
input_validation_enabled = true

[performance]
parallel_processing_enabled = true
proof_caching_enabled = true
memory_optimization_enabled = true
EOF

# Create startup scripts
echo -e "${BLUE}📜 Creating startup scripts...${NC}"

# Node 1 startup script
cat > $DATA_DIR/nodes/node1/start.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting zkC0DL3 Testnet Node 1..."
cd "$(dirname "$0")"
cargo run --bin codl3-zksync -- --config config.toml --data-dir ./data --log-level info
EOF

# Node 2 startup script
cat > $DATA_DIR/nodes/node2/start.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting zkC0DL3 Testnet Node 2..."
cd "$(dirname "$0")"
cargo run --bin codl3-zksync -- --config config.toml --data-dir ./data --log-level info
EOF

# Node 3 startup script
cat > $DATA_DIR/nodes/node3/start.sh << 'EOF'
#!/bin/bash
echo "🚀 Starting zkC0DL3 Testnet Node 3..."
cd "$(dirname "$0")"
cargo run --bin codl3-zksync -- --config config.toml --data-dir ./data --log-level info
EOF

# Make scripts executable
chmod +x $DATA_DIR/nodes/*/start.sh

# Create testnet management script
cat > $DATA_DIR/manage_testnet.sh << 'EOF'
#!/bin/bash

# zkC0DL3 Testnet Management Script

case "$1" in
    start)
        echo "🚀 Starting zkC0DL3 Testnet..."
        echo "Starting Node 1..."
        ./nodes/node1/start.sh &
        sleep 5
        echo "Starting Node 2..."
        ./nodes/node2/start.sh &
        sleep 5
        echo "Starting Node 3..."
        ./nodes/node3/start.sh &
        echo "✅ All nodes started!"
        ;;
    stop)
        echo "🛑 Stopping zkC0DL3 Testnet..."
        pkill -f "codl3-zksync"
        echo "✅ All nodes stopped!"
        ;;
    status)
        echo "📊 zkC0DL3 Testnet Status:"
        ps aux | grep "codl3-zksync" | grep -v grep
        ;;
    logs)
        echo "📋 Viewing testnet logs..."
        tail -f logs/*.log
        ;;
    *)
        echo "Usage: $0 {start|stop|status|logs}"
        exit 1
        ;;
esac
EOF

chmod +x $DATA_DIR/manage_testnet.sh

# Create testnet monitoring script
cat > $DATA_DIR/monitor_testnet.sh << 'EOF'
#!/bin/bash

# zkC0DL3 Testnet Monitoring Script

echo "📊 zkC0DL3 Testnet Monitor"
echo "=========================="

while true; do
    clear
    echo "📊 zkC0DL3 Testnet Monitor - $(date)"
    echo "=========================="
    
    # Check node processes
    echo "🔍 Node Status:"
    ps aux | grep "codl3-zksync" | grep -v grep | while read line; do
        echo "  ✅ $line"
    done
    
    # Check network connections
    echo ""
    echo "🌐 Network Status:"
    netstat -an | grep -E "(30303|8545|8546)" | head -10
    
    # Check disk usage
    echo ""
    echo "💾 Disk Usage:"
    du -sh . 2>/dev/null || echo "  No data yet"
    
    # Check memory usage
    echo ""
    echo "🧠 Memory Usage:"
    ps aux | grep "codl3-zksync" | grep -v grep | awk '{sum+=$6} END {print "  Total RSS: " sum/1024 " MB"}'
    
    sleep 10
done
EOF

chmod +x $DATA_DIR/monitor_testnet.sh

# Create testnet documentation
cat > $DATA_DIR/README.md << 'EOF'
# zkC0DL3 Testnet

This directory contains the complete testnet environment for zkC0DL3.

## Quick Start

1. **Start the testnet:**
   ```bash
   ./manage_testnet.sh start
   ```

2. **Monitor the testnet:**
   ```bash
   ./monitor_testnet.sh
   ```

3. **Stop the testnet:**
   ```bash
   ./manage_testnet.sh stop
   ```

## Configuration

- **Node 1**: Mining enabled, RPC on port 8545
- **Node 2**: Mining enabled, RPC on port 8546  
- **Node 3**: Mining disabled, RPC on port 8547

## Features Enabled

- ✅ Production STARK Proofs
- ✅ Address Encryption (ChaCha20Poly1305)
- ✅ Transaction Privacy
- ✅ Timing Privacy
- ✅ Security Vulnerability Fixes
- ✅ Performance Optimization
- ✅ XFG Winterfell Integration
- ✅ CN-UPX/2 Mining Algorithm
- ✅ Merge Mining with Fuego
- ✅ HEAT Token Bridging
- ✅ COLD Token Generation

## Network Details

- **Chain ID**: 1337
- **Block Time**: 60 seconds
- **Mining Algorithm**: CN-UPX/2
- **Privacy**: STARK-based zero-knowledge proofs
- **Security**: 100% vulnerability score

## Testing

Use the following RPC endpoints for testing:

- Node 1: `http://127.0.0.1:8545`
- Node 2: `http://127.0.0.1:8546`
- Node 3: `http://127.0.0.1:8547`

## Logs

All node logs are stored in the `logs/` directory.
EOF

echo -e "${GREEN}✅ zkC0DL3 Testnet setup complete!${NC}"
echo ""
echo -e "${YELLOW}📋 Next steps:${NC}"
echo "1. cd $DATA_DIR"
echo "2. ./manage_testnet.sh start"
echo "3. ./monitor_testnet.sh"
echo ""
echo -e "${BLUE}🔗 Testnet will be available at:${NC}"
echo "   Node 1: http://127.0.0.1:8545"
echo "   Node 2: http://127.0.0.1:8546"
echo "   Node 3: http://127.0.0.1:8547"
echo ""
echo -e "${GREEN}🚀 Ready to launch zkC0DL3 testnet!${NC}"
