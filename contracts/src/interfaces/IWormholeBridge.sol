
pragma solidity ^0.8.20;



interface IWormholeBridge {
    function publishRandomnessResult(
        bytes calldata payload,
        uint32 nonce,
        uint8 consistencyLevel
    ) external payable returns (uint64 sequence);

    function receiveRandomnessResult(
        bytes calldata encodedVm
    ) external;

    function parseAndVerifyVM(
        bytes calldata encodedVm
    ) external view returns (bool valid, string memory reason);
}
