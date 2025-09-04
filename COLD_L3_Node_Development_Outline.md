# COLD L3 Node Development Outline
## Background AI Agent Work Plan

### Project Overview
**Goal**: Build a complete COLD L3 node that bridges Fuego (CryptoNote) blockchain with Arbitrum L3, implementing BFT consensus with PoW merge-mining.

**Current State**: 
- ✅ P2P networking module (`net-p2p`) is working with libp2p 0.53
- ✅ Basic CLI structure exists
- 🔄 Need to scaffold remaining 8 modules and integrate them

---

## 1. PROJECT STRUCTURE & SETUP

### 1.1 Crate Architecture
```
coldl3-node/
├── Cargo.toml (workspace)
├── crates/
│   ├── net-p2p/          ✅ COMPLETE
│   ├── block-sync/       🔄 TO BUILD
│   ├── consensus/        🔄 TO BUILD  
│   ├── txpool/           🔄 TO BUILD
│   ├── state-db/         🔄 TO BUILD
│   ├── commitments/      🔄 TO BUILD
│   ├── bridge/           🔄 TO BUILD
│   ├── rpc/              🔄 TO BUILD
│   ├── encryption/       🔄 TO BUILD
│   ├── cli/              ✅ EXISTS
│   └── node/             🔄 TO BUILD (integration layer)
```

### 1.2 Workspace Configuration
- Update root `Cargo.toml` to include all new crates
- Set up shared dependencies and version constraints
- Configure FFI bindings for C++ integration points

---

## 2. MODULE IMPLEMENTATION PRIORITY

### Phase 1: Core Infrastructure (Weeks 1-2)
**Priority**: High - Foundation for everything else

#### 2.1 State Database (`state-db`)
- **Dependencies**: `rocksdb`, `merkle-light`
- **Key Features**:
  - KV store with versioned Merkle tries
  - Block state management
  - Commitment storage
- **API Design**:
  ```rust
  pub trait StateDB {
      async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
      async fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;
      async fn commit(&self, version: u64) -> Result<MerkleRoot>;
  }
  ```

#### 2.2 Commitments (`commitments`)
- **Dependencies**: `poseidon`, `blake2`, `serde`
- **Key Features**:
  - HEAT & Yield commitment calculations
  - Unit tests replicating `test_heat_commitment.cpp`
  - Commitment verification
- **API Design**:
  ```rust
  pub struct CommitmentEngine {
      pub fn calculate_heat_commitment(&self, data: &[u8]) -> Result<[u8; 32]>;
      pub fn calculate_yield_commitment(&self, data: &[u8]) -> Result<[u8; 32]>;
      pub fn verify_commitment(&self, commitment: &[u8; 32], data: &[u8]) -> bool;
  }
  ```

### Phase 2: Blockchain Logic (Weeks 3-4)
**Priority**: High - Core blockchain functionality

#### 2.3 Block Sync (`block-sync`)
- **Dependencies**: FFI to C++ block parser
- **Key Features**:
  - Rust block structs and validation
  - FFI calls to Fuego's C++ block parser
  - Fast-sync implementation
- **API Design**:
  ```rust
  pub struct BlockSync {
      pub async fn sync_blocks(&mut self, from_height: u64) -> Result<Vec<Block>>;
      pub async fn validate_block(&self, block: &Block) -> Result<bool>;
      pub async fn get_block_by_hash(&self, hash: &[u8; 32]) -> Result<Option<Block>>;
  }
  ```

#### 2.4 Transaction Pool (`txpool`)
- **Dependencies**: `dashmap`, `priority-queue`
- **Key Features**:
  - Stateless transaction management
  - Pluggable fee algorithms
  - Priority-based ordering
- **API Design**:
  ```rust
  pub struct TxPool {
      pub async fn add_transaction(&mut self, tx: Transaction) -> Result<()>;
      pub async fn get_transactions(&self, limit: usize) -> Vec<Transaction>;
      pub async fn remove_transaction(&mut self, tx_hash: &[u8; 32]) -> Result<()>;
  }
  ```

### Phase 3: Consensus & Bridge (Weeks 5-6)
**Priority**: High - Consensus and cross-chain functionality

