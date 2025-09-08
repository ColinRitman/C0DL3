# 🚀 zkC0DL3 node - zkSync Hyperchain Node

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Production Ready](https://img.shields.io/badge/production-ready-green.svg)](#production-deployment)

**zkC0DL3** is a zkSync Hyperchain rollup with zkSTARK transaction privacy and Fuego L1 compatibility for merge mining operations.

## ✨ **Key Features**

### 🔐 **Advanced Privacy**
- **User-level privacy** with transaction encryption
- **Address encryption** using ChaCha20Poly1305
- **Timing privacy** with encrypted timestamps
- **STARK proof generation** for privacy preservation
- **Cross-chain privacy** coordination methods

### ⛏️ **Mining & Consensus**
- **CN-UPX/2 Algorithm**: Full implementation for Fuego compatibility
- **60-second block time** for optimal stability
- **Merge mining** with Fuego L1 every 60 seconds, 8:1 ratio
- **Dual mining support** for Fuego miners
- **Difficulty adjustment** every 10 blocks

### 🌐 **Networking**
- **P2P Network**: libp2p 0.56.0 with Kademlia DHT
- **Pub/Sub Messaging**: Floodsub protocol
- **Transport Security**: Noise protocol with Yamux multiplexing
- **Bootstrap Peers**: Configurable peer discovery

### 🔗 **Cross-Chain Integration**
- **Fuego L1 Compatibility**: Full CNUPX/2 merge mining support
- **Merge Mining**: AuxPoW integration
- **Bridge Management**: Multi-blockchain support
- **Unified ZK Proofs**: Cross-chain privacy preservation

## 🚀 **Quick Start**

### **Prerequisites**
- **Rust**: 1.70+ (latest stable)
- **Memory**: 16GB+ RAM (for CN-UPX/2 mining)
- **Storage**: SSD with 100GB+ free space
- **Network**: Stable internet connection

### **Installation**
```bash
# Clone the repository
git clone https://github.com/ColinRitman/C0DL3.git
cd C0DL3/c0dl3-zksync

# Build the project
cargo build --release

# Run with default configuration
cargo run --release
```

## ⚙️ **Production Configuration**

### **Mining Settings**
- **Block Time**: 60 seconds
- **Mining Algorithm**: CN-UPX/2 (Standard)
- **Memory Usage**: 2MB scratchpad
- **Hash Rate**: ~11-13 seconds per hash
- **Difficulty**: Auto-adjusting every 10 blocks

### **Merge Mining Settings**
- **Merge Mining Interval**: 60 seconds
- **Fuego Block Time**: 480 seconds (8 minutes)
- **Block Ratio**: 8:1 (8 zkC0DL3 blocks per Fuego block)
- **AuxPoW Tag**: "C0DL3-MERGE-MINING"

## 🔧 **API Endpoints**

### **RPC API (Port 8080)**
- `GET /stats` - Network statistics
- `GET /network/info` - Network information
- `GET /blocks` - Block information
- `GET /transactions` - Transaction data
- `GET /merge-mining/stats` - Merge mining statistics
- `GET /privacy/status` - Privacy system status

## 🧪 **Testing**

### **Run All Tests**
```bash
# Basic functionality test
cargo run --example simple_test

# CN-UPX/2 algorithm test
cargo run --example cn_upx2_test

# Multi-mode compatibility test
cargo run --example cn_upx2_multimode_test

# STARK proof system test
cargo run --example production_stark_test

# RPC integration test
cargo run --example rpc_integration_test
```

## 📊 **Performance Metrics**

### **Mining Performance**
- **Block Time**: 60 seconds
- **Mining Algorithm**: CN-UPX/2 (Standard)
- **Memory Usage**: 2MB scratchpad
- **Hash Rate**: ~11-13 seconds per hash
- **Difficulty**: Auto-adjusting every 10 blocks

### **Network Performance**
- **Blocks per Hour**: 60 blocks
- **Blocks per Day**: 1,440 blocks
- **Theoretical TPS**: ~100 transactions per block
- **Maximum Daily Transactions**: ~144,000
- **Batch Processing**: Up to 100 transactions per batch

### **Trading Performance**
- **Multiple positions per minute**: ✅ Supported
- **High-frequency trading**: ✅ Supported
- **Sub-second submission**: Mempool accepts instantly
- **Parallel execution**: Multiple trades in same block
- **Priority transactions**: Higher gas = faster inclusion

## 🔗 **Fuego L1 Integration**

### **Dual Mining Support**
- **Identical Algorithm**: CN-UPX/2 with same parameters
- **Merge Mining**: Every 60 seconds
- **Block Ratio**: 8:1 (8 zkC0DL3 blocks per Fuego block)
- **Cross-Chain ZK Proofs**: Unified proof system
- **AuxPoW Integration**: Full auxiliary proof-of-work

### **Compatibility Matrix**
| Feature | zkC0DL3 | Fuego L1 | Status |
|---------|---------|----------|--------|
| Mining Algorithm | CN-UPX/2 | CN-UPX/2 | ✅ Compatible |
| Block Time | 60s | 480s | ✅ Compatible |
| Merge Mining | ✅ | ✅ | ✅ Compatible |
| Dual Mining | ✅ | ✅ | ✅ Compatible |
| ZK Proofs | ✅ | ✅ | ✅ Compatible |

## 🚀 **Production Deployment**

### **Infrastructure Requirements**
- **CPU**: Multi-core processor (8+ cores recommended)
- **RAM**: 16GB+ (2MB scratchpad + system overhead)
- **Storage**: SSD with 100GB+ free space
- **Network**: Stable internet connection
- **OS**: Linux/macOS (Windows supported)

### **Deployment Commands**
```bash
# Build release version
cargo build --release

# Run in production
cargo run --release

# Run in background
nohup cargo run --release > c0dl3.log 2>&1 &

# Monitor logs
tail -f c0dl3.log
```

## 📈 **Monitoring & Maintenance**

### **Health Checks**
```bash
# Check RPC endpoint
curl http://localhost:8080/stats

# Check merge mining
curl http://localhost:8080/merge-mining/stats

# Check privacy status
curl http://localhost:8080/privacy/status
```

## 🔒 **Security Features**

### **Privacy Protection**
- **Transaction Encryption**: ChaCha20Poly1305 encryption
- **Address Encryption**: AEAD encryption for addresses
- **Timing Privacy**: Encrypted timestamps
- **STARK Proofs**: Zero-knowledge proof generation
- **Privacy Monitoring**: Real-time privacy analytics

### **Network Security**
- **Transport Security**: Noise protocol encryption
- **Peer Authentication**: Cryptographic peer verification
- **Message Integrity**: Cryptographic message verification
- **DDoS Protection**: Rate limiting and connection management

## 🤝 **Contributing**

### **Development Setup**
```bash
# Clone repository
git clone https://github.com/ColinRitman/C0DL3.git
cd C0DL3/c0dl3-zksync

# Install dependencies
cargo build

# Run tests
cargo test

# Run examples
cargo run --example simple_test
```

## 📄 **License**

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 **Acknowledgments**

- **zkSync Team**: For the original zkSync implementation
- **Fuego Team**: For L1 blockchain compatibility
- **winter-crypto**: For STARK proof system implementation
- **libp2p**: For P2P networking capabilities
- **Rust Community**: For excellent tooling and ecosystem

## 📞 **Support**

- **Issues**: [GitHub Issues](https://github.com/ColinRitman/C0DL3/issues)
- **Discussions**: [GitHub Discussions](https://github.com/ColinRitman/C0DL3/discussions)
- **Documentation**: [Production Deployment Guide](PRODUCTION_DEPLOYMENT.md)

---

**zkC0DL3** - Production-grade zkSync node with advanced privacy and Fuego L1 compatibility 🚀
