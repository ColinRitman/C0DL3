// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Capped.sol";
import "@openzeppelin/contracts/access/AccessControlEnumerable.sol";

/// @title C0LD Token (CD) – DAO-governed capped supply token for COLↁAO
/// @notice Max supply 20 * 10^12 units (decimals = 12). Mintable only by authorised minter contracts.
contract ColdToken is ERC20Capped, AccessControlEnumerable {
    /// @dev 12-decimals version of 1 token.
    uint256 private constant ONE = 10 ** 12;

    /// @dev Max supply: 20 COLD (20 * 10^12).
    uint256 private constant MAX_SUPPLY = 20 * ONE;

    // --- Roles ---
    bytes32 public constant DAO_ADMIN_ROLE    = keccak256("DAO_ADMIN_ROLE");
    bytes32 public constant YIELD_MINTER_ROLE = keccak256("YIELD_MINTER_ROLE");
    bytes32 public constant LP_MINTER_ROLE    = keccak256("LP_MINTER_ROLE");
    bytes32 public constant STAKE_MINTER_ROLE = keccak256("STAKE_MINTER_ROLE");

    constructor(address daoTreasury)
        ERC20("C0LD Token", "COLD")
        ERC20Capped(MAX_SUPPLY)
    {
        require(daoTreasury != address(0), "dao zero");
        _setupRole(DAO_ADMIN_ROLE, daoTreasury);
        _setRoleAdmin(YIELD_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(LP_MINTER_ROLE, DAO_ADMIN_ROLE);
        _setRoleAdmin(STAKE_MINTER_ROLE, DAO_ADMIN_ROLE);
    }

    /// @dev Override decimals to 12.
    function decimals() public pure override returns (uint8) {
        return 12;
    }

    // --------- Mint functions (only authorised contracts) ---------

    function mintYield(address to, uint256 amount) external onlyRole(YIELD_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    function mintLpRewards(address to, uint256 amount) external onlyRole(LP_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    function mintStakingRewards(address to, uint256 amount) external onlyRole(STAKE_MINTER_ROLE) {
        _mintChecked(to, amount);
    }

    // Internal mint with cap enforcement
    function _mintChecked(address to, uint256 amount) internal {
        require(totalSupply() + amount <= cap(), "cap exceeded");
        _mint(to, amount);
    }

    // Disallow transfers before any supply exists (optional)
    function _beforeTokenTransfer(address from, address to, uint256 amount) internal override(ERC20) {
        super._beforeTokenTransfer(from, to, amount);
    }
}
