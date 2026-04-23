
pragma solidity ^0.8.20;


interface ITransparentVerifier {
	function verifyProof(
		bytes calldata _proof,
		uint256[7] calldata _pubSignals
	) external view returns (bool);
}