#### 2.5 Consensus (`consensus`)
- **Dependencies**: `hotshot`, FFI to Fuego hash
- **Key Features**:
  - HotStuff BFT consensus
  - PoW merge-mining adapter
  - FFI calls to Fuego hash function
- **API Design**:
  ```rust
  pub struct Consensus {
      pub async fn start_consensus(&mut self) -> Result<()>;
      pub async fn propose_block(&mut self, block: Block) -> Result<()>;
      pub async fn get_finalized_blocks(&self) -> Vec<Block>;
  }
  ```

#### 2.6 Bridge (`bridge`)
- **Dependencies**: `ethers-rs`, `serde_json`
- **Key Features**:
  - Fuego header verification
  - Arbitrum proof submission
  - IBC-like relayer functionality
- **API Design**:
  ```rust
  pub struct Bridge {
      pub async fn verify_fuego_header(&self, header: &Header) -> Result<bool>;
      pub async fn submit_to_arbitrum(&self, proof: &Proof) -> Result<()>;
      pub async fn get_bridge_state(&self) -> BridgeState;
  }
  ```

### Phase 4: API & Integration (Weeks 7-8)
**Priority**: Medium - User interfaces and integration

#### 2.7 RPC/REST (`rpc`)
- **Dependencies**: `axum`, `jsonrpc-core`, `tower`
- **Key Features**:
  - JSON-RPC endpoints
  - REST API for wallets/dApps
  - WebSocket support
- **API Design**:
  ```rust
  pub struct RPCServer {
      pub async fn start(&self, addr: SocketAddr) -> Result<()>;
      pub async fn register_methods(&mut self) -> Result<()>;
  }
  ```

#### 2.8 Encryption (`encryption`)
- **Dependencies**: FFI to C++ AEGIS-256X
- **Key Features**:
  - Wallet-side encryption
  - FFI to existing C++ AEGIS-256X
- **API Design**:
  ```rust
  pub struct Encryption {
      pub fn encrypt(&self, data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;
      pub fn decrypt(&self, data: &[u8], key: &[u8; 32]) -> Result<Vec<u8>>;
  }
  ```

### Phase 5: Integration Layer (Week 9)
**Priority**: High - Wire everything together

#### 2.9 Node Integration (`node`)
- **Dependencies**: All other crates
- **Key Features**:
  - Orchestrate all subsystems
  - Manage async tasks and channels
  - Handle startup/shutdown
- **API Design**:
  ```rust
  pub struct ColdL3Node {
      pub async fn start(&mut self) -> Result<()>;
      pub async fn stop(&mut self) -> Result<()>;
      pub async fn get_status(&self) -> NodeStatus;
  }
  ```

---

## 3. DEVELOPMENT GUIDELINES

### 3.1 Code Quality Standards
- **Rust Best Practices**: Use `clippy`, `rustfmt`, comprehensive error handling
- **Testing**: Unit tests for each module, integration tests for critical paths
- **Documentation**: Rust doc comments, README files for each crate
- **Error Handling**: Use `anyhow` or `thiserror` for consistent error types

### 3.2 FFI Integration Strategy
- **C++ Bindings**: Use `cxx` crate for safe FFI
- **Memory Management**: Ensure proper cleanup and no memory leaks
- **Error Propagation**: Convert C++ errors to Rust `Result` types
- **Testing**: Test FFI boundaries thoroughly

### 3.3 Async Architecture
- **Tokio Runtime**: Use tokio for async/await throughout
- **Channel Communication**: Use `tokio::sync::mpsc` for inter-module communication
- **Task Management**: Proper task spawning and lifecycle management
- **Graceful Shutdown**: Implement proper shutdown sequences

### 3.4 Configuration Management
- **Environment Variables**: Use `config` crate for configuration
- **CLI Arguments**: Extend existing CLI with new options
- **Default Values**: Sensible defaults for all configurable parameters

---

## 4. TESTING STRATEGY

### 4.1 Unit Tests
- Each crate should have comprehensive unit tests
- Mock external dependencies where appropriate
- Test error conditions and edge cases

