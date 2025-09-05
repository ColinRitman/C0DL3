# Unified CLI Daemon

The Unified CLI Daemon is a **3rd daemon wrapper** that provides an interactive command-line interface for managing both zkC0DL3 and Fuego daemons. It's specifically designed for miners and validators who need a single, unified interface for monitoring and controlling both chains.

## 🎯 **Overview**

The CLI daemon acts as a **wrapper service** that:
- **Pulls data** from both running daemons (zkC0DL3 and Fuego)
- **Provides interactive CLI** for mining and validation management
- **Monitors status** of both chains in real-time
- **Manages validators** (Eldorados for C0DL3, Elderfiers for Fuego)
- **Controls mining** operations across both networks

## 🏗️ **Architecture**

```
┌─────────────────────────────────────────────────────────────┐
│                Unified CLI Daemon                           │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Interactive   │  │   Status        │  │   Mining    │ │
│  │   CLI Interface │  │   Monitor       │  │   Manager    │ │
│  │                 │  │                 │  │             │ │
│  │ • Commands      │  │ • Real-time     │  │ • Start/Stop│ │
│  │ • Help System   │  │   Updates       │  │ • Statistics│ │
│  │ • User Input    │  │ • Health Checks │  │ • Efficiency│ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│           │                     │                 │        │
│           └─────────────────────┼─────────────────┘        │
│                                 │                         │
│                    ┌─────────────────┐                    │
│                    │  Data Sources   │                    │
│                    │                 │                    │
│                    │ • zkC0DL3 RPC   │                    │
│                    │ • Fuego RPC     │                    │
│                    │ • Local Daemons │                    │
│                    └─────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 **Getting Started**

### **1. Start CLI Daemon**

```bash
# Method 1: Using startup script (recommended)
./scripts/start-cli-daemon.sh

# Method 2: Direct CLI
cargo run -- --cli-mode true --interactive-mode true

# Method 3: Background mode
cargo run -- --cli-mode true --interactive-mode false
```

### **2. Interactive Commands**

Once started, you'll see the interactive prompt:
```
unified-cli> 
```

## 📋 **Available Commands**

### **Status & Monitoring**
```bash
status, s                    # Show system status
network, n                   # Show network statistics
```

### **Mining Management**
```bash
mining, m                    # Show mining statistics
mining start                 # Start mining on both chains
mining stop                  # Stop mining on both chains
mining stats                 # Show detailed mining stats
```

### **Validator Management**
```bash
validators, v                # List all validators
validators list              # List all validators
validators info <address>    # Show validator details
validators stake <addr> <amount> # Stake tokens to validator
```

### **Daemon Management**
```bash
daemon restart <c0dl3|fuego> # Restart daemon
daemon stop <c0dl3|fuego>     # Stop daemon
daemon start <c0dl3|fuego>    # Start daemon
```

### **Utility Commands**
```bash
help, h, ?                   # Show help
exit, quit, q                 # Exit CLI
```

## 📊 **Status Display**

### **System Status**
```
📊 SYSTEM STATUS
═══════════════════════════════════════════════════════════════
🔧 DAEMON STATUS:
   C0DL3: Running
   Fuego: Running

⛏️  MINING STATUS:
   C0DL3 Mining: ✅ Active
   Fuego Mining: ✅ Active
   C0DL3 Hash Rate: 1000000 H/s
   Fuego Hash Rate: 500000 H/s
   Total Rewards: 19000000 tokens

🛡️  VALIDATION STATUS:
   C0DL3 Validation: ✅ Active
   Fuego Validation: ✅ Active
   Total Stake: 7500000 tokens

🌐 NETWORK STATS:
   C0DL3: 25 peers, height 150, difficulty 1000
   Fuego: 15 peers, height 12, difficulty 2000
   Uptime: 3600 seconds
═══════════════════════════════════════════════════════════════
```

### **Mining Statistics**
```
⛏️  MINING STATISTICS
═══════════════════════════════════════════════════════════════
C0DL3 Mining:
   Status: ✅ Active
   Hash Rate: 1000000 H/s
   Blocks Mined: 150

Fuego Mining:
   Status: ✅ Active
   Hash Rate: 500000 H/s
   Blocks Mined: 12

Combined:
   Total Rewards: 19000000 tokens
   Mining Efficiency: 75.0%
═══════════════════════════════════════════════════════════════
```

### **Validator Information**
```
🛡️  VALIDATORS
═══════════════════════════════════════════════════════════════
C0DL3 ELDORADOS:
   1. 0x1111111111111111111111111111111111111111 (Active)
      Stake: 1000000 tokens
      Uptime: 3600 seconds
      Blocks Validated: 150
      Rewards: 50000 tokens
      Reputation: 0.95

Fuego ELDERFIERS:
   1. 0x3333333333333333333333333333333333333333 (Active)
      Stake: 1500000 tokens
      Uptime: 1800 seconds
      Blocks Validated: 75
      Rewards: 25000 tokens
      Elderfier Level: 3
      Reputation: 0.92
═══════════════════════════════════════════════════════════════
```

## ⚙️ **Configuration**

### **CLI Options**

```bash
# CLI Mode Configuration
--cli-mode true                    # Enable CLI daemon mode
--interactive-mode true            # Enable interactive CLI
--status-refresh-interval 5        # Status refresh interval (seconds)

# Data Directories
--data-dir ./data                  # C0DL3 data directory
--fuego-data-dir ./fuego-data      # Fuego data directory

# RPC Endpoints
--l2-rpc-url http://localhost:9944 # C0DL3 RPC URL
--fuego-rpc-url http://localhost:8546 # Fuego RPC URL
```

### **Environment Variables**

```bash
# CLI Configuration
CLI_MODE=true
INTERACTIVE_MODE=true
STATUS_REFRESH_INTERVAL=5

# Data Directories
DATA_DIR=./data
FUEGO_DATA_DIR=./fuego-data

# RPC Endpoints
C0DL3_RPC_URL=http://localhost:9944
FUEGO_RPC_URL=http://localhost:8546
```

## 🔧 **Features**

### **Real-time Monitoring**
- **Live Status Updates**: Real-time daemon status monitoring
- **Health Checks**: Continuous health monitoring of both daemons
- **Performance Metrics**: Mining efficiency and network statistics
- **Validator Tracking**: Real-time validator status and rewards

### **Interactive Management**
- **Command-line Interface**: Easy-to-use interactive commands
- **Help System**: Built-in help and command documentation
- **Auto-completion**: Command suggestions and shortcuts
- **Status Display**: Rich, formatted status displays

### **Mining Control**
- **Dual-chain Mining**: Control mining on both C0DL3 and Fuego
- **Mining Statistics**: Detailed mining performance metrics
- **Efficiency Monitoring**: Track mining efficiency across chains
- **Reward Tracking**: Monitor rewards from both networks

### **Validator Management**
- **Eldorado Monitoring**: Track C0DL3 Eldorado validators
- **Elderfier Monitoring**: Track Fuego Elderfier validators
- **Stake Management**: Stake tokens to validators
- **Reputation Tracking**: Monitor validator reputation scores

## 🎮 **Interactive Examples**

### **Starting Mining**
```bash
unified-cli> mining start
🚀 Starting mining on both chains...
✅ Mining started successfully
   C0DL3: Mining blocks every 30 seconds
   Fuego: Mining blocks every 480 seconds
