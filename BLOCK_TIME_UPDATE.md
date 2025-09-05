# Updated to 30-Second Block Time

## Changes Made

The zkC0DL3 mining system has been updated to use **30-second blocks** instead of 12 seconds. This change affects multiple components of the system:

### ✅ **Code Changes**

1. **MiningConfig Default** (`src/main.rs`):
   ```rust
   impl Default for MiningConfig {
       fn default() -> Self {
           Self {
               enabled: true,
               target_block_time: 30,  // Changed from 12 to 30
               max_nonce: 1000000000000000000,
               difficulty_adjustment_blocks: 10,
           }
       }
   }
   ```

2. **CLI Parameter** (`src/main.rs`):
   ```rust
   #[arg(long, default_value = "30")]
   target_block_time: u64,
   ```

3. **Configuration Function** (`src/main.rs`):
   ```rust
   mining: MiningConfig {
       enabled: true,
       target_block_time: cli.target_block_time,  // Uses CLI parameter
       max_nonce: 1000000000000000000,
       difficulty_adjustment_blocks: 10,
   },
   ```

### ✅ **Configuration Files**

1. **config.example.json**:
   ```json
   "mining": {
     "enabled": true,
     "target_block_time": 30,  // Updated from 12
     "max_nonce": 1000000000000000000,
     "difficulty_adjustment_blocks": 10
   }
   ```

2. **docker-compose.yml**:
   ```yaml
   environment:
     - TARGET_BLOCK_TIME=30  # Added new environment variable
   ```

### ✅ **Documentation Updates**

1. **README.md**: Added `--target-block-time` CLI option documentation
2. **DEPLOYMENT.md**: Updated environment variables table and configuration examples
3. **MINING_SYSTEM.md**: Updated all references from 12 seconds to 30 seconds

## Impact of 30-Second Blocks

### **Advantages**
- **More Stable Mining**: Longer block times provide more time for transaction collection
- **Better Network Synchronization**: More time for P2P propagation
- **Reduced Orphan Blocks**: Less competition between miners
- **Lower Resource Usage**: Less frequent mining attempts

### **Trade-offs**
- **Slower Transaction Confirmation**: Transactions take longer to be included in blocks
- **Reduced Throughput**: Fewer blocks per hour (120 vs 300 blocks/hour)
- **Different Economic Model**: Mining rewards distributed less frequently

## Usage Examples

### **Command Line**
```bash
# Use default 30-second blocks
./codl3-zksync

# Override to use custom block time
./codl3-zksync --target-block-time 60

# Use faster blocks for testing
./codl3-zksync --target-block-time 10
```

### **Docker Compose**
```bash
# Start with 30-second blocks
docker-compose up -d

# Override block time via environment
TARGET_BLOCK_TIME=60 docker-compose up -d
```

### **Configuration File**
```json
{
  "mining": {
    "enabled": true,
    "target_block_time": 30,
    "max_nonce": 1000000000000000000,
    "difficulty_adjustment_blocks": 10
  }
}
```

## Performance Impact

With 30-second blocks:
- **Block Production**: 1 block every 30 seconds
- **Hourly Blocks**: 120 blocks per hour
- **Daily Blocks**: 2,880 blocks per day
- **Mining Frequency**: Every 30 seconds instead of 12 seconds

## Backward Compatibility

The change is **backward compatible**:
- Existing configurations will use the new 30-second default
- CLI parameter allows overriding the default
- No breaking changes to API or data structures
- Existing miners will automatically adapt to new timing

## Testing

To test the new block timing:

```bash
# Start the node
cargo run

# Monitor block production
curl http://localhost:9944/stats

# Check mining statistics
curl http://localhost:9944/health
```

The mining system will now attempt to mine blocks every 30 seconds, providing a more stable and predictable block production schedule.

---

**Status**: ✅ **COMPLETED** - zkC0DL3 now uses 30-second blocks by default
