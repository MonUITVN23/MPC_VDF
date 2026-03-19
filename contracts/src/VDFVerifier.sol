// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract VDFVerifier {
	error ModExpCallFailed();

	function verifyVDF(
		bytes memory base,
		bytes memory exponent,
		bytes memory modulus
	) internal view returns (bytes memory result) {
		uint256 baseLen = base.length;
		uint256 expLen = exponent.length;
		uint256 modLen = modulus.length;

		if (modLen == 0) {
			return new bytes(0);
		}

		uint256 inputLen = 96 + baseLen + expLen + modLen;
		bytes memory input = new bytes(inputLen);
		result = new bytes(modLen);
		bool success;

		assembly {
			let inputData := add(input, 32)

			mstore(inputData, baseLen)
			mstore(add(inputData, 32), expLen)
			mstore(add(inputData, 64), modLen)

			let dst := add(inputData, 96)

			{
				let src := add(base, 32)
				let words := div(add(baseLen, 31), 32)
				for { let i := 0 } lt(i, words) { i := add(i, 1) } {
					mstore(add(dst, mul(i, 32)), mload(add(src, mul(i, 32))))
				}
				dst := add(dst, baseLen)
			}

			{
				let src := add(exponent, 32)
				let words := div(add(expLen, 31), 32)
				for { let i := 0 } lt(i, words) { i := add(i, 1) } {
					mstore(add(dst, mul(i, 32)), mload(add(src, mul(i, 32))))
				}
				dst := add(dst, expLen)
			}

			{
				let src := add(modulus, 32)
				let words := div(add(modLen, 31), 32)
				for { let i := 0 } lt(i, words) { i := add(i, 1) } {
					mstore(add(dst, mul(i, 32)), mload(add(src, mul(i, 32))))
				}
			}

			success := staticcall(gas(), 0x05, inputData, inputLen, add(result, 32), modLen)
		}

		if (!success) {
			revert ModExpCallFailed();
		}
	}
}
