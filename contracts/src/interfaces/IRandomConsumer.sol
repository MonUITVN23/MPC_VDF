
pragma solidity ^0.8.20;



interface IRandomConsumer {
    function fulfillRandomness(uint256 requestId, uint256 randomResult) external;
}
