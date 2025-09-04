# C0DL3 - COLD L3 Rollup Implementations

This repository contains implementations of the C0DL3 (COLD L3) rollup system for both zkSync Hyperchains and Arbitrum Orbit, featuring dual mining with the Fuego blockchain.

## 🏗️ Architecture Overview

C0DL3 is a Layer 3 rollup system that enables validators to run both COLD L3 and Fuego daemons simultaneously, creating a dual mining ecosystem:

- **Fuego Mining**: Earns XFG block rewards, Fuego transaction/deposit/burn fees
- **C0DL3 Mining**: Earns HEAT gas fees only (no block rewards, CD rewards for LP providers)

### Validator Economics

- **Staking Requirement**: 800 billion HEAT tokens
- **Revenue Sources**:
  - Fuego PoW block fees
  - Dynamic XFG block rewards
  - C0DL3 gas fees
  - Eldernode transaction fees
  - Other block fees

### Slashing Conditions

- **Critical**: Double signing, invalid state transitions
- **Moderate**: Inactivity, invalid proofs
- **Minor**: Performance issues, network violations

## 🚀 Implementations

### 1. C0DL3 zkSync Implementation

**Location**: `c0dl3-zksync/`

**Features**:
- ZK-rollup framework with zkSync Hyperchains
- Custom ERC20 gas token support (HEAT on L1)
- ZK proofs for security and finality
- Permissionless validator system
- Real P2P networking with libp2p
- RESTful RPC server with comprehensive API
- Block synchronization and consensus
- Bridge monitoring and dual mining coordination

**Technology Stack**:
- **Language**: Rust
- **Runtime**: Tokio async runtime
- **Networking**: libp2p (mDNS, Kademlia DHT)
- **RPC Server**: Axum HTTP framework
- **Serialization**: Serde
- **Logging**: Tracing

**API Endpoints**:
- `GET /` - Root endpoint
- `GET /health` - Health check
- `GET /status` - Node status
- `GET /peers` - Connected peers
- `GET /transactions` - Pending transactions
- `POST /transactions` - Add transaction
- `GET /blocks` - Latest blocks
- `GET /blocks/:height` - Block by height
- `GET /mining/rewards` - Mining rewards
- `GET /mining/stats` - Mining statistics
- `GET /network/info` - Network information

### 2. C0DL3 Arbitrum Implementation

**Location**: `c0dl3-arbitrum/`

**Features**:
- Optimistic rollup with Arbitrum Orbit
- Custom gas token support (HEAT)
- Fraud proof system with challenge period
- Data availability committee
- Real P2P networking with libp2p
- RESTful RPC server with comprehensive API
- Block synchronization and consensus
- Bridge monitoring and dual mining coordination

**Technology Stack**:
- **Language**: Rust
- **Runtime**: Tokio async runtime
- **Networking**: libp2p (mDNS, Kademlia DHT)
- **RPC Server**: Axum HTTP framework
- **Serialization**: Serde
- **Logging**: Tracing

**Arbitrum-Specific Features**:
- Optimistic rollup with 7-day challenge period
- Data availability committee for L1 data posting
- Fraud proof generation and verification
- Orbit chain ID 42161 configuration

## 🛠️ Getting Started

### Prerequisites

- Rust 1.70+ and Cargo
- Git
- Network access for dependencies

### Building

#### zkSync Implementation

```bash
cd c0dl3-zksync
cargo build --release
```

#### Arbitrum Implementation

```bash
cd c0dl3-arbitrum
cargo build --release
```

### Running

#### zkSync Node

```bash
cd c0dl3-zksync
cargo run -- --log-level debug
```

