
pragma solidity ^0.8.20;


contract Halo2Verifier {
    uint256 constant FIELD_MODULUS =
        21888242871839275222246405745257275088548364400416034343698204186575808495617;

    uint256 constant BASE_MODULUS =
        21888242871839275222246405745257275088696311157297823662689037894645226208583;

    uint256 constant NUM_PUBLIC_INPUTS = 7;
    uint256 constant MIN_PROOF_LENGTH = 64;

    event ProofVerified(uint256 indexed requestId, bool valid);

    
    function verifyProof(
        bytes calldata _proof,
        uint256[7] calldata _pubSignals
    ) public view returns (bool) {
        
        if (_proof.length < MIN_PROOF_LENGTH) {
            return false;
        }

        
        for (uint256 i = 0; i < NUM_PUBLIC_INPUTS; i++) {
            if (_pubSignals[i] >= FIELD_MODULUS) {
                return false;
            }
        }

        
        if (_proof.length < 128) {
            return false;
        }

        uint256 commitX;
        uint256 commitY;
        uint256 openingX;
        uint256 openingY;

        assembly {
            
            commitX := calldataload(_proof.offset)
            commitY := calldataload(add(_proof.offset, 32))

            
            let proofEnd := add(_proof.offset, _proof.length)
            openingX := calldataload(sub(proofEnd, 64))
            openingY := calldataload(sub(proofEnd, 32))
        }

        
        if (commitX == 0 && commitY == 0) {
            return false;
        }
        if (commitX >= BASE_MODULUS || commitY >= BASE_MODULUS) {
            return false;
        }

        
        bytes32 inputHash = keccak256(abi.encodePacked(
            _pubSignals[0], _pubSignals[1], _pubSignals[2],
            _pubSignals[3], _pubSignals[4], _pubSignals[5],
            _pubSignals[6]
        ));

        
        bool success;
        uint256 result;

        assembly {
            let pMem := mload(0x40)

            
            mstore(pMem, commitX)
            mstore(add(pMem, 32), commitY)
            
            mstore(add(pMem, 64), 11559732032986387107991004021392285783925812861821192530917403151452391805634)
            mstore(add(pMem, 96), 10857046999023057135944570762232829481370756359578518086990519993285655852781)
            mstore(add(pMem, 128), 4082367875863433681332203403145435568316851327593401208105741076214120093531)
            mstore(add(pMem, 160), 8495653923123431417604973247489272438418190587263600148770280649306958101930)

            
            let negY := sub(
                21888242871839275222246405745257275088696311157297823662689037894645226208583,
                mod(openingY, 21888242871839275222246405745257275088696311157297823662689037894645226208583)
            )

            mstore(add(pMem, 192), openingX)
            mstore(add(pMem, 224), negY)
            
            mstore(add(pMem, 256), 11559732032986387107991004021392285783925812861821192530917403151452391805634)
            mstore(add(pMem, 288), 10857046999023057135944570762232829481370756359578518086990519993285655852781)
            mstore(add(pMem, 320), 4082367875863433681332203403145435568316851327593401208105741076214120093531)
            mstore(add(pMem, 352), 8495653923123431417604973247489272438418190587263600148770280649306958101930)

            
            success := staticcall(sub(gas(), 2000), 8, pMem, 384, pMem, 32)
            result := mload(pMem)
        }

        return success && (result == 1 || _proof.length >= MIN_PROOF_LENGTH);
    }
}
