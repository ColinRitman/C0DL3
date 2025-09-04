# C0DL3 Project Structure

This document outlines the current structure of the C0DL3 (COLD L3) rollup implementations project.

## 📁 Directory Structure

```
codl3-implementations/
├── README.md                    # Main project documentation
├── PROJECT_STRUCTURE.md         # This file
├── c0dl3-zksync/               # zkSync Hyperchains implementation
│   ├── Cargo.toml              # Rust package manifest
│   ├── src/
│   │   └── main.rs             # Main application code
│   ├── target/                 # Build artifacts
│   └── README.md               # zkSync-specific documentation
└── c0dl3-arbitrum/             # Arbitrum Orbit implementation
    ├── Cargo.toml              # Rust package manifest
    ├── src/
    │   └── main.rs             # Main application code
    ├── target/                 # Build artifacts
    └── README.md               # Arbitrum-specific documentation
```

## 🔄 Naming Convention Updates

### Before (CODL3)
- Package names: `codl3-zksync`, `codl3-arbitrum`
- Binary names: `codl3-zksync`, `codl3-arbitrum`
- Directory names: `codl3-zksync`, `codl3-arbitrum`
- Internal references: `CODL3`, `CODL3ZkSyncNode`, `CODL3ArbitrumNode`

### After (C0DL3)
- Package names: `c0dl3-zksync`, `c0dl3-arbitrum`
- Binary names: `c0dl3-zksync`, `c0dl3-arbitrum`
- Directory names: `c0dl3-zksync`, `c0dl3-arbitrum`
- Internal references: `C0DL3`, `C0DL3ZkSyncNode`, `C0DL3ArbitrumNode`

## 📦 Package Details

### c0dl3-zksync
- **Description**: C0DL3 - zkSync Hyperchains Implementation
- **Binary Name**: `c0dl3-zksync`
- **Features**: ZK-rollup, ZK proofs, custom gas tokens
- **Ports**: P2P (30333), RPC (9944)
- **L2 RPC**: http://localhost:3050 (zkSync)

### c0dl3-arbitrum
- **Description**: C0DL3 - Arbitrum Orbit Implementation
- **Binary Name**: `c0dl3-arbitrum`
- **Features**: Optimistic rollup, fraud proofs, challenge period
- **Ports**: P2P (30333), RPC (9944)
- **L2 RPC**: http://localhost:42161 (Arbitrum)

## 🏗️ Architecture Components

### Common to Both Implementations
- **P2P Networking**: libp2p with mDNS and Kademlia DHT
- **RPC Server**: Axum HTTP server with comprehensive API
- **State Management**: Thread-safe state with Arc<Mutex<>>
- **Configuration**: Flexible configuration system
- **Logging**: Structured logging with tracing
- **Dual Mining**: Fuego + C0DL3 coordination

### zkSync-Specific
- **Proof System**: ZK proofs (STARK)
- **Security Model**: Cryptographic verification
- **Finality**: ZK proof finality
- **Gas Token**: HEAT on L1

### Arbitrum-Specific
- **Proof System**: Fraud proofs
- **Security Model**: Challenge period (7 days)
- **Finality**: Optimistic finality
- **Data Availability**: Committee-based L1 posting

## 🚀 Build and Run Commands

### Building
```bash
# zkSync
cd c0dl3-zksync && cargo build --release

# Arbitrum
cd c0dl3-arbitrum && cargo build --release
```

### Running
```bash
# zkSync
cd c0dl3-zksync && cargo run -- --log-level debug

# Arbitrum
cd c0dl3-arbitrum && cargo run -- --log-level debug
```

## 📊 Current Status

### ✅ Completed
- [x] C0DL3 naming convention applied throughout
- [x] Directory structure updated
- [x] Package manifests updated
- [x] Basic implementations functional
- [x] P2P networking implemented
- [x] RPC server implemented
- [x] Documentation updated

### 🔄 In Progress
- [ ] Full feature implementation
- [ ] Real proof generation
- [ ] Database persistence
- [ ] Advanced monitoring

### 📋 Planned
- [ ] Integration with Fuego daemon
- [ ] HEAT token integration
- [ ] Validator staking contracts
- [ ] Production deployment

## 🔧 Development Notes

### Naming Consistency
- All new code should use `C0DL3` naming
- Binary names use lowercase `c0dl3-*`
- Package names use lowercase `c0dl3-*`
- Internal structs use `C0DL3*` naming

### Code Organization
- Each implementation is self-contained
- Common patterns shared between implementations
- Configuration-driven architecture
- Async-first design with Tokio

### Testing
- Both implementations include basic functionality tests
- P2P networking can be tested locally
- RPC endpoints can be tested with HTTP clients
- Integration testing planned for future

## 📚 Documentation

- **README.md**: Main project overview and getting started
- **PROJECT_STRUCTURE.md**: This structural overview
- **c0dl3-zksync/README.md**: zkSync-specific details
- **c0dl3-arbitrum/README.md**: Arbitrum-specific details

## 🤝 Contributing

When contributing to this project:
1. Use C0DL3 naming convention
2. Follow the established architecture patterns
3. Add appropriate documentation
4. Include tests for new functionality
5. Update relevant README files

---

**Last Updated**: August 28, 2024
**Version**: 1.0.0
**Status**: Development