```

### **Checking Validator Status**
```bash
unified-cli> validators info 0x1111111111111111111111111111111111111111
🛡️  C0DL3 ELDORADO VALIDATOR INFO
═══════════════════════════════════════════════════════════════
Address: 0x1111111111111111111111111111111111111111
Status: Active
Stake: 1000000 tokens
Uptime: 3600 seconds
Blocks Validated: 150
Rewards Earned: 50000 tokens
Reputation Score: 0.95
═══════════════════════════════════════════════════════════════
```

### **Staking to Validator**
```bash
unified-cli> validators stake 0x1111111111111111111111111111111111111111 500000
💰 Staking 500000 tokens to validator 0x1111111111111111111111111111111111111111
✅ Stake transaction submitted successfully
   Validator: 0x1111111111111111111111111111111111111111
   Amount: 500000 tokens
```

## 🐳 **Docker Support**

### **Docker Compose**
```yaml
services:
  cli-daemon:
    build: .
    container_name: c0dl3-cli-daemon
    environment:
      - CLI_MODE=true
      - INTERACTIVE_MODE=true
      - STATUS_REFRESH_INTERVAL=5
      - DATA_DIR=/app/data
      - FUEGO_DATA_DIR=/app/fuego-data
      - C0DL3_RPC_URL=http://c0dl3-daemon:9944
      - FUEGO_RPC_URL=http://fuego-daemon:8546
    volumes:
      - ./data:/app/data
      - ./fuego-data:/app/fuego-data
    stdin_open: true
    tty: true
    depends_on:
      - c0dl3-daemon
      - fuego-daemon
```

### **Docker Run**
```bash
docker run -it \
  --name c0dl3-cli-daemon \
  -e CLI_MODE=true \
  -e INTERACTIVE_MODE=true \
  -v ./data:/app/data \
  -v ./fuego-data:/app/fuego-data \
  c0dl3-cli-daemon:latest
```

## 🔍 **Troubleshooting**

### **Common Issues**

1. **Daemon Connection Failed**
   ```bash
   # Check if daemons are running
   unified-cli> daemon restart c0dl3
   unified-cli> daemon restart fuego
   ```

2. **RPC Connection Issues**
   ```bash
   # Check RPC endpoints
   curl http://localhost:9944/health
   curl -X POST http://localhost:8546 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"getinfo","params":[],"id":1}'
   ```

3. **Interactive Mode Not Working**
   ```bash
   # Start with interactive mode
   cargo run -- --cli-mode true --interactive-mode true
   ```

### **Debug Mode**
```bash
# Enable debug logging
RUST_LOG=debug cargo run -- --cli-mode true
```

## 📈 **Benefits**

### **For Miners**
- ✅ **Unified Interface**: Single CLI for both chains
- ✅ **Real-time Monitoring**: Live mining statistics
- ✅ **Easy Control**: Simple commands to start/stop mining
- ✅ **Efficiency Tracking**: Monitor mining efficiency

### **For Validators**
- ✅ **Validator Management**: Track Eldorados and Elderfiers
- ✅ **Stake Management**: Easy staking to validators
- ✅ **Reputation Monitoring**: Track validator reputation
- ✅ **Reward Tracking**: Monitor validation rewards

### **For Operators**
- ✅ **Status Monitoring**: Real-time daemon health
- ✅ **Network Statistics**: Live network information
- ✅ **Daemon Control**: Start/stop/restart daemons
- ✅ **Interactive Management**: User-friendly interface

## 🎉 **Use Cases**

### **Mining Operations**
- Monitor mining performance across both chains
- Control mining operations with simple commands
- Track rewards and efficiency in real-time
- Manage mining resources and optimization

### **Validator Operations**
- Monitor Eldorado and Elderfier validators
- Manage stakes and validator participation
- Track validation rewards and reputation
- Oversee validator network health

### **System Administration**
- Monitor daemon health and performance
- Manage daemon lifecycle (start/stop/restart)
- Track network statistics and connectivity
- Oversee system resources and efficiency

---

**The Unified CLI Daemon provides a powerful, interactive interface for managing both zkC0DL3 and Fuego operations, making it the perfect tool for miners, validators, and system administrators who need unified control over both networks.**
