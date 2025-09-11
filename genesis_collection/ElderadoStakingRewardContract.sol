// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "./ColdToken.sol";

/// @title Elderado Staking Reward Contract – Rewards Elderado NFT staking with C0LD tokens
/// @notice Distributes 5 COLD tokens over 10 years for staking Elderado NFTs
contract ElderadoStakingRewardContract is AccessControlEnumerable, ReentrancyGuard {
    using SafeMath for uint256;

    // --- Constants ---
    uint256 private constant TOTAL_REWARD_ALLOCATION = 5 * 10**12; // 5 COLD tokens (12 decimals)
    uint256 private constant DISTRIBUTION_PERIOD = 10 * 365 * 24 * 60 * 60; // 10 years in seconds
    uint256 private constant PHASE_DURATION = 2 * 365 * 24 * 60 * 60; // 2 years per phase
    
    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant STAKE_MINTER_ROLE = keccak256("STAKE_MINTER_ROLE");
    bytes32 public constant NFT_UPDATER_ROLE = keccak256("NFT_UPDATER_ROLE");
    
    // --- State Variables ---
    ColdToken public immutable coldToken;
    IERC721 public immutable elderadoNFT;
    
    uint256 public immutable startTime;
    uint256 public totalDistributed;
    bool public distributionActive;
    
    // NFT staking tracking
    struct NFTStake {
        uint256 tokenId;
        uint256 timestamp;
        uint256 rarityScore;
        bool isGenesis;
        bool isActive;
        uint256 claimedRewards;
    }
    
    mapping(address => NFTStake[]) public userStakes;
    mapping(address => uint256) public userTotalStaked;
    uint256 public totalActiveStakes;
    
    // Phase-based reward rates (per second per rarity score)
    uint256[5] public phaseRates = [
        2.0e12 / (3 * 365 * 24 * 60 * 60), // Phase 1: 2.0 COLD over 3 years
        1.5e12 / (3 * 365 * 24 * 60 * 60), // Phase 2: 1.5 COLD over 3 years
        1.0e12 / (2 * 365 * 24 * 60 * 60), // Phase 3: 1.0 COLD over 2 years
        0.5e12 / (2 * 365 * 24 * 60 * 60), // Phase 4: 0.5 COLD over 2 years
        0.0e12 / (1 * 365 * 24 * 60 * 60)  // Phase 5: 0.0 COLD (distribution complete)
    ];
    
    // Genesis NFT multiplier
    uint256 private constant GENESIS_MULTIPLIER = 200; // 2x for genesis NFTs
    
    // --- Events ---
    event NFTStaked(
        address indexed user, 
        uint256 tokenId, 
        uint256 rarityScore, 
        bool isGenesis
    );
    event NFTUnstaked(address indexed user, uint256 stakeIndex, uint256 tokenId);
    event StakingRewardsClaimed(address indexed user, uint256 amount);
    event DistributionPaused();
    event DistributionResumed();
    
    constructor(address _coldToken, address _elderadoNFT, address daoTreasury) {
        require(_coldToken != address(0), "Cold token zero address");
        require(_elderadoNFT != address(0), "Elderado NFT zero address");
        require(daoTreasury != address(0), "DAO treasury zero address");
        
        coldToken = ColdToken(_coldToken);
        elderadoNFT = IERC721(_elderadoNFT);
        
        startTime = block.timestamp;
        distributionActive = true;
        
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(STAKE_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(NFT_UPDATER_ROLE, DAO_ADMIN_ROLE);
    }
    
    /// @dev Stake an Elderado NFT for rewards
    function stakeNFT(uint256 tokenId, uint256 rarityScore, bool isGenesis) external nonReentrant {
        require(elderadoNFT.ownerOf(tokenId) == msg.sender, "Not NFT owner");
        require(rarityScore > 0, "Rarity score must be positive");
        
        // Transfer NFT from user to contract
        elderadoNFT.transferFrom(msg.sender, address(this), tokenId);
        
        // Create stake record
        NFTStake memory newStake = NFTStake({
            tokenId: tokenId,
            timestamp: block.timestamp,
            rarityScore: rarityScore,
            isGenesis: isGenesis,
            isActive: true,
            claimedRewards: 0
        });
        
        userStakes[msg.sender].push(newStake);
        userTotalStaked[msg.sender] = userTotalStaked[msg.sender].add(1);
        totalActiveStakes = totalActiveStakes.add(1);
        
        emit NFTStaked(msg.sender, tokenId, rarityScore, isGenesis);
    }
    
    /// @dev Unstake an Elderado NFT
    function unstakeNFT(uint256 stakeIndex) external nonReentrant {
        require(stakeIndex < userStakes[msg.sender].length, "Invalid stake index");
        
        NFTStake storage stake = userStakes[msg.sender][stakeIndex];
        require(stake.isActive, "Stake not active");
        
        // Update stake status
        stake.isActive = false;
        userTotalStaked[msg.sender] = userTotalStaked[msg.sender].sub(1);
        totalActiveStakes = totalActiveStakes.sub(1);
        
        // Transfer NFT back to user
        elderadoNFT.transferFrom(address(this), msg.sender, stake.tokenId);
        
        emit NFTUnstaked(msg.sender, stakeIndex, stake.tokenId);
    }
    
    /// @dev Calculate pending rewards for a user
    function calculatePendingRewards(address user) public view returns (uint256) {
        if (!distributionActive) {
            return 0;
        }
        
        uint256 totalRewards = 0;
        NFTStake[] memory stakes = userStakes[user];
        
        for (uint256 i = 0; i < stakes.length; i++) {
            if (!stakes[i].isActive) continue;
            
            uint256 stakeRewards = calculateStakeRewards(stakes[i]);
            totalRewards = totalRewards.add(stakeRewards);
        }
        
        return totalRewards;
    }
    
    /// @dev Calculate rewards for a specific stake
    function calculateStakeRewards(NFTStake memory stake) public view returns (uint256) {
        uint256 currentTime = block.timestamp;
        uint256 timeElapsed = currentTime.sub(stake.timestamp);
        
        if (timeElapsed == 0) {
            return 0;
        }
        
        // Calculate current phase and rate
        uint256 totalTimeElapsed = currentTime.sub(startTime);
        uint256 currentPhase = totalTimeElapsed.div(PHASE_DURATION);
        
        if (currentPhase >= phaseRates.length) {
            return 0; // Distribution period ended
        }
        
        uint256 currentRate = phaseRates[currentPhase];
        
        // Calculate base rewards: rarityScore * rate * time
        uint256 baseRewards = stake.rarityScore
            .mul(currentRate)
            .mul(timeElapsed)
            .div(1e18); // Normalize rarity score
        
        // Apply genesis multiplier if applicable
        if (stake.isGenesis) {
            baseRewards = baseRewards.mul(GENESIS_MULTIPLIER).div(100);
        }
        
        return baseRewards;
    }
    
    /// @dev Claim pending staking rewards
    function claimRewards() external nonReentrant {
        require(distributionActive, "Distribution paused");
        
        uint256 pendingRewards = calculatePendingRewards(msg.sender);
        require(pendingRewards > 0, "No rewards to claim");
        
        // Check if we can mint the required amount
        require(
            coldToken.totalSupply().add(pendingRewards) <= coldToken.cap(),
            "Would exceed COLD token cap"
        );
        
        // Update claimed rewards for all active stakes
        NFTStake[] storage stakes = userStakes[msg.sender];
        for (uint256 i = 0; i < stakes.length; i++) {
            if (stakes[i].isActive) {
                stakes[i].claimedRewards = stakes[i].claimedRewards.add(
                    calculateStakeRewards(stakes[i])
                );
            }
        }
        
        // Update total distributed
        totalDistributed = totalDistributed.add(pendingRewards);
        
        // Mint COLD tokens to user
        coldToken.mintStakingRewards(msg.sender, pendingRewards);
        
        emit StakingRewardsClaimed(msg.sender, pendingRewards);
    }
    
    /// @dev Get current distribution phase
    function getCurrentPhase() public view returns (uint256) {
        uint256 totalTimeElapsed = block.timestamp.sub(startTime);
        return totalTimeElapsed.div(PHASE_DURATION);
    }
    
    /// @dev Get current phase rate
    function getCurrentPhaseRate() public view returns (uint256) {
        uint256 currentPhase = getCurrentPhase();
        if (currentPhase >= phaseRates.length) {
            return 0;
        }
        return phaseRates[currentPhase];
    }
    
    /// @dev Get total remaining COLD to distribute
    function getRemainingCold() public view returns (uint256) {
        return TOTAL_REWARD_ALLOCATION.sub(totalDistributed);
    }
    
    /// @dev Get user stake statistics
    function getUserStakeStats(address user) 
        external 
        view 
        returns (
            uint256 totalStaked,
            uint256 activeStakes,
            uint256 pendingRewards,
            uint256 totalClaimed
        ) 
    {
        uint256 activeCount = 0;
        uint256 totalClaimedAmount = 0;
        
        NFTStake[] memory stakes = userStakes[user];
        for (uint256 i = 0; i < stakes.length; i++) {
            if (stakes[i].isActive) {
                activeCount = activeCount.add(1);
            }
            totalClaimedAmount = totalClaimedAmount.add(stakes[i].claimedRewards);
        }
        
        return (
            userTotalStaked[user],
            activeCount,
            calculatePendingRewards(user),
            totalClaimedAmount
        );
    }
    
    /// @dev Get contract statistics
    function getContractStats() 
        external 
        view 
        returns (
            uint256 totalDistributed,
            uint256 remainingCold,
            uint256 currentPhase,
            uint256 currentPhaseRate,
            uint256 totalActiveStakes,
            bool distributionActive
        ) 
    {
        return (
            totalDistributed,
            getRemainingCold(),
            getCurrentPhase(),
            getCurrentPhaseRate(),
            totalActiveStakes,
            distributionActive
        );
    }
    
    /// @dev Pause distribution (emergency function)
    function pauseDistribution() external onlyRole(DAO_ADMIN_ROLE) {
        distributionActive = false;
        emit DistributionPaused();
    }
    
    /// @dev Resume distribution
    function resumeDistribution() external onlyRole(DAO_ADMIN_ROLE) {
        distributionActive = true;
        emit DistributionResumed();
    }
    
    /// @dev Grant STAKE_MINTER_ROLE to this contract
    function grantStakeMinterRole() external onlyRole(DAO_ADMIN_ROLE) {
        coldToken.grantRole(coldToken.STAKE_MINTER_ROLE(), address(this));
    }
    
    /// @dev Emergency function to recover stuck NFTs
    function emergencyRecoverNFT(uint256 tokenId, address to) external onlyRole(DAO_ADMIN_ROLE) {
        require(elderadoNFT.ownerOf(tokenId) == address(this), "NFT not owned by contract");
        elderadoNFT.transferFrom(address(this), to, tokenId);
    }
}
