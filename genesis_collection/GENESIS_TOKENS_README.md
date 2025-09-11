# Genesis Token Contracts

This directory contains the genesis token contracts for the C0DL3 ecosystem.

## Token Overview

### 1. Para Token (PARA)
- **Supply**: Fixed at 1.0 token (non-mintable)
- **Decimals**: 18
- **Purpose**: Genesis token with fixed supply
- **Treasury**: DAO Treasury
- **Contract**: `ParaToken.sol`

### 2. C0LD Token (CD)
- **Supply**: Capped at 20 COLD tokens (12 decimals)
- **Purpose**: DAO-governed token for COLↁAO ecosystem
- **Treasury**: DAO Treasury (has DAO_ADMIN_ROLE)
- **Contract**: `ColdToken.sol`

#### CD Token Minting Roles
The C0LD token can be minted by contracts with the following roles:

1. **YIELD_MINTER_ROLE** - For yield farming rewards
2. **LP_MINTER_ROLE** - For liquidity provider rewards  
3. **STAKE_MINTER_ROLE** - For staking rewards

**What mints CD token?**
- Yield farming contracts (with YIELD_MINTER_ROLE)
- Liquidity provider reward contracts (with LP_MINTER_ROLE)
- Staking reward contracts (with STAKE_MINTER_ROLE)

The DAO Treasury (DAO_ADMIN_ROLE) can grant these roles to authorized contracts.

### 3. Elderado Genesis Collection (NFTs)
- **Supply**: 21 NFTs (fixed)
- **Purpose**: Genesis NFT collection for zkC0DL3 𝝣lderado validators
- **Treasury**: DIGM Treasury (receives all 21 NFTs)
- **Contract**: `ElderadoGenesisContract.sol`

## Deployment

### Quick Deployment
```bash
# Deploy all genesis contracts
npx hardhat run genesis_collection/deploy_genesis_tokens.js --network <network>
```

### Environment Variables
Set these environment variables for custom treasury addresses:
```bash
export DIGM_TREASURY_ADDRESS="0x..."
export DAO_TREASURY_ADDRESS="0x..."
```

### Individual Contract Deployment
```bash
# Deploy Para Token only
npx hardhat run genesis_collection/deploy_para_token.js --network <network>

# Deploy C0LD Token only  
npx hardhat run genesis_collection/deploy_cold_token.js --network <network>

# Deploy Elderado Collection only
npx hardhat run genesis_collection/deploy_genesis.js --network <network>
```

## Contract Details

### ParaToken.sol
- Inherits: `ERC20`, `Ownable`
- Fixed supply of 1.0 token minted to owner on deployment
- No additional minting allowed
- 18 decimals

### ColdToken.sol  
- Inherits: `ERC20Capped`, `AccessControlEnumerable`
- Capped supply of 20 COLD tokens (12 decimals)
- Role-based minting system
- DAO governance capabilities

### ElderadoGenesisContract.sol
- Inherits: `ERC721`, `ERC721Enumerable`, `ERC721URIStorage`, `Ownable`, `ReentrancyGuard`
- Fixed supply of 21 NFTs
- Genesis transaction mints all NFTs to DIGM Treasury
- Validator metadata and rarity system

## DAO Governance

The C0LD token implements a DAO governance system through OpenZeppelin's AccessControl:

- **DAO_ADMIN_ROLE**: Can grant/revoke other roles
- **YIELD_MINTER_ROLE**: Can mint tokens for yield rewards
- **LP_MINTER_ROLE**: Can mint tokens for LP rewards
- **STAKE_MINTER_ROLE**: Can mint tokens for staking rewards

## Treasury Addresses

- **DAO Treasury**: Receives Para Token and has DAO admin rights for C0LD Token
- **DIGM Treasury**: Receives all 21 Elderado NFTs

## Security Features

- Role-based access control
- Supply caps and fixed supplies
- Reentrancy protection
- Input validation
- Ownership controls

## Network Compatibility

These contracts are designed to work on:
- Ethereum mainnet
- Arbitrum
- zkSync Era
- Other EVM-compatible networks

## Verification

After deployment, verify contracts on block explorers:
```bash
npx hardhat verify --network <network> <contract_address> <constructor_args>
```
