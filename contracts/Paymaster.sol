// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title Paymaster — Gas Payment Abstraction for COLDL3
/// @notice Pays gas on behalf of AA wallet users, breaking the sender-gas link.
/// @dev The paymaster holds a public balance and authorizes gas payments via
///      Schnorr-signed approvals. This separates "who pays gas" from "who
///      initiated the transaction" — a core privacy property.
contract Paymaster {
    bytes32 public ownerPubkey;
    uint256 public balance; // Paymaster's public balance (for gas payments)
    mapping(bytes32 => bool) public usedApprovals; // prevent replay

    address constant SCHNORR_VERIFY = address(0x0106);

    event GasPaid(address indexed wallet, uint256 gasAmount);
    event Deposited(uint256 amount);
    event Withdrawn(uint256 amount);

    constructor(bytes32 _ownerPubkey) payable {
        ownerPubkey = _ownerPubkey;
        balance = msg.value;
    }

    /// Deposit gas funds into the paymaster.
    function deposit() external payable {
        balance += msg.value;
        emit Deposited(msg.value);
    }

    /// Pay gas for a UserOperation.
    /// Called by the sequencer during block execution.
    /// @param wallet The AA wallet address this gas payment covers
    /// @param gasAmount Gas cost in fwei
    /// @param approvalHash Unique hash of the paymaster approval (prevents replay)
    /// @param schnorrR Schnorr signature R point
    /// @param schnorrS Schnorr signature s scalar
    function payGas(
        address wallet,
        uint256 gasAmount,
        bytes32 approvalHash,
        bytes32 schnorrR,
        bytes32 schnorrS
    ) external {
        require(!usedApprovals[approvalHash], "approval already used");
        require(balance >= gasAmount, "insufficient paymaster balance");

        // Verify paymaster approved this gas payment
        bytes memory msg_ = abi.encodePacked(
            "C0DL3:paymaster:",
            approvalHash,
            gasAmount
        );
        require(
            verifySchnorr(ownerPubkey, msg_, schnorrR, schnorrS),
            "invalid paymaster sig"
        );

        usedApprovals[approvalHash] = true;
        balance -= gasAmount;
        emit GasPaid(wallet, gasAmount);
    }

    /// Withdraw unused funds (owner only, requires Schnorr auth).
    /// @param amount Amount to withdraw in fwei
    /// @param to Recipient address
    /// @param schnorrR Schnorr signature R point
    /// @param schnorrS Schnorr signature s scalar
    function withdraw(
        uint256 amount,
        address payable to,
        bytes32 schnorrR,
        bytes32 schnorrS
    ) external {
        require(balance >= amount, "insufficient balance");

        bytes memory msg_ = abi.encodePacked(
            "C0DL3:paymaster_withdraw:",
            amount,
            to
        );
        require(
            verifySchnorr(ownerPubkey, msg_, schnorrR, schnorrS),
            "invalid owner sig"
        );

        balance -= amount;
        to.transfer(amount);
        emit Withdrawn(amount);
    }

    function verifySchnorr(
        bytes32 pubkey, bytes memory message,
        bytes32 sigR, bytes32 sigS
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(pubkey, sigR, sigS, uint32(message.length), message);
        (bool ok, bytes memory result) = SCHNORR_VERIFY.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }
}