### 4.2 Integration Tests
- Test inter-module communication
- Test FFI boundaries
- Test end-to-end workflows

### 4.3 Performance Tests
- Benchmark critical paths
- Test with realistic data volumes
- Monitor memory usage and performance

---

## 5. DEPLOYMENT & OPERATIONS

### 5.1 Build System
- **Docker**: Create Dockerfile for containerized deployment
- **CI/CD**: Set up GitHub Actions for automated testing and building
- **Cross-compilation**: Support for multiple target platforms

### 5.2 Monitoring & Logging
- **Logging**: Use `tracing` for structured logging
- **Metrics**: Use `prometheus` for metrics collection
- **Health Checks**: Implement health check endpoints

### 5.3 Configuration Management
- **Config Files**: Support for TOML/YAML configuration files
- **Environment Variables**: Override config with environment variables
- **Validation**: Validate configuration at startup

---

## 6. MILESTONES & DELIVERABLES

### Milestone 1 (Week 2): Core Infrastructure
- ✅ State database with Merkle tries
- ✅ Commitment calculations
- ✅ Basic integration tests

### Milestone 2 (Week 4): Blockchain Logic
- ✅ Block sync with FFI integration
- ✅ Transaction pool with fee algorithms
- ✅ Block validation pipeline

### Milestone 3 (Week 6): Consensus & Bridge
- ✅ HotStuff BFT consensus
- ✅ PoW merge-mining
- ✅ Bridge to Arbitrum
- ✅ Cross-chain proof verification

### Milestone 4 (Week 8): API Layer
- ✅ JSON-RPC server
- ✅ REST API endpoints
- ✅ WebSocket support
- ✅ Encryption module

### Milestone 5 (Week 9): Integration & Polish
- ✅ Complete node integration
- ✅ End-to-end testing
- ✅ Documentation and deployment
- ✅ Performance optimization

---

## 7. RISK MITIGATION

### 7.1 Technical Risks
- **FFI Complexity**: Start with simple FFI calls, gradually increase complexity
- **Async Deadlocks**: Use proper channel design and timeout mechanisms
- **Memory Leaks**: Regular profiling and memory usage monitoring

### 7.2 Integration Risks
- **Module Dependencies**: Clear API contracts and versioning
- **External Dependencies**: Pin dependency versions, have fallback options
- **Performance**: Regular benchmarking and optimization

### 7.3 Operational Risks
- **Configuration Errors**: Comprehensive validation and error messages
- **Network Issues**: Robust error handling and retry mechanisms
- **Data Corruption**: Proper backup and recovery procedures

---

## 8. SUCCESS CRITERIA

### 8.1 Functional Requirements
- ✅ Node starts and connects to P2P network
- ✅ Syncs blocks from Fuego blockchain
- ✅ Runs BFT consensus with PoW merge-mining
- ✅ Bridges data to Arbitrum L3
- ✅ Provides RPC/REST API for clients
- ✅ Handles transactions and maintains state

### 8.2 Performance Requirements
- ✅ Syncs blocks at reasonable speed
- ✅ Handles transaction throughput
- ✅ Low memory usage
- ✅ Fast API response times

### 8.3 Quality Requirements
- ✅ Comprehensive test coverage
- ✅ Well-documented APIs
- ✅ Error handling and logging
- ✅ Security best practices

---

## 9. NEXT STEPS

### Immediate Actions (Next 24-48 hours)
1. **Scaffold all crates** with basic structure and Cargo.toml files
2. **Define module APIs** with trait definitions
3. **Set up workspace configuration** in root Cargo.toml
4. **Create stub implementations** for rapid integration testing

### Week 1 Goals
1. **Complete state-db module** with RocksDB integration
2. **Complete commitments module** with Poseidon/Blake2
3. **Set up FFI infrastructure** for C++ integration
4. **Begin block-sync module** with basic Rust structs

### Success Metrics
- All crates compile successfully
- Basic integration tests pass
- FFI calls work correctly
- Node can start and run basic operations

---

**Note**: This outline follows the "use what works first" principle, prioritizing stable, well-tested libraries and gradual complexity increase. Each phase builds upon the previous one, ensuring a solid foundation before adding advanced features.

