// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "../VDFVerifier.sol";

contract VDFVerifierMock is VDFVerifier {
    function verifyVDFPublic(
        bytes memory base,
        bytes memory exponent,
        bytes memory modulus
    ) public view returns (bytes memory) {
        return verifyVDF(base, exponent, modulus);
    }

    function verifyVDF_Unoptimized(
        bytes memory base,
        bytes memory exponent,
        bytes memory modulus
    ) public pure returns (bytes memory) {
        uint256 modLen = modulus.length;
        bytes memory out = new bytes(modLen);
        if (modLen == 0) {
            return out;
        }

        uint256 b = _bytesToUint256(base);
        uint256 m = _bytesToUint256(modulus);
        if (m == 0) {
            return out;
        }

        uint256 rounds = _tailToUint16(exponent);
        if (rounds == 0) {
            rounds = uint256(exponent.length) * 8;
        }

        uint256 acc = 1 % m;
        uint256 baseAcc = b % m;

        for (uint256 i = 0; i < rounds; i++) {
            acc = mulmod(acc, baseAcc, m);
            baseAcc = mulmod(baseAcc, baseAcc, m);
        }

        _writeUint256ToTail(out, acc);
        return out;
    }

    function _tailToUint16(bytes memory data) private pure returns (uint16 v) {
        if (data.length == 0) {
            return 0;
        }
        if (data.length == 1) {
            return uint16(uint8(data[0]));
        }
        uint256 n = data.length;
        return (uint16(uint8(data[n - 2])) << 8) | uint16(uint8(data[n - 1]));
    }

    function _bytesToUint256(bytes memory data) private pure returns (uint256 x) {
        if (data.length == 0) {
            return 0;
        }
        uint256 start = data.length > 32 ? data.length - 32 : 0;
        for (uint256 i = start; i < data.length; i++) {
            x = (x << 8) | uint8(data[i]);
        }
    }

    function _writeUint256ToTail(bytes memory out, uint256 value) private pure {
        uint256 outLen = out.length;
        uint256 limit = outLen < 32 ? outLen : 32;
        for (uint256 i = 0; i < limit; i++) {
            out[outLen - 1 - i] = bytes1(uint8(value));
            value >>= 8;
        }
    }
}
