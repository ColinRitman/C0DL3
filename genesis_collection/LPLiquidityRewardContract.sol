// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "./ColdToken.sol";

/// @title LP Liquidity Reward Contract – Rewards HEAT/ETH LP providers with C0LD tokens
/// @notice Distributes 7 COLD tokens over 8 years for L1 HEAT/ETH liquidity provision
contract LPLiquidityRewardContract is AccessControlEnumerable, ReentrancyGuard {
    using SafeMath for uint256;

    // --- Constants ---
    uint256 private constant TOTAL_REWARD_ALLOCATION = 7 * 10**12; // 7 COLD tokens (12 decimals)
    uint256 private constant DISTRIBUTION_PERIOD = 8 * 365 * 24 * 60 * 60; // 8 years in seconds
    uint256 private constant PHASE_DURATION = 2 * 365 * 24 * 60 * 60; // 2 years per phase
    
    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant LP_MINTER_ROLE = keccak256("LP_MINTER_ROLE");
    bytes32 public constant LP_UPDATER_ROLE = keccak256("LP_UPDATER_ROLE");
    
    // --- State Variables ---
    ColdToken public immutable coldToken;
    IERC20 public immutable heatToken;
    IERC20 public immutable ethToken;
    IERC20 public immutable lpToken;
    
    uint256 public immutable startTime;
    uint256 public totalDistributed;
    bool public distributionActive;
    
    // LP staking tracking
    struct LPStake {
        uint256 lpTokenAmount;
        uint256 timestamp;
        uint256 duration; // Staking duration in seconds
        uint256 rewardMultiplier; // Based on duration
        bool isActive;
        uint256 claimedRewards;
    }
    
    mapping(address => LPStake[]) public userStakes;
    mapping(address => uint256) public userTotalStaked;
    uint256 public totalActiveStakes;
    
    // Phase-based reward rates (per second per LP token staked)
    uint256[4] public phaseRates = [
        2.5e12 / (2 * 365 * 24 * 60 * 60), // Phase 1: 2.5 COLD over 2 years
        2.0e12 / (2 * 365 * 24 * 60 * 60), // Phase 2: 2.0 COLD over 2 years
        1.5e12 / (2 * 365 * 24 * 60 * 60), // Phase 3: 1.5 COLD over 2 years
        1.0e12 / (2 * 365 * 24 * 60 * 60)  // Phase 4: 1.0 COLD over 2 years
    ];
    
    // Duration multipliers for LP staking
    uint256 private constant SHORT_TERM_MULTIPLIER = 100; // 1x for < 30 days
    uint256 private constant MEDIUM_TERM_MULTIPLIER = 150; // 1.5x for 30-90 days
    uint256 private constant LONG_TERM_MULTIPLIER = 200; // 2x for > 90 days
    uint256 private constant ULTRA_LONG_TERM_MULTIPLIER = 300; // 3x for > 1 year
    
    // --- Events ---
    event LPStakeRecorded(
        address indexed user, 
        uint256 lpTokenAmount, 
        uint256 duration, 
        uint256 multiplier
    );
    event LPStakeUnstaked(address indexed user, uint256 stakeIndex, uint256 amount);
    event LPRewardsClaimed(address indexed user, uint256 amount);
    event DistributionPaused();
    event DistributionResumed();
    
    constructor(
        address _coldToken,
        address _heatToken,
        address _ethToken,
        address _lpToken,
        address daoTreasury
    ) {
        require(_coldToken != address(0), "Cold token zero address");
        require(_heatToken != address(0), "Heat token zero address");
        require(_ethToken != address(0), "Eth token zero address");
        require(_lpToken != address(0), "LP token zero address");
        require(daoTreasury != address(0), "DAO treasury zero address");
        
        coldToken = ColdToken(_coldToken);
        heatToken = IERC20(_heatToken);
        ethToken = IERC20(_ethToken);
        lpToken = IERC20(_lpToken);
        
        startTime = block.timestamp;
        distributionActive = true;
        
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(LP_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(LP_UPDATER_ROLE, DAO_ADMIN_ROLE);
    }
    
    /// @dev Stake LP tokens for rewards
    function stakeLPTokens(uint256 lpTokenAmount, uint256 duration) external nonReentrant {
        require(lpTokenAmount > 0, "Amount must be positive");
        require(duration > 0, "Duration must be positive");
        require(lpToken.balanceOf(msg.sender) >= lpTokenAmount, "Insufficient LP token balance");
        
        // Transfer LP tokens from user to contract
        lpToken.transferFrom(msg.sender, address(this), lpTokenAmount);
        
        // Calculate reward multiplier based on duration
        uint256 multiplier = calculateDurationMultiplier(duration);
        
        // Create stake record
        LPStake memory newStake = LPStake({
            lpTokenAmount: lpTokenAmount,
            timestamp: block.timestamp,
            duration: duration,
            rewardMultiplier: multiplier,
            isActive: true,
            claimedRewards: 0
        });
        
        userStakes[msg.sender].push(newStake);
        userTotalStaked[msg.sender] = userTotalStaked[msg.sender].add(lpTokenAmount);
        totalActiveStakes = totalActiveStakes.add(lpTokenAmount);
        
        emit LPStakeRecorded(msg.sender, lpTokenAmount, duration, multiplier);
    }
    
    /// @dev Unstake LP tokens (with penalty if before duration)
    function unstakeLPTokens(uint256 stakeIndex) external nonReentrant {
        require(stakeIndex < userStakes[msg.sender].length, "Invalid stake index");
        
        LPStake storage stake = userStakes[msg.sender][stakeIndex];
        require(stake.isActive, "Stake not active");
        
        uint256 currentTime = block.timestamp;
        uint256 timeStaked = currentTime.sub(stake.timestamp);
        
        // Calculate unstaking amount (penalty if before duration)
        uint256 unstakeAmount = stake.lpTokenAmount;
        if (timeStaked < stake.duration) {
            // Apply penalty: lose 50% of LP tokens if unstaking early
            unstakeAmount = unstakeAmount.div(2);
        }
        
        // Update stake status
        stake.isActive = false;
        userTotalStaked[msg.sender] = userTotalStaked[msg.sender].sub(stake.lpTokenAmount);
        totalActiveStakes = totalActiveStakes.sub(stake.lpTokenAmount);
        
        // Transfer LP tokens back to user
        lpToken.transfer(msg.sender, unstakeAmount);
        
        emit LPStakeUnstaked(msg.sender, stakeIndex, unstakeAmount);
    }
    
    /// @dev Calculate duration multiplier
    function calculateDurationMultiplier(uint256 duration) public pure returns (uint256) {
        if (duration < 30 days) {
            return SHORT_TERM_MULTIPLIER;
        } else if (duration < 90 days) {
            return MEDIUM_TERM_MULTIPLIER;
        } else if (duration < 365 days) {
            return LONG_TERM_MULTIPLIER;
        } else {
            return ULTRA_LONG_TERM_MULTIPLIER;
        }
    }
    
    /// @dev Calculate pending rewards for a user
    function calculatePendingRewards(address user) public view returns (uint256) {
        if (!distributionActive) {
            return 0;
        }
        
        uint256 totalRewards = 0;
        LPStake[] memory stakes = userStakes[user];
        
        for (uint256 i = 0; i < stakes.length; i++) {
            if (!stakes[i].isActive) continue;
            
            uint256 stakeRewards = calculateStakeRewards(stakes[i]);
            totalRewards = totalRewards.add(stakeRewards);
        }
        
        return totalRewards;
    }
    
    /// @dev Calculate rewards for a specific stake
    function calculateStakeRewards(LPStake memory stake) public view returns (uint256) {
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
        
        // Calculate rewards: lpTokenAmount * rate * time * multiplier / 100
        uint256 rewards = stake.lpTokenAmount
            .mul(currentRate)
            .mul(timeElapsed)
            .mul(stake.rewardMultiplier)
            .div(100)
            .div(1e18); // Normalize LP token amount
        
        return rewards;
    }
    
    /// @dev Claim pending LP rewards
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
        LPStake[] storage stakes = userStakes[msg.sender];
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
        coldToken.mintLpRewards(msg.sender, pendingRewards);
        
        emit LPRewardsClaimed(msg.sender, pendingRewards);
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
        
        LPStake[] memory stakes = userStakes[user];
        for (uint256 i = 0; i < stakes.length; i++) {
            if (stakes[i].isActive) {
                activeCount = activeCount.add(stakes[i].lpTokenAmount);
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
    
    /// @dev Grant LP_MINTER_ROLE to this contract
    function grantLPMinterRole() external onlyRole(DAO_ADMIN_ROLE) {
        coldToken.grantRole(coldToken.LP_MINTER_ROLE(), address(this));
    }
}
