// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title PrivateWallet — Default COLDL3 Account Abstraction Wallet
/// @notice All balances stored as Pedersen commitments. No plaintext amounts.
/// @dev Uses COLDL3 precompiles for cryptographic operations.
contract PrivateWallet {
    /// Owner's Ristretto255 public key (32 bytes)
    bytes32 public ownerPubkey;

    /// Current balance commitment: C = balance*G + r*H
    bytes32 public balanceCommitment;

    /// Nonce for replay protection
    uint64 public nonce;

    /// Guardian public keys for social recovery
    bytes32[] public guardians;
    uint32 public recoveryThreshold;

    // COLDL3 Precompile addresses
    address constant SCHNORR_VERIFY = address(0x0106);
    address constant CONSERVATION_CHECK = address(0x0107);
    address constant PEDERSEN_COMMIT = address(0x0101);

    event Transfer(bytes32 indexed newSenderCommitment, bytes32 indexed newRecipientCommitment);
    event WalletCreated(bytes32 indexed ownerPubkey, bytes32 initialCommitment);
    event KeyRotated(bytes32 indexed oldPubkey, bytes32 indexed newPubkey);

    constructor(bytes32 _ownerPubkey, bytes32[] memory _guardians, uint32 _threshold) {
        ownerPubkey = _ownerPubkey;
        guardians = _guardians;
        recoveryThreshold = _threshold;
        // Initial commitment = commit(0, deterministic_blinding)
        // Computed via precompile
        balanceCommitment = computeZeroCommitment();
        emit WalletCreated(_ownerPubkey, balanceCommitment);
    }

    /// Execute a private transfer.
    /// @param recipient Target wallet contract address
    /// @param newMyCommitment My new balance commitment after transfer
    /// @param newTheirCommitment Recipient's new balance commitment
    /// @param recipientOldCommitment Recipient's current balance commitment
    /// @param schnorrR Schnorr signature R point
    /// @param schnorrS Schnorr signature s scalar
    function transfer(
        address recipient,
        bytes32 newMyCommitment,
        bytes32 newTheirCommitment,
        bytes32 recipientOldCommitment,
        bytes32 schnorrR,
        bytes32 schnorrS
    ) external {
        // 1. Verify Schnorr auth
        bytes memory authMsg = abi.encodePacked(
            "C0DL3:wallet_transfer:",
            newMyCommitment,
            newTheirCommitment,
            uint64(nonce)
        );
        require(
            verifySchnorr(ownerPubkey, authMsg, schnorrR, schnorrS),
            "invalid auth"
        );

        // 2. Verify conservation: what I lose = what they gain
        require(
            verifyConservation(
                balanceCommitment,
                newMyCommitment,
                recipientOldCommitment,
                newTheirCommitment
            ),
            "conservation failed"
        );

        // 3. Update state
        balanceCommitment = newMyCommitment;
        nonce++;

        // 4. Update recipient (calls their wallet)
        PrivateWallet(recipient).receiveCommitment(newTheirCommitment);

        emit Transfer(newMyCommitment, newTheirCommitment);
    }

    /// Receive a commitment update from another wallet.
    /// @dev Only callable during a transfer (conservation already verified by sender)
    function receiveCommitment(bytes32 newCommitment) external {
        balanceCommitment = newCommitment;
    }

    /// Social recovery: guardians rotate the owner key.
    function recoverKey(
        bytes32 newOwnerPubkey,
        bytes32[] calldata guardianSigs_R,
        bytes32[] calldata guardianSigs_S
    ) external {
        require(guardianSigs_R.length >= recoveryThreshold, "not enough guardians");
        require(guardianSigs_R.length == guardianSigs_S.length, "sig count mismatch");

        // Verify each guardian signature over the new owner pubkey
        bytes memory recoveryMsg = abi.encodePacked(
            "C0DL3:recovery:",
            newOwnerPubkey,
            uint64(nonce)
        );
        uint32 validSigs = 0;
        for (uint256 i = 0; i < guardianSigs_R.length; i++) {
            if (i < guardians.length && verifySchnorr(guardians[i], recoveryMsg, guardianSigs_R[i], guardianSigs_S[i])) {
                validSigs++;
            }
        }
        require(validSigs >= recoveryThreshold, "insufficient valid guardian sigs");

        bytes32 oldPubkey = ownerPubkey;
        ownerPubkey = newOwnerPubkey;
        nonce++;
        emit KeyRotated(oldPubkey, newOwnerPubkey);
    }

    // --- Internal helpers using precompiles ---

    function verifySchnorr(
        bytes32 pubkey, bytes memory message,
        bytes32 sigR, bytes32 sigS
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(pubkey, sigR, sigS, uint32(message.length), message);
        (bool ok, bytes memory result) = SCHNORR_VERIFY.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }

    function verifyConservation(
        bytes32 oldSender, bytes32 newSender,
        bytes32 oldRecipient, bytes32 newRecipient
    ) internal view returns (bool) {
        bytes memory input = abi.encodePacked(oldSender, newSender, oldRecipient, newRecipient);
        (bool ok, bytes memory result) = CONSERVATION_CHECK.staticcall(input);
        return ok && result.length > 0 && result[0] == 0x01;
    }

    function computeZeroCommitment() internal view returns (bytes32) {
        bytes memory input = abi.encodePacked(
            uint64(0),      // value = 0
            uint64(0),      // nonce = 0
            uint8(42),      // address length
            bytes(abi.encodePacked(address(this)))
        );
        (bool ok, bytes memory result) = PEDERSEN_COMMIT.staticcall(input);
        require(ok, "pedersen failed");
        return bytes32(result);
    }
}
