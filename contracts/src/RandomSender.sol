// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGateway.sol";
import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGasService.sol";

contract RandomSender {
    uint256 public nextRequestId = 1;

    string public constant DESTINATION_CHAIN = "polygon-sepolia";
    string public destinationAddress;

    IAxelarGateway public immutable gateway;
    IAxelarGasService public immutable gasService;

    error NativeGasPaymentRequired();

    event LogRequest(
        uint256 indexed requestId,
        address indexed requester,
        bytes32 payloadHash,
        uint256 timestamp
    );

    event AxelarRequestDispatched(
        uint256 indexed requestId,
        string destinationChain,
        string destinationAddress,
        bytes32 payloadHash
    );

    constructor(address gateway_, address gasService_, string memory destinationAddress_) {
        require(gateway_ != address(0), "invalid gateway");
        require(gasService_ != address(0), "invalid gas service");
        require(bytes(destinationAddress_).length > 0, "invalid destination address");

        gateway = IAxelarGateway(gateway_);
        gasService = IAxelarGasService(gasService_);
        destinationAddress = destinationAddress_;
    }

    function requestRandomness(
        bytes calldata y,
        bytes calldata pi,
        bytes calldata seedCollective,
        bytes calldata modulus,
        bytes calldata blsSignature
    ) external payable returns (uint256 requestId) {
        if (msg.value == 0) revert NativeGasPaymentRequired();
        require(y.length > 0, "empty y");
        require(pi.length > 0, "empty pi");
        require(seedCollective.length > 0, "empty seed collective");
        require(modulus.length > 0, "empty modulus");
        require(blsSignature.length > 0, "empty bls signature");

        requestId = nextRequestId;
        unchecked {
            nextRequestId = requestId + 1;
        }

        bytes memory payload = abi.encode(requestId, y, pi, seedCollective, modulus, blsSignature);
        bytes32 payloadHash = keccak256(payload);

        emit LogRequest(requestId, msg.sender, payloadHash, block.timestamp);

        gasService.payNativeGasForContractCall{value: msg.value}(
            address(this),
            DESTINATION_CHAIN,
            destinationAddress,
            payload,
            msg.sender
        );

        gateway.callContract(DESTINATION_CHAIN, destinationAddress, payload);

        emit AxelarRequestDispatched(
            requestId,
            DESTINATION_CHAIN,
            destinationAddress,
            payloadHash
        );
    }
}
