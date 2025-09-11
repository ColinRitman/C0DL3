// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";
import "./ColdToken.sol";

/// @title CD Yield Reward Contract – Rewards XFG CDyield deposits with C0LD tokens
/// @notice Distributes 8 COLD tokens over 7 years for Fuego transactions with txextra tag 0x09
contract CDYieldRewardContract is AccessControlEnumerable, ReentrancyGuard {
    using SafeMath for uint256;

    // --- Constants ---
    uint256 private constant TOTAL_REWARD_ALLOCATION = 8 * 10**12; // 8 COLD tokens (12 decimals)
    uint256 private constant DISTRIBUTION_PERIOD = 7 * 365 * 24 * 60 * 60; // 7 years in seconds
    uint256 private constant PHASE_DURATION = 2 * 365 * 24 * 60 * 60; // 2 years per phase
    
    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant YIELD_MINTER_ROLE = keccak256("YIELD_MINTER_ROLE");
    bytes32 public constant DEPOSIT_UPDATER_ROLE = keccak256("DEPOSIT_UPDATER_ROLE");
    
    // --- State Variables ---
    ColdToken public immutable coldToken;
    uint256 public immutable startTime;
    uint256 public totalDistributed;
    bool public distributionActive;
    
    // CDyield deposit tracking
    struct CDyieldDeposit {
        uint256 amount; // XFG deposit amount
        uint256 timestamp;
        uint256 duration; // Deposit duration in seconds
        uint256 rewardMultiplier; // Based on duration
        bool isActive;
        uint256 claimedRewards;
    }
    
    mapping(address => CDyieldDeposit[]) public userDeposits;
    mapping(address => uint256) public userTotalDeposits;
    uint256 public totalActiveDeposits;
    
    // Phase-based reward rates (per second per XFG deposited)
    uint256[4] public phaseRates = [
        3e12 / (2 * 365 * 24 * 60 * 60), // Phase 1: 3.0 COLD over 2 years
        2.5e12 / (2 * 365 * 24 * 60 * 60), // Phase 2: 2.5 COLD over 2 years
        1.8e12 / (2 * 365 * 24 * 60 * 60), // Phase 3: 1.8 COLD over 2 years
        0.7e12 / (1 * 365 * 24 * 60 * 60)  // Phase 4: 0.7 COLD over 1 year
    ];
    
    // Duration multipliers for deposits
    uint256 private constant SHORT_TERM_MULTIPLIER = 100; // 1x for < 30 days
    uint256 private constant MEDIUM_TERM_MULTIPLIER = 150; // 1.5x for 30-90 days
    uint256 private constant LONG_TERM_MULTIPLIER = 200; // 2x for > 90 days
    
    // --- Events ---
    event CDyieldDepositRecorded(
        address indexed user, 
        uint256 amount, 
        uint256 duration, 
        uint256 multiplier
    );
    event CDyieldRewardsClaimed(address indexed user, uint256 amount);
    event DistributionPaused();
    event DistributionResumed();
    
    constructor(address _coldToken, address daoTreasury) {
        require(_coldToken != address(0), "Cold token zero address");
        require(daoTreasury != address(0), "DAO treasury zero address");
        
        coldToken = ColdToken(_coldToken);
        startTime = block.timestamp;
        distributionActive = true;
        
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(YIELD_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(DEPOSIT_UPDATER_ROLE, DAO_ADMIN_ROLE);
    }
    
    /// @dev Record a CDyield deposit (called by Fuego integration)
    function recordCDyieldDeposit(
        address user,
        uint256 amount,
        uint256 duration
    ) external onlyRole(DEPOSIT_UPDATER_ROLE) {
        require(user != address(0), "User zero address");
        require(amount > 0, "Amount must be positive");
        require(duration > 0, "Duration must be positive");
        
        // Calculate reward multiplier based on duration
        uint256 multiplier = calculateDurationMultiplier(duration);
        
        // Create deposit record
        CDyieldDeposit memory newDeposit = CDyieldDeposit({
            amount: amount,
            timestamp: block.timestamp,
            duration: duration,
            rewardMultiplier: multiplier,
            isActive: true,
            claimedRewards: 0
        });
        
        userDeposits[user].push(newDeposit);
        userTotalDeposits[user] = userTotalDeposits[user].add(amount);
        totalActiveDeposits = totalActiveDeposits.add(amount);
        
        emit CDyieldDepositRecorded(user, amount, duration, multiplier);
    }
    
    /// @dev Calculate duration multiplier
    function calculateDurationMultiplier(uint256 duration) public pure returns (uint256) {
        if (duration < 30 days) {
            return SHORT_TERM_MULTIPLIER;
        } else if (duration < 90 days) {
            return MEDIUM_TERM_MULTIPLIER;
        } else {
            return LONG_TERM_MULTIPLIER;
        }
    }
    
    /// @dev Calculate pending rewards for a user
    function calculatePendingRewards(address user) public view returns (uint256) {
        if (!distributionActive) {
            return 0;
        }
        
        uint256 totalRewards = 0;
        CDyieldDeposit[] memory deposits = userDeposits[user];
        
        for (uint256 i = 0; i < deposits.length; i++) {
            if (!deposits[i].isActive) continue;
            
            uint256 depositRewards = calculateDepositRewards(deposits[i]);
            totalRewards = totalRewards.add(depositRewards);
        }
        
        return totalRewards;
    }
    
    /// @dev Calculate rewards for a specific deposit
    function calculateDepositRewards(CDyieldDeposit memory deposit) public view returns (uint256) {
        uint256 currentTime = block.timestamp;
        uint256 timeElapsed = currentTime.sub(deposit.timestamp);
        
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
        
        // Calculate rewards: amount * rate * time * multiplier / 100
        uint256 rewards = deposit.amount
            .mul(currentRate)
            .mul(timeElapsed)
            .mul(deposit.rewardMultiplier)
            .div(100)
            .div(1e18); // Normalize XFG amount
        
        return rewards;
    }
    
    /// @dev Claim pending CDyield rewards
    function claimRewards() external nonReentrant {
        require(distributionActive, "Distribution paused");
        
        uint256 pendingRewards = calculatePendingRewards(msg.sender);
        require(pendingRewards > 0, "No rewards to claim");
        
        // Check if we can mint the required amount
        require(
            coldToken.totalSupply().add(pendingRewards) <= coldToken.cap(),
            "Would exceed COLD token cap"
        );
        
        // Update claimed rewards for all active deposits
        CDyieldDeposit[] storage deposits = userDeposits[msg.sender];
        for (uint256 i = 0; i < deposits.length; i++) {
            if (deposits[i].isActive) {
                deposits[i].claimedRewards = deposits[i].claimedRewards.add(
                    calculateDepositRewards(deposits[i])
                );
            }
        }
        
        // Update total distributed
        totalDistributed = totalDistributed.add(pendingRewards);
        
        // Mint COLD tokens to user
        coldToken.mintYield(msg.sender, pendingRewards);
        
        emit CDyieldRewardsClaimed(msg.sender, pendingRewards);
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
    
    /// @dev Get user deposit statistics
    function getUserDepositStats(address user) 
        external 
        view 
        returns (
            uint256 totalDeposits,
            uint256 activeDeposits,
            uint256 pendingRewards,
            uint256 totalClaimed
        ) 
    {
        uint256 activeCount = 0;
        uint256 totalClaimedAmount = 0;
        
        CDyieldDeposit[] memory deposits = userDeposits[user];
        for (uint256 i = 0; i < deposits.length; i++) {
            if (deposits[i].isActive) {
                activeCount = activeCount.add(deposits[i].amount);
            }
            totalClaimedAmount = totalClaimedAmount.add(deposits[i].claimedRewards);
        }
        
        return (
            userTotalDeposits[user],
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
            uint256 totalActiveDeposits,
            bool distributionActive
        ) 
    {
        return (
            totalDistributed,
            getRemainingCold(),
            getCurrentPhase(),
            getCurrentPhaseRate(),
            totalActiveDeposits,
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
    
    /// @dev Grant YIELD_MINTER_ROLE to this contract
    function grantYieldMinterRole() external onlyRole(DAO_ADMIN_ROLE) {
        coldToken.grantRole(coldToken.YIELD_MINTER_ROLE(), address(this));
    }
}
