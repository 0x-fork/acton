import "@gen/VerificationRecord.code"
import "storage"

const REGISTRY_WORKCHAIN = BASECHAIN

fun verificationRecordState(masterAddress: address, codeHash: uint256): ContractState {
    val emptyRecordStorage: VerificationRecordStorage = { masterAddress, codeHash };

    return { code: verificationRecordCompiledCode(), data: emptyRecordStorage.toCell() };
}

fun verificationRecordDeployAddress(masterAddress: address, codeHash: uint256): AutoDeployAddress {
    return {
        workchain: REGISTRY_WORKCHAIN,
        stateInit: verificationRecordState(masterAddress, codeHash),
    };
}

fun calculateVerificationRecordAddress(masterAddress: address, codeHash: uint256): address {
    val deployed = verificationRecordDeployAddress(masterAddress, codeHash);
    return deployed.calculateAddress();
}
