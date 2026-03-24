// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGateway.sol";
import "@axelar-network/axelar-gmp-sdk-solidity/contracts/interfaces/IAxelarGasService.sol";
import "../interfaces/IBridgeAdapter.sol";

contract AxelarAdapter is IBridgeAdapter {
    IAxelarGateway public immutable gateway;
    IAxelarGasService public immutable gasService;
    address public immutable router;
    uint256 public feeHintWei;

    error NotRouter();
    error InvalidAddress();
    error InsufficientFee();
    event FeeHintUpdated(uint256 feeHintWei);

    modifier onlyRouter() {
        if (msg.sender != router) revert NotRouter();
        _;
    }

    constructor(address gateway_, address gasService_, address router_, uint256 feeHintWei_) {
        if (gateway_ == address(0) || gasService_ == address(0) || router_ == address(0)) {
            revert InvalidAddress();
        }

        gateway = IAxelarGateway(gateway_);
        gasService = IAxelarGasService(gasService_);
        router = router_;
        feeHintWei = feeHintWei_;
    }

    function setFeeHint(uint256 feeHintWei_) external onlyRouter {
        feeHintWei = feeHintWei_;
        emit FeeHintUpdated(feeHintWei_);
    }

    function estimateFee(
        string calldata,
        string calldata,
        bytes calldata
    ) external view override returns (uint256) {
        return feeHintWei;
    }

    function dispatchPayload(
        string calldata destChain,
        string calldata destAddress,
        bytes calldata payload
    ) external payable override onlyRouter {
        if (msg.value == 0) revert InsufficientFee();

        gasService.payNativeGasForContractCall{value: msg.value}(
            address(this),
            destChain,
            destAddress,
            payload,
            tx.origin
        );

        gateway.callContract(destChain, destAddress, payload);
    }
}