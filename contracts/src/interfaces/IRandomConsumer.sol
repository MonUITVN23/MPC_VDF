// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title IRandomConsumer
/// @notice Interface for dApps that consume randomness from the protocol
interface IRandomConsumer {
    function fulfillRandomness(uint256 requestId, uint256 randomResult) external;
}
