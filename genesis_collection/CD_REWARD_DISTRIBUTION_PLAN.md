# C0LD Token Reward Distribution Plan (5-10 Years)

## Overview
This plan outlines the distribution of 20 COLD tokens over 5-10 years through three specific reward mechanisms, with PARA token streaming rewards.

## Token Allocation Strategy

### Total Supply: 20 COLD Tokens (12 decimals)
- **Max Circulating Supply**: 20 COLD tokens (hard cap)
- **Initial Supply**: 0 COLD (none minted at launch)
- **Distribution Period**: 5-10 years
- **Target Completion**: 2030-2035
- **Burning Mechanism**: 1 COLD token burned per community token listing
- **Circulating Supply Model**: When tokens are burned, more can be minted to maintain max circulating supply

## Reward Mechanisms

### 1. XFG CDyield Deposits (Fuego txextra tag 0x09)
**Allocation**: 8 COLD tokens (40% of total supply)
**Duration**: 7 years
**Mechanism**: 
- Triggered by Fuego transactions with txextra tag 0x09
- CDyield deposits earn CD tokens based on deposit amount and duration
- Gradual release schedule with diminishing returns

**Distribution Schedule**:
- Year 1-2: 3.0 COLD (high initial rewards)
- Year 3-4: 2.4 COLD (moderate rewards)
- Year 5-6: 1.8 COLD (reduced rewards)
- Year 7: 0.8 COLD (final phase)

### 2. L1 HEAT/ETH Liquidity Providers
**Allocation**: 7 COLD tokens (35% of total supply)
**Duration**: 8 years
**Mechanism**:
- Rewards for providing liquidity to HEAT/ETH pairs
- Based on LP token staking duration and amount
- Bonus rewards for long-term liquidity provision

**Distribution Schedule**:
- Year 1-2: 2.5 COLD (high LP incentives)
- Year 3-4: 2.0 COLD (moderate incentives)
- Year 5-6: 1.5 COLD (reduced incentives)
- Year 7-8: 1.0 COLD (final phase)

### 3. Elderado Staking Rewards
**Allocation**: 5 COLD tokens (25% of total supply)
**Duration**: 10 years
**Mechanism**:
- Rewards for staking Elderado NFTs
- Based on NFT rarity score and staking duration
- Genesis NFTs receive bonus multipliers

**Distribution Schedule**:
- Year 1-3: 2.0 COLD (high staking rewards)
- Year 4-6: 1.5 COLD (moderate rewards)
- Year 7-8: 1.0 COLD (reduced rewards)
- Year 9-10: 0.5 COLD (final phase)

## PARA Token Streaming Rewards

### PARA Distributor Contract
**Supply**: 1.0 PARA token (18 decimals)
**Purpose**: Continuous reward streaming
**Mechanism**:
- Streams PARA tokens to active participants
- Based on activity across all three reward mechanisms
- Diminishing stream rate over time

**Streaming Schedule**:
- Year 1-2: 0.4 PARA (high streaming rate)
- Year 3-5: 0.3 PARA (moderate streaming)
- Year 6-8: 0.2 PARA (reduced streaming)
- Year 9-10: 0.1 PARA (final phase)

## Burning Mechanism

### Community Token Listing Burns
**Purpose**: Deflationary mechanism for ecosystem growth
**Mechanism**:
- Requires burning 1 COLD token per community token listing
- DAO-approved community tokens for XFG yield options
- Creates scarcity and value appreciation over time

**Burn Requirements**:
- 1 COLD token burned per listing
- Only DAO members with COMMUNITY_LISTING_ROLE can execute
- Token must be approved by DAO governance
- Burn amount is fixed at 1 COLD token (non-negotiable)

**Expected Impact**:
- Deflationary pressure on circulating COLD token supply
- Increased scarcity over time
- Higher value per remaining COLD token
- Incentivizes careful token selection by DAO
- Burns free up minting space for new rewards
- Maintains max circulating supply of 20 COLD tokens

**Burn Statistics Tracking**:
- Total burned tokens counter
- Community listings count
- Circulating supply calculation (total supply - burned)
- Remaining mintable supply tracking (cap - circulating supply)
- Available minting space after burns

## Implementation Contracts

### 1. CDYieldRewardContract
- Handles XFG CDyield deposits with txextra tag 0x09
- Implements diminishing reward schedule
- Tracks deposit amounts and durations

### 2. LPLiquidityRewardContract
- Manages HEAT/ETH LP token staking
- Calculates rewards based on LP token value and duration
- Implements bonus multipliers for long-term staking

### 3. ElderadoStakingContract
- Handles Elderado NFT staking
- Calculates rewards based on NFT rarity and staking duration
- Implements genesis NFT bonus multipliers

### 4. ParaDistributorContract
- Streams PARA tokens to active participants
- Aggregates activity across all reward mechanisms
- Implements diminishing stream rates

## Technical Implementation

### Reward Calculation Formulas

#### CDYield Rewards
```
reward = baseReward * (depositAmount / totalDeposits) * timeMultiplier * phaseMultiplier
```

#### LP Rewards
```
reward = baseReward * (lpTokenValue / totalLPValue) * stakingDurationMultiplier * phaseMultiplier
```

#### Elderado Staking Rewards
```
reward = baseReward * rarityScore * stakingDurationMultiplier * genesisMultiplier * phaseMultiplier
```

#### PARA Streaming
```
streamRate = baseStreamRate * activityScore * phaseMultiplier
```

### Phase Multipliers
- **Phase 1** (Years 1-2): 1.0x
- **Phase 2** (Years 3-4): 0.8x
- **Phase 3** (Years 5-6): 0.6x
- **Phase 4** (Years 7-8): 0.4x
- **Phase 5** (Years 9-10): 0.2x

## Security Considerations

### Access Control
- DAO Treasury maintains admin rights
- Each reward contract has specific minter roles
- Emergency pause functionality for all contracts

### Supply Caps
- Hard cap of 20 COLD tokens enforced
- Individual reward mechanism caps
- Time-based release schedules

### Audit Requirements
- All contracts must be audited before deployment
- Regular security reviews during distribution period
- Bug bounty program for reward contracts

## Monitoring and Analytics

### Key Metrics
- Total COLD tokens distributed per mechanism
- Average reward per participant
- Participation rates across mechanisms
- PARA token streaming rates

### Reporting
- Monthly distribution reports
- Quarterly mechanism performance analysis
- Annual reward optimization reviews

## Governance

### DAO Control
- DAO can adjust reward rates within predefined bounds
- Emergency pause/unpause functionality
- Mechanism parameter updates through governance

### Community Input
- Regular community feedback on reward mechanisms
- Proposal system for reward optimization
- Transparency in all distribution decisions

## Risk Mitigation

### Technical Risks
- Contract upgradeability for bug fixes
- Emergency pause mechanisms
- Gradual rollout of new features

### Economic Risks
- Diminishing returns prevent inflation
- Time-based distribution prevents dumping
- Multiple mechanisms reduce single points of failure

## Success Metrics

### Participation Goals
- 1000+ active CDyield depositors by Year 2
- 500+ LP providers by Year 3
- 100+ Elderado NFT stakers by Year 1
- 10,000+ PARA token stream recipients by Year 5

### Distribution Targets
- 50% of COLD tokens distributed by Year 5
- 80% of COLD tokens distributed by Year 8
- 100% of COLD tokens distributed by Year 10
- 100% of PARA tokens streamed by Year 10

This plan ensures sustainable, long-term reward distribution while maintaining ecosystem growth and participant engagement over a 5-10 year period.
hen