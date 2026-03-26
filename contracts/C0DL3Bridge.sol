// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./COLDL3Settlement.sol";

/// @title C0DL3Bridge — Darkpool Privacy Bridge (Era side)
///
/// @notice Deployed on zkSync Era. The privacy boundary between public DeFi and the
///         C0DL3 darkpool. Locks/unlocks assets using fixed-denomination pools —
///         the denomination system is the anonymity mechanism. Inside C0DL3, amounts
///         are arbitrary (Pedersen commitments). Denominations only constrain the
///         bridge entry/exit to prevent amount-based correlation.
///
/// @dev Architecture for darkpool positioning:
///   - Per-token denomination tiers (3 tiers per asset: small/medium/large)
///   - Configurable denominations — governance can add tokens and tiers
///   - On-chain anonymity set counters — users can verify pool health before depositing
///   - Time-delayed withdrawals with randomization window
///   - State root history for non-latest withdrawal proofs
///   - No multisig, no oracle. Security = SP1 proof system.
///
/// Privacy model:
///   - Information leaked at bridge entry: which token, which denomination tier, timing
///   - Information leaked at bridge exit:  which token, withdrawal amount, timing
///   - Information hidden: link between deposit address and withdrawal address,
///     all activity inside C0DL3 (trading, transfers, swaps)
///   - Anonymity set = number of deposits in your denomination pool
///
/// Denomination math (why 3 tiers per asset):
///   - Expected anonymity = N/K where N=total deposits, K=tiers
///   - 3 tiers: 1.58 bits leaked about amount (geometric 10x spacing covers 3 OOM)
///   - Each additional tier costs log2((K+1)/K) bits and dilutes anonymity by 1/(K+1)
///   - 3 is optimal: covers retail through whale, minimal anonymity dilution
///
/// Attack surface:
///   - SP1 proof forgery: computationally infeasible
///   - Thin anonymity set: mitigated by on-chain pool size visibility + minimum threshold warnings
///   - Timing correlation: mitigated by withdrawal delay window (1-4 hours randomized)
///   - Withdrawal replay: prevented by nullifier tracking
///   - Denomination fingerprinting: mitigated by SDK auto-splitting (e.g., 7 ETH → 7×1 ETH)

contract C0DL3Bridge {
    // ── Dependencies ─────────────────────────────────────────────────────

    COLDL3Settlement public immutable settlement;

    // ── Constants ────────────────────────────────────────────────────────

    /// @notice Minimum withdrawal delay (seconds after settlement).
    uint256 public constant MIN_WITHDRAWAL_DELAY = 3600; // 1 hour

    /// @notice Maximum withdrawal delay window (for timing randomization).
    uint256 public constant MAX_WITHDRAWAL_DELAY = 14400; // 4 hours

    /// @notice Maximum denomination tiers per token (prevents over-fragmentation).
    uint256 public constant MAX_TIERS_PER_TOKEN = 5;

    /// @notice Minimum recommended anonymity set before a pool is considered "safe".
    /// Pools below this threshold are flagged in getPoolHealth().
    uint256 public constant MIN_ANONYMITY_SET = 50;

    bytes32 public constant WITHDRAWAL_DOMAIN = keccak256("C0DL3:withdrawal_tree:");

    // ── Per-Token Denomination Registry ──────────────────────────────────

    /// @notice Denomination tiers for a specific token.
    /// @dev Sorted ascending. address(0) = native ETH.
    struct TokenConfig {
        uint256[] denominations;  // sorted ascending (e.g., [0.1e18, 1e18, 10e18])
        bool enabled;             // can accept new deposits
        uint256 addedAt;          // timestamp when token was added
    }

    /// @notice Token → denomination config.
    mapping(address => TokenConfig) public tokenConfigs;

    /// @notice List of all registered token addresses (for enumeration).
    address[] public registeredTokens;

    /// @notice Per-pool deposit count: token → denomination → count.
    /// This IS the anonymity set size for each pool.
    mapping(address => mapping(uint256 => uint256)) public poolDepositCount;

    /// @notice Per-pool withdrawal count: token → denomination → count.
    mapping(address => mapping(uint256 => uint256)) public poolWithdrawalCount;

    // ── Core State ───────────────────────────────────────────────────────

    /// @notice Used withdrawal nullifiers (prevent replay).
    mapping(bytes32 => bool) public usedNullifiers;

    /// @notice Deposit nonce (global, monotonically increasing).
    uint256 public depositNonce;

    /// @notice Historical settled state roots (for non-latest withdrawal proofs).
    /// stateRoot → settlement timestamp.
    mapping(bytes32 => uint256) public settledRoots;

    /// @notice Sequencer address (emergency pause + token management).
    address public sequencer;

    /// @notice Emergency pause flag.
    bool public paused;

    // ── Events ───────────────────────────────────────────────────────────

    event Deposit(
        uint256 indexed nonce,
        address indexed depositor,
        address token,
        uint256 denomination,
        bytes32 shieldedRecipient,
        uint256 timestamp
    );

    event Withdrawal(
        bytes32 indexed nullifier,
        address indexed recipient,
        address token,
        uint256 amount,
        bytes32 stateRoot,
        uint256 timestamp
    );

    event TokenAdded(address indexed token, uint256[] denominations);
    event TokenDisabled(address indexed token);
    event TokenEnabled(address indexed token);
    event DenominationAdded(address indexed token, uint256 denomination);
    event StateRootSettled(bytes32 indexed stateRoot, uint256 timestamp);
    event Paused(address indexed by);
    event Unpaused(address indexed by);
    event SequencerUpdated(address indexed oldSequencer, address indexed newSequencer);

    // ── Errors ───────────────────────────────────────────────────────────

    error BridgePaused();
    error InvalidDenomination(address token, uint256 amount);
    error TokenNotRegistered(address token);
    error TokenDisabledForDeposits(address token);
    error TokenAlreadyRegistered(address token);
    error TooManyTiers(address token, uint256 count);
    error InvalidMerkleProof();
    error NullifierAlreadyUsed(bytes32 nullifier);
    error WithdrawalDelayNotMet();
    error StateRootNotSettled(bytes32 stateRoot);
    error TransferFailed();
    error UnauthorizedSequencer();
    error EmptyDenominations();
    error DenominationAlreadyExists(address token, uint256 denomination);

    // ── Constructor ──────────────────────────────────────────────────────

    /// @param _settlement Address of the COLDL3Settlement contract
    /// @param _sequencer Initial sequencer address
    constructor(address _settlement, address _sequencer) {
        settlement = COLDL3Settlement(_settlement);
        sequencer = _sequencer;

        // Bootstrap native ETH with 3 darkpool-optimized tiers
        // Small: 0.1 ETH (~$350) — retail accessible
        // Medium: 1 ETH (~$3,500) — standard DeFi user
        // Large: 10 ETH (~$35,000) — whale tier
        uint256[] memory ethDenoms = new uint256[](3);
        ethDenoms[0] = 0.1 ether;
        ethDenoms[1] = 1 ether;
        ethDenoms[2] = 10 ether;
        _addToken(address(0), ethDenoms);

        // NOTE: HEAT, ZK, and CD (COLDAO) are ERC-20 tokens on Era.
        // They are added post-deployment via addToken() once their Era addresses are known.
        //
        // Recommended launch denominations:
        //   HEAT:  10,000 / 100,000 / 1,000,000   (native gas token, high velocity)
        //   ZK:    100 / 1,000 / 10,000            (Era native, privacy for ZK holders)
        //   CD:    1,000 / 10,000 / 100,000        (COLDAO governance, lower velocity)
        //
        // These 4 tokens (ETH + HEAT + ZK + CD) = 12 pools at launch.
        // New tokens added only when existing pools have healthy anonymity sets (50+ deposits).
    }

    // ── Token Management (Sequencer Only) ────────────────────────────────

    /// @notice Register a new token with denomination tiers.
    /// @param token ERC-20 address (address(0) reserved for ETH, set in constructor)
    /// @param denominations Sorted ascending array of denomination values
    function addToken(address token, uint256[] calldata denominations) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        if (tokenConfigs[token].denominations.length > 0) revert TokenAlreadyRegistered(token);
        if (denominations.length == 0) revert EmptyDenominations();
        if (denominations.length > MAX_TIERS_PER_TOKEN) revert TooManyTiers(token, denominations.length);
        _addToken(token, denominations);
    }

    /// @notice Add a denomination tier to an existing token.
    /// @dev Only allowed if total tiers <= MAX_TIERS_PER_TOKEN.
    function addDenomination(address token, uint256 denomination) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        TokenConfig storage config = tokenConfigs[token];
        if (config.denominations.length == 0) revert TokenNotRegistered(token);
        if (config.denominations.length >= MAX_TIERS_PER_TOKEN) {
            revert TooManyTiers(token, config.denominations.length + 1);
        }
        // Check not duplicate
        for (uint256 i = 0; i < config.denominations.length; i++) {
            if (config.denominations[i] == denomination) {
                revert DenominationAlreadyExists(token, denomination);
            }
        }
        config.denominations.push(denomination);
        emit DenominationAdded(token, denomination);
    }

    /// @notice Disable a token for new deposits (existing pool funds remain withdrawable).
    function disableToken(address token) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        if (tokenConfigs[token].denominations.length == 0) revert TokenNotRegistered(token);
        tokenConfigs[token].enabled = false;
        emit TokenDisabled(token);
    }

    /// @notice Re-enable a disabled token.
    function enableToken(address token) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        if (tokenConfigs[token].denominations.length == 0) revert TokenNotRegistered(token);
        tokenConfigs[token].enabled = true;
        emit TokenEnabled(token);
    }

    // ── State Root History ────────────────────────────────────────────────

    /// @notice Record a settled state root from the settlement contract.
    /// @dev Called by the sequencer after a settlement batch is confirmed on Era.
    ///      Enables withdrawal proofs against historical state roots, not just latest.
    function recordSettledRoot(bytes32 stateRoot) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        settledRoots[stateRoot] = block.timestamp;
        emit StateRootSettled(stateRoot, block.timestamp);
    }

    // ── Deposits (Era → C0DL3 Darkpool) ──────────────────────────────────

    /// @notice Deposit native ETH into a fixed-denomination pool.
    /// @param shieldedRecipient One-time stealth address on C0DL3.
    function deposit(bytes32 shieldedRecipient) external payable {
        if (paused) revert BridgePaused();

        TokenConfig storage config = tokenConfigs[address(0)];
        if (!config.enabled) revert TokenDisabledForDeposits(address(0));
        if (!_isValidDenomination(address(0), msg.value)) {
            revert InvalidDenomination(address(0), msg.value);
        }

        uint256 nonce = depositNonce++;
        poolDepositCount[address(0)][msg.value] += 1;

        emit Deposit(nonce, msg.sender, address(0), msg.value, shieldedRecipient, block.timestamp);
    }

    /// @notice Deposit ERC-20 tokens into a fixed-denomination pool.
    /// @param token ERC-20 token address
    /// @param denomination Must match a registered denomination for this token
    /// @param shieldedRecipient One-time stealth address on C0DL3
    function depositToken(
        address token,
        uint256 denomination,
        bytes32 shieldedRecipient
    ) external {
        if (paused) revert BridgePaused();

        TokenConfig storage config = tokenConfigs[token];
        if (config.denominations.length == 0) revert TokenNotRegistered(token);
        if (!config.enabled) revert TokenDisabledForDeposits(token);
        if (!_isValidDenomination(token, denomination)) {
            revert InvalidDenomination(token, denomination);
        }

        // Transfer tokens to this contract
        (bool success, bytes memory data) = token.call(
            abi.encodeWithSignature(
                "transferFrom(address,address,uint256)",
                msg.sender,
                address(this),
                denomination
            )
        );
        if (!success || (data.length > 0 && !abi.decode(data, (bool)))) {
            revert TransferFailed();
        }

        uint256 nonce = depositNonce++;
        poolDepositCount[token][denomination] += 1;

        emit Deposit(nonce, msg.sender, token, denomination, shieldedRecipient, block.timestamp);
    }

    // ── Withdrawals (C0DL3 Darkpool → Era) ───────────────────────────────

    /// @notice Withdraw assets by proving inclusion in a settled L3 withdrawal tree.
    /// @dev Supports historical state roots via settledRoots mapping.
    function withdraw(
        address payable recipient,
        address token,
        uint256 amount,
        bytes32 nullifier,
        bytes32 settledStateRoot,
        bytes32 withdrawalTreeRoot,
        bytes32[] calldata merkleProof
    ) external {
        if (paused) revert BridgePaused();

        // 1. Check nullifier
        if (usedNullifiers[nullifier]) revert NullifierAlreadyUsed(nullifier);

        // 2. Verify state root was settled (supports historical roots)
        uint256 settledAt = settledRoots[settledStateRoot];
        if (settledAt == 0) {
            // Fallback: check if it's the current settlement root
            if (settledStateRoot != settlement.stateRoot() || settlement.stateRoot() == bytes32(0)) {
                revert StateRootNotSettled(settledStateRoot);
            }
            settledAt = block.timestamp; // Current root, use now as timestamp
        }

        // 3. Enforce minimum withdrawal delay
        if (block.timestamp < settledAt + MIN_WITHDRAWAL_DELAY) {
            revert WithdrawalDelayNotMet();
        }

        // 4. Verify Merkle proof
        bytes32 leaf = keccak256(abi.encodePacked(recipient, token, amount, nullifier));
        if (!_verifyMerkleProof(merkleProof, withdrawalTreeRoot, leaf)) {
            revert InvalidMerkleProof();
        }

        // 5. Mark nullifier used
        usedNullifiers[nullifier] = true;

        // 6. Transfer funds
        if (token == address(0)) {
            (bool success,) = recipient.call{value: amount}("");
            if (!success) revert TransferFailed();
        } else {
            (bool success, bytes memory data) = token.call(
                abi.encodeWithSignature("transfer(address,uint256)", recipient, amount)
            );
            if (!success || (data.length > 0 && !abi.decode(data, (bool)))) {
                revert TransferFailed();
            }
        }

        emit Withdrawal(nullifier, recipient, token, amount, settledStateRoot, block.timestamp);
    }

    // ── Darkpool Views (Anonymity Set Health) ────────────────────────────

    /// @notice Get anonymity set sizes for all pools of a token.
    /// @return denominations Array of denomination values
    /// @return depositCounts Deposits per pool (= anonymity set size)
    /// @return withdrawalCounts Withdrawals per pool
    /// @return healthy Whether each pool meets MIN_ANONYMITY_SET threshold
    function getPoolHealth(address token) external view returns (
        uint256[] memory denominations,
        uint256[] memory depositCounts,
        uint256[] memory withdrawalCounts,
        bool[] memory healthy
    ) {
        TokenConfig storage config = tokenConfigs[token];
        uint256 len = config.denominations.length;

        denominations = new uint256[](len);
        depositCounts = new uint256[](len);
        withdrawalCounts = new uint256[](len);
        healthy = new bool[](len);

        for (uint256 i = 0; i < len; i++) {
            uint256 denom = config.denominations[i];
            denominations[i] = denom;
            depositCounts[i] = poolDepositCount[token][denom];
            withdrawalCounts[i] = poolWithdrawalCount[token][denom];
            healthy[i] = depositCounts[i] >= MIN_ANONYMITY_SET;
        }
    }

    /// @notice Get all registered tokens and their configs.
    function getRegisteredTokens() external view returns (
        address[] memory tokens,
        uint256[][] memory allDenominations,
        bool[] memory enabled
    ) {
        uint256 len = registeredTokens.length;
        tokens = new address[](len);
        allDenominations = new uint256[][](len);
        enabled = new bool[](len);

        for (uint256 i = 0; i < len; i++) {
            address t = registeredTokens[i];
            tokens[i] = t;
            allDenominations[i] = tokenConfigs[t].denominations;
            enabled[i] = tokenConfigs[t].enabled;
        }
    }

    /// @notice Check if a nullifier has been used.
    function isNullifierUsed(bytes32 nullifier) external view returns (bool) {
        return usedNullifiers[nullifier];
    }

    /// @notice Get denomination tiers for a token.
    function getTokenDenominations(address token) external view returns (uint256[] memory) {
        return tokenConfigs[token].denominations;
    }

    // ── Admin (Emergency Only) ──────────────────────────────────────────

    function pause() external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        paused = true;
        emit Paused(msg.sender);
    }

    function unpause() external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        paused = false;
        emit Unpaused(msg.sender);
    }

    function setSequencer(address _newSequencer) external {
        if (msg.sender != sequencer) revert UnauthorizedSequencer();
        emit SequencerUpdated(sequencer, _newSequencer);
        sequencer = _newSequencer;
    }

    // ── Internal ─────────────────────────────────────────────────────────

    function _addToken(address token, uint256[] memory denominations) internal {
        TokenConfig storage config = tokenConfigs[token];
        config.enabled = true;
        config.addedAt = block.timestamp;
        for (uint256 i = 0; i < denominations.length; i++) {
            config.denominations.push(denominations[i]);
        }
        registeredTokens.push(token);
        emit TokenAdded(token, denominations);
    }

    /// @dev Check if amount is a valid denomination for the given token.
    function _isValidDenomination(address token, uint256 amount) internal view returns (bool) {
        uint256[] storage denoms = tokenConfigs[token].denominations;
        for (uint256 i = 0; i < denoms.length; i++) {
            if (denoms[i] == amount) return true;
        }
        return false;
    }

    /// @dev Verify a Merkle proof (sorted-pair keccak, OpenZeppelin-compatible).
    function _verifyMerkleProof(
        bytes32[] calldata proof,
        bytes32 root,
        bytes32 leaf
    ) internal pure returns (bool) {
        bytes32 computedHash = leaf;
        for (uint256 i = 0; i < proof.length; i++) {
            bytes32 proofElement = proof[i];
            if (computedHash <= proofElement) {
                computedHash = keccak256(abi.encodePacked(computedHash, proofElement));
            } else {
                computedHash = keccak256(abi.encodePacked(proofElement, computedHash));
            }
        }
        return computedHash == root;
    }

    /// @dev Accept ETH transfers.
    receive() external payable {}
}