**CLI Options**:
- `--log-level`: Logging level (default: info)
- `--data-dir`: Data directory (default: ./data)
- `--l1-rpc-url`: L1 RPC URL (default: http://localhost:8545)
- `--l2-rpc-url`: L2 RPC URL (default: http://localhost:3050)
- `--fuego-rpc-url`: Fuego RPC URL (default: http://localhost:8080)
- `--p2p-port`: P2P port (default: 30333)
- `--rpc-port`: RPC port (default: 9944)
- `--validator-address`: Validator address (optional)
- `--gas-token-address`: Gas token address (optional)

#### Arbitrum Node

```bash
cd c0dl3-arbitrum
cargo run -- --log-level debug
```

**CLI Options**:
- `--log-level`: Logging level (default: info)
- `--data-dir`: Data directory (default: ./data)
- `--l1-rpc-url`: L1 RPC URL (default: http://localhost:8545)
- `--l2-rpc-url`: L2 RPC URL (default: http://localhost:42161)
- `--fuego-rpc-url`: Fuego RPC URL (default: http://localhost:8080)
- `--p2p-port`: P2P port (default: 30333)
- `--rpc-port`: RPC port (default: 9944)
- `--validator-address`: Validator address (optional)
- `--gas-token-address`: Gas token address (optional)

## 🔧 Configuration

### Network Configuration

Both implementations support:
- P2P networking on configurable ports
- Bootstrap peer discovery
- Maximum peer limits
- Custom data directories

### RPC Configuration

- HTTP server with CORS support
- Configurable host and port
- Connection limits
- JSON API endpoints

### Bridge Configuration

- L1 ↔ L2 bridge monitoring
- Configurable confirmation blocks
- Polling intervals
- Enable/disable options

### Consensus Configuration

- Validator count
- Block time settings
- Finality thresholds
- Challenge periods (Arbitrum)

## 🌐 Network Architecture

### P2P Networking

- **Transport**: TCP with noise encryption
- **Multiplexing**: Yamux
- **Discovery**: mDNS for local peers, Kademlia DHT for global
- **Peer Management**: Automatic peer discovery and management

### RPC Server

- **Framework**: Axum HTTP server
- **CORS**: Configurable cross-origin resource sharing
- **State Management**: Thread-safe shared state with Arc<Mutex<>>
- **API**: RESTful endpoints for all node operations

## 🔒 Security Features

### zkSync Implementation

- ZK proofs for state transitions
- Cryptographic verification
- Permissionless validator system
- Secure P2P communication

### Arbitrum Implementation

- Fraud proof system
- Challenge period protection
- Data availability committee
- Optimistic rollup security

## 📊 Monitoring and Metrics

Both implementations provide:
- Real-time node status
- Mining statistics
- Network peer information
- Transaction processing metrics
- Block synchronization status

## 🚧 Development Status

### Current Features

✅ **Implemented**:
- Core node architecture
- P2P networking stack
- RPC server with comprehensive API
- Basic transaction processing
- Block creation and management
- Dual mining coordination
- Bridge monitoring framework
- Consensus mechanism framework

🔄 **In Progress**:
- Real ZK proof generation (zkSync)
- Real fraud proof generation (Arbitrum)
- Database persistence
- Advanced monitoring and logging
- Performance optimization

📋 **Planned**:
- Integration with actual Fuego daemon
- HEAT token integration
- Validator staking contracts
- Advanced consensus algorithms
- Security hardening
- Production deployment tools

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with C0DL3 naming
4. Add tests and documentation
5. Submit a pull request

## 📄 License

C0DL3 - COLD L3 Rollup Implementations - C0DL3 Team

## 🔗 Related Projects

- [Fuego Blockchain](https://github.com/usexfg/fuego.git) - Base layer blockchain
- [zkSync Hyperchains](https://docs.zksync.io/build/developer-reference/hyperchains/) - ZK-rollup framework
- [Arbitrum Orbit](https://docs.arbitrum.io/launch-orbit-chain/) - Optimistic rollup framework

## 📞 Support

For questions and support:
- Create an issue in this repository
- Join the C0DL3 community discussions
- Review the implementation documentation

---

**Note**: This is a development implementation. For production use, additional security audits, testing, and hardening are required.


