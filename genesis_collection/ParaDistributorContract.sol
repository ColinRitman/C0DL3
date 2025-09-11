// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/math/SafeMath.sol";

/// @title PARA Token Distributor – Streams PARA rewards to active participants
/// @notice Distributes 1.0 PARA token over 10 years to participants across all reward mechanisms
contract ParaDistributorContract is AccessControlEnumerable, ReentrancyGuard {
    using SafeMath for uint256;

    // --- Constants ---
    uint256 private constant ONE_PARA = 1 * 10**18; // 1.0 PARA with 18 decimals
    uint256 private constant DISTRIBUTION_PERIOD = 10 * 365 * 24 * 60 * 60; // 10 years in seconds
    uint256 private constant PHASE_DURATION = 2 * 365 * 24 * 60 * 60; // 2 years per phase
    
    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant ACTIVITY_UPDATER_ROLE = keccak256("ACTIVITY_UPDATER_ROLE");
    
    // --- State Variables ---
    IERC20 public immutable paraToken;
    uint256 public immutable startTime;
    uint256 public totalDistributed;
    bool public distributionActive;
    
    // Participant activity tracking
    struct ParticipantActivity {
        uint256 totalActivityScore;
        uint256 lastUpdateTime;
        uint256 totalClaimed;
        bool isActive;
    }
    
    mapping(address => ParticipantActivity) public participants;
    address[] public activeParticipants;
    
    // Phase-based distribution rates (per second)
    uint256[5] public phaseRates = [
        0.4e18 / (2 * 365 * 24 * 60 * 60), // Phase 1: 0.4 PARA over 2 years
        0.3e18 / (2 * 365 * 24 * 60 * 60), // Phase 2: 0.3 PARA over 2 years  
        0.2e18 / (2 * 365 * 24 * 60 * 60), // Phase 3: 0.2 PARA over 2 years
        0.1e18 / (2 * 365 * 24 * 60 * 60), // Phase 4: 0.1 PARA over 2 years
        0.0e18 / (2 * 365 * 24 * 60 * 60)  // Phase 5: 0.0 PARA (distribution complete)
    ];
    
    // --- Events ---
    event ParticipantRegistered(address indexed participant, uint256 activityScore);
    event ActivityUpdated(address indexed participant, uint256 newActivityScore);
    event ParaClaimed(address indexed participant, uint256 amount);
    event DistributionPaused();
    event DistributionResumed();
    
    constructor(address _paraToken, address daoTreasury) {
        require(_paraToken != address(0), "Para token zero address");
        require(daoTreasury != address(0), "DAO treasury zero address");
        
        paraToken = IERC20(_paraToken);
        startTime = block.timestamp;
        distributionActive = true;
        
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(ACTIVITY_UPDATER_ROLE, DAO_ADMIN_ROLE);
    }
    
    /// @dev Register a new participant with initial activity score
    function registerParticipant(address participant, uint256 initialActivityScore) 
        external 
        onlyRole(ACTIVITY_UPDATER_ROLE) 
    {
        require(participant != address(0), "Participant zero address");
        require(!participants[participant].isActive, "Participant already registered");
        
        participants[participant] = ParticipantActivity({
            totalActivityScore: initialActivityScore,
            lastUpdateTime: block.timestamp,
            totalClaimed: 0,
            isActive: true
        });
        
        activeParticipants.push(participant);
        
        emit ParticipantRegistered(participant, initialActivityScore);
    }
    
    /// @dev Update participant activity score
    function updateActivityScore(address participant, uint256 newActivityScore) 
        external 
        onlyRole(ACTIVITY_UPDATER_ROLE) 
    {
        require(participants[participant].isActive, "Participant not registered");
        
        participants[participant].totalActivityScore = newActivityScore;
        participants[participant].lastUpdateTime = block.timestamp;
        
        emit ActivityUpdated(participant, newActivityScore);
    }
    
    /// @dev Calculate pending PARA rewards for a participant
    function calculatePendingRewards(address participant) public view returns (uint256) {
        ParticipantActivity memory participantData = participants[participant];
        
        if (!participantData.isActive || !distributionActive) {
            return 0;
        }
        
        uint256 currentTime = block.timestamp;
        uint256 timeElapsed = currentTime.sub(participantData.lastUpdateTime);
        
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
        
        // Calculate rewards based on activity score and time
        uint256 rewards = participantData.totalActivityScore
            .mul(currentRate)
            .mul(timeElapsed)
            .div(1e18); // Normalize activity score
        
        return rewards;
    }
    
    /// @dev Claim pending PARA rewards
    function claimRewards() external nonReentrant {
        require(distributionActive, "Distribution paused");
        require(participants[msg.sender].isActive, "Not an active participant");
        
        uint256 pendingRewards = calculatePendingRewards(msg.sender);
        require(pendingRewards > 0, "No rewards to claim");
        
        // Check if we have enough PARA tokens
        uint256 contractBalance = paraToken.balanceOf(address(this));
        require(contractBalance >= pendingRewards, "Insufficient PARA balance");
        
        // Update participant data
        participants[msg.sender].lastUpdateTime = block.timestamp;
        participants[msg.sender].totalClaimed = participants[msg.sender].totalClaimed.add(pendingRewards);
        
        // Update total distributed
        totalDistributed = totalDistributed.add(pendingRewards);
        
        // Transfer PARA tokens
        paraToken.transfer(msg.sender, pendingRewards);
        
        emit ParaClaimed(msg.sender, pendingRewards);
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
    
    /// @dev Get total remaining PARA to distribute
    function getRemainingPara() public view returns (uint256) {
        return ONE_PARA.sub(totalDistributed);
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
    
    /// @dev Get participant statistics
    function getParticipantStats(address participant) 
        external 
        view 
        returns (
            uint256 totalActivityScore,
            uint256 lastUpdateTime,
            uint256 totalClaimed,
            uint256 pendingRewards,
            bool isActive
        ) 
    {
        ParticipantActivity memory participantData = participants[participant];
        return (
            participantData.totalActivityScore,
            participantData.lastUpdateTime,
            participantData.totalClaimed,
            calculatePendingRewards(participant),
            participantData.isActive
        );
    }
    
    /// @dev Get contract statistics
    function getContractStats() 
        external 
        view 
        returns (
            uint256 totalDistributed,
            uint256 remainingPara,
            uint256 currentPhase,
            uint256 currentPhaseRate,
            uint256 activeParticipantsCount,
            bool distributionActive
        ) 
    {
        return (
            totalDistributed,
            getRemainingPara(),
            getCurrentPhase(),
            getCurrentPhaseRate(),
            activeParticipants.length,
            distributionActive
        );
    }
    
    /// @dev Emergency function to recover stuck tokens (only PARA tokens)
    function emergencyRecoverPara(uint256 amount) external onlyRole(DAO_ADMIN_ROLE) {
        require(amount <= paraToken.balanceOf(address(this)), "Amount exceeds balance");
        paraToken.transfer(msg.sender, amount);
    }
}
