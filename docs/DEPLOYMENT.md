# zkC0DL3 Deployment Guide

This guide covers different ways to deploy and run the zkC0DL3 zkSync hyperchain node.

## Prerequisites

- Rust 1.75+ (for building from source)
- Docker and Docker Compose (for containerized deployment)
- Git (for cloning the repository)

## Quick Start

### Option 1: Docker Compose (Recommended)

1. Clone the repository:
```bash
git clone <repository-url>
cd c0dl3-zksync
```

2. Configure environment variables in `docker-compose.yml`:
```yaml
environment:
  - L1_RPC_URL=https://mainnet.infura.io/v3/YOUR_KEY
  - L2_RPC_URL=https://zksync-mainnet.infura.io/v3/YOUR_KEY
  - VALIDATOR_ADDRESS=0x...
  - BRIDGE_ADDRESS=0x...
  - L1_CONTRACT_ADDRESS=0x...
```

3. Start the node:
```bash
docker-compose up -d
```

4. Check logs:
```bash
docker-compose logs -f zkc0dl3-node
```

5. Test the API:
```bash
curl http://localhost:9944/health
```

### Option 2: Build from Source

1. Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

2. Clone and build:
```bash
git clone <repository-url>
cd c0dl3-zksync
cargo build --release
```

3. Run the node:
```bash
./target/release/codl3-zksync \
  --l1-rpc-url "https://mainnet.infura.io/v3/YOUR_KEY" \
  --l2-rpc-url "https://zksync-mainnet.infura.io/v3/YOUR_KEY" \
  --hyperchain-id 324 \
  --validator-address "0x..." \
  --bridge-address "0x..." \
  --l1-contract-address "0x..."
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `L1_RPC_URL` | L1 Ethereum RPC endpoint | `http://localhost:8545` |
| `L2_RPC_URL` | L2 zkSync RPC endpoint | `http://localhost:3050` |
| `HYPERCHAIN_ID` | Hyperchain ID | `324` |
| `VALIDATOR_ADDRESS` | Validator address | Required |
| `BRIDGE_ADDRESS` | Bridge contract address | Required |
| `L1_CONTRACT_ADDRESS` | L1 contract address | Required |
| `P2P_PORT` | P2P listening port | `30333` |
| `RPC_PORT` | RPC server port | `9944` |
| `DATA_DIR` | Data directory | `./data` |
| `LOG_LEVEL` | Logging level | `info` |
| `TARGET_BLOCK_TIME` | Target block time in seconds | `30` |

### Configuration File

Create a `config.json` file:

```json
{
  "network": {
    "data_dir": "./data",
    "p2p_port": 30333,
    "listen_addr": "0.0.0.0",
    "bootstrap_peers": [
      "/ip4/127.0.0.1/tcp/30334/p2p/QmBootstrap1"
    ],
    "max_peers": 50
  },
  "rpc": {
    "port": 9944,
    "host": "127.0.0.1",
    "cors_origins": ["*"],
    "max_connections": 100
  },
  "mining": {
    "enabled": true,
    "target_block_time": 30,
    "max_nonce": 1000000000000000000,
    "difficulty_adjustment_blocks": 10
  },
  "zksync": {
    "l1_rpc_url": "https://mainnet.infura.io/v3/YOUR_KEY",
    "l2_rpc_url": "https://zksync-mainnet.infura.io/v3/YOUR_KEY",
    "hyperchain_id": 324,
    "validator_address": "0x...",
    "bridge_address": "0x...",
    "l1_contract_address": "0x...",
    "batch_size": 100,
    "batch_timeout": 300,
    "l1_batch_commitment": true,
    "priority_ops_enabled": true,
    "zk_proof_generation": true
  }
}
```

## Network Setup

### Mainnet Deployment

1. **Get RPC Endpoints**:
   - L1: Use Infura, Alchemy, or run your own Ethereum node
   - L2: Use zkSync mainnet RPC

2. **Configure Validator**:
   - Set up your validator address
   - Ensure sufficient ETH for gas fees
   - Configure bridge and contract addresses

3. **Firewall Configuration**:
   ```bash
   # Allow P2P traffic
   sudo ufw allow 30333/tcp
   
   # Allow RPC traffic (restrict to trusted IPs)
   sudo ufw allow from TRUSTED_IP to any port 9944
   ```

### Testnet Deployment

1. **Use Testnet Endpoints**:
   - L1: Goerli or Sepolia testnet
   - L2: zkSync testnet

