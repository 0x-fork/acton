struct RegistryMasterStorage {
    owner: address
}

fun RegistryMasterStorage.load(): RegistryMasterStorage {
    return RegistryMasterStorage.fromCell(contract.getData());
}

fun RegistryMasterStorage.save(self) {
    contract.setData(self.toCell());
}

struct VerificationRecordStorage {
    masterAddress: address
    codeHash: uint256
    bundles: map<uint256, ()> = []
    bundleCount: uint32 = 0
}

fun VerificationRecordStorage.load(): VerificationRecordStorage {
    return VerificationRecordStorage.fromCell(contract.getData());
}

fun VerificationRecordStorage.save(self) {
    contract.setData(self.toCell());
}
