// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

/// @title Para Token – Genesis token with fixed supply
/// @notice Fixed supply of 1.0 token (18 decimals). Non-mintable after deployment.
contract ParaToken is ERC20, Ownable {
    /// @dev 18-decimals version of 1 token.
    uint256 private constant ONE_TOKEN = 1 * 10**18;
    
    /// @dev Fixed supply: 1.0 PARA (1 * 10^18).
    uint256 private constant FIXED_SUPPLY = ONE_TOKEN;

    constructor(address initialOwner)
        ERC20("Para Token", "PARA")
        Ownable(initialOwner)
    {
        require(initialOwner != address(0), "Owner cannot be zero address");
        
        // Mint the entire supply to the owner
        _mint(initialOwner, FIXED_SUPPLY);
    }

    /// @dev Override decimals to 18.
    function decimals() public pure override returns (uint8) {
        return 18;
    }

    /// @dev Get the fixed supply amount
    function getFixedSupply() external pure returns (uint256) {
        return FIXED_SUPPLY;
    }

    /// @dev Prevent any additional minting (supply is fixed)
    function _mint(address to, uint256 amount) internal override {
        require(totalSupply() == 0, "Supply already minted");
        require(amount == FIXED_SUPPLY, "Must mint exact fixed supply");
        super._mint(to, amount);
    }
}