import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"

import "@contracts/storage"
import "@wrappers/RegistryMaster.gen"
import "@wrappers/VerificationRecord.gen"

const CODE_HASH_E2E: uint256 = 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
const SOURCE_BUNDLE_HASH_E2E: uint256 = 0xfedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210

fun main() {
    val deployer = scripts.wallet("deployer");
    println("Localnet deployer: {}", deployer.address);

    val master = RegistryMaster.fromStorage({
        owner: deployer.address,
    });
    val deployTrace = master.deploy(deployer.address, { value: ton("0.1") }).waitForTrace();
    if (deployTrace == null) {
        Assert.fail("RegistryMaster deployment trace was not found");
    }

    println("RegistryMaster: {}", master.address);

    val registerTrace = master
        .sendRegisterBundle(
            deployer.address,
            CODE_HASH_E2E,
            SOURCE_BUNDLE_HASH_E2E,
            { value: ton("0.5") },
        )
        .waitForTrace();
    if (registerTrace == null) {
        Assert.fail("RegisterBundle trace was not found");
    }

    val recordAddress = master.verificationAddress(CODE_HASH_E2E);
    val record = VerificationRecord.fromAddress(recordAddress);

    Assert.equal(record.masterAddress(), master.address, "record master must be RegistryMaster");
    Assert.equal(record.codeHash(), CODE_HASH_E2E, "record code hash must match input");
    Assert.equal(record.hasBundle(SOURCE_BUNDLE_HASH_E2E), true, "bundle must be registered");
    Assert.equal(record.bundleCount(), 1, "exactly one bundle must be registered");
    Assert.equal(record.isVerified(), true, "record must be verified");

    println("VerificationRecord: {}", record.address);
    println("Localnet e2e passed");
}