2. **Test Configuration**:
   ```bash
   ./codl3-zksync \
     --l1-rpc-url "https://goerli.infura.io/v3/YOUR_KEY" \
     --l2-rpc-url "https://testnet.zksync.io" \
     --hyperchain-id 280 \
     --validator-address "0x..." \
     --bridge-address "0x..." \
     --l1-contract-address "0x..."
   ```

## Monitoring

### Health Checks

The node provides several health check endpoints:

```bash
# Basic health check
curl http://localhost:9944/health

# Detailed statistics
curl http://localhost:9944/stats

# Hyperchain information
curl http://localhost:9944/hyperchain/info
```

### Logging

Configure logging levels:

```bash
# Debug logging
RUST_LOG=debug ./codl3-zksync

# Specific module logging
RUST_LOG=codl3_zksync=debug,libp2p=info ./codl3-zksync
```

### Metrics

If using Prometheus monitoring:

```bash
# Start with metrics
docker-compose up -d prometheus grafana

# Access Grafana at http://localhost:3000
# Username: admin, Password: admin
```

## Troubleshooting

### Common Issues

1. **Port Already in Use**:
   ```bash
   # Check what's using the port
   sudo lsof -i :30333
   
   # Kill the process or use a different port
   ./codl3-zksync --p2p-port 30334
   ```

2. **RPC Connection Issues**:
   ```bash
   # Test RPC connectivity
   curl -X POST -H "Content-Type: application/json" \
     --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
     https://mainnet.infura.io/v3/YOUR_KEY
   ```

3. **Disk Space**:
   ```bash
   # Check disk usage
   df -h
   
   # Clean up old data if needed
   rm -rf ./data/old_blocks
   ```

4. **Memory Issues**:
   ```bash
   # Monitor memory usage
   htop
   
   # Adjust batch size if needed
   ./codl3-zksync --batch-size 50
   ```

### Performance Tuning

1. **Increase File Descriptors**:
   ```bash
   ulimit -n 65536
   ```

2. **Optimize Network Settings**:
   ```bash
   # Increase network buffer sizes
   echo 'net.core.rmem_max = 16777216' >> /etc/sysctl.conf
   echo 'net.core.wmem_max = 16777216' >> /etc/sysctl.conf
   sysctl -p
   ```

3. **Database Optimization**:
   ```bash
   # Use SSD storage for data directory
   ./codl3-zksync --data-dir /ssd/codl3-data
   ```

## Security Considerations

1. **Firewall Rules**:
   - Only expose necessary ports
   - Restrict RPC access to trusted IPs
   - Use VPN for remote access

2. **API Security**:
   - Implement authentication for RPC endpoints
   - Use HTTPS in production
   - Rate limit API requests

3. **Key Management**:
   - Store private keys securely
   - Use hardware wallets for validator keys
   - Rotate keys regularly

4. **Monitoring**:
   - Set up alerts for unusual activity
   - Monitor resource usage
   - Log all API requests

## Backup and Recovery

### Backup Strategy

1. **Regular Backups**:
   ```bash
   # Backup data directory
   tar -czf codl3-backup-$(date +%Y%m%d).tar.gz ./data
   
   # Backup configuration
   cp config.json config-backup-$(date +%Y%m%d).json
   ```

2. **Automated Backups**:
   ```bash
   # Add to crontab
   0 2 * * * /path/to/backup-script.sh
   ```

### Recovery Process

1. **Restore from Backup**:
   ```bash
   # Stop the node
   docker-compose down
   
   # Restore data
   tar -xzf codl3-backup-20240101.tar.gz
   
   # Restart the node
   docker-compose up -d
   ```

2. **Sync from Network**:
   ```bash
   # The node will automatically sync from peers
   # Monitor logs for sync progress
   docker-compose logs -f zkc0dl3-node
   ```

## Production Checklist

- [ ] Configure proper RPC endpoints
- [ ] Set up monitoring and alerting
- [ ] Implement backup strategy
- [ ] Configure firewall rules
- [ ] Set up log rotation
- [ ] Test disaster recovery procedures
- [ ] Document operational procedures
- [ ] Train operations team
- [ ] Set up health checks
- [ ] Configure auto-restart policies

## Support

For additional support:
- Check the logs: `docker-compose logs zkc0dl3-node`
- Review the README.md for detailed documentation
- Create an issue in the repository
- Join the C0DL3 community for help

---

**Note**: This is a production-ready implementation of zkC0DL3. Always test thoroughly in a testnet environment before deploying to mainnet.
