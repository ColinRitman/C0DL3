# C0LD Token Circulating Supply Model Example

## Overview
The C0LD token implements a **maximum circulating supply model** where the hard cap is 20 COLD tokens, but when tokens are burned, more tokens can be minted to maintain the maximum circulating supply.

## How It Works

### Initial State
- **Max Circulating Supply**: 20 COLD tokens
- **Total Supply**: 0 COLD tokens
- **Burned**: 0 COLD tokens
- **Circulating Supply**: 0 COLD tokens
- **Available Minting Space**: 20 COLD tokens

### Example Scenario

#### Step 1: Mint 10 COLD tokens for rewards
- **Total Supply**: 10 COLD tokens
- **Burned**: 0 COLD tokens
- **Circulating Supply**: 10 COLD tokens
- **Available Minting Space**: 10 COLD tokens (20 - 10)

#### Step 2: Burn 2 COLD tokens for community listings
- **Total Supply**: 10 COLD tokens
- **Burned**: 2 COLD tokens
- **Circulating Supply**: 8 COLD tokens (10 - 2)
- **Available Minting Space**: 12 COLD tokens (20 - 8)

#### Step 3: Mint 5 more COLD tokens for rewards
- **Total Supply**: 15 COLD tokens
- **Burned**: 2 COLD tokens
- **Circulating Supply**: 13 COLD tokens (15 - 2)
- **Available Minting Space**: 7 COLD tokens (20 - 13)

#### Step 4: Burn 1 more COLD token for another listing
- **Total Supply**: 15 COLD tokens
- **Burned**: 3 COLD tokens
- **Circulating Supply**: 12 COLD tokens (15 - 3)
- **Available Minting Space**: 8 COLD tokens (20 - 12)

## Key Benefits

### 1. Deflationary Pressure
- Burns reduce the circulating supply
- Creates scarcity and value appreciation
- Higher value per remaining COLD token

### 2. Continuous Reward Distribution
- Burns free up minting space
- Allows continued reward distribution
- Maintains ecosystem incentives

### 3. Controlled Supply
- Hard cap of 20 COLD tokens enforced
- Never exceeds maximum circulating supply
- Predictable supply mechanics

## Contract Functions

### Minting Functions
```solidity
function mintYield(address to, uint256 amount) external onlyRole(YIELD_MINTER_ROLE)
function mintLpRewards(address to, uint256 amount) external onlyRole(LP_MINTER_ROLE)
function mintStakingRewards(address to, uint256 amount) external onlyRole(STAKE_MINTER_ROLE)
```

### Burning Functions
```solidity
function burnForCommunityListing(address tokenAddress, string calldata tokenSymbol) external onlyRole(COMMUNITY_LISTING_ROLE)
function burn(uint256 amount) public override
function burnFrom(address account, uint256 amount) public override
```

### Utility Functions
```solidity
function getCirculatingSupply() external view returns (uint256)
function getRemainingMintableSupply() external view returns (uint256)
function getTotalBurned() external view returns (uint256)
function getCommunityListingsCount() external view returns (uint256)
```

## Economic Model

### Supply Dynamics
- **Maximum Circulating Supply**: 20 COLD tokens (hard cap)
- **Burning Rate**: 1 COLD token per community token listing
- **Minting Rate**: Gradual distribution over 5-10 years
- **Net Effect**: Deflationary pressure with continued rewards

### Value Appreciation
- Burns reduce circulating supply
- Scarcity increases over time
- Higher value per remaining token
- Incentivizes careful token selection

### Ecosystem Growth
- Community token listings require burns
- Creates barrier to entry for new tokens
- Ensures quality control by DAO
- Maintains ecosystem integrity

## Implementation Details

### Minting Logic
```solidity
function _mintChecked(address to, uint256 amount) internal {
    // Calculate available minting space: cap - (totalSupply - totalBurned)
    uint256 availableMintingSpace = cap().sub(totalSupply().sub(totalBurned));
    require(amount <= availableMintingSpace, "cap exceeded");
    _mint(to, amount);
}
```

### Circulating Supply Calculation
```solidity
function getCirculatingSupply() external view returns (uint256) {
    return totalSupply().sub(totalBurned);
}
```

### Remaining Mintable Supply
```solidity
function getRemainingMintableSupply() external view returns (uint256) {
    return cap().sub(getCirculatingSupply());
}
```

This model ensures that the C0LD token maintains its maximum circulating supply of 20 tokens while creating deflationary pressure through burns, ultimately leading to increased value and ecosystem growth.
