import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@contracts/errors"
import "@contracts/messages"
import "@contracts/storage"
import "@wrappers/RegistryMaster.gen"
import "@wrappers/VerificationRecord.gen"

const CODE_HASH_ONE: uint256 = 0x1111111111111111111111111111111111111111111111111111111111111111
const CODE_HASH_TWO: uint256 = 0x2222222222222222222222222222222222222222222222222222222222222222
const BUNDLE_HASH_ONE: uint256 = 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
const BUNDLE_HASH_TWO: uint256 = 0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb

get fun `test master exposes owner`() {
    val (master, owner, _) = setupTest();

    expect(master.owner()).toEqual(owner.address);
}

get fun `test verification address is deterministic`() {
    val (master, _, _) = setupTest();
    val expectedRecord = expectedRecord(master, CODE_HASH_ONE);

    expect(master.verificationAddress(CODE_HASH_ONE)).toEqual(expectedRecord.address);
    expect(master.verificationAddress(CODE_HASH_ONE)).toEqual(expectedRecord.address);
    expect(master.verificationAddress(CODE_HASH_TWO)).toNotEqual(expectedRecord.address);
}

get fun `test non-owner cannot register bundle`() {
    val (master, _, notOwner) = setupTest();
    val expectedRecord = expectedRecord(master, CODE_HASH_ONE);

    val res = master.sendRegisterBundle(
        notOwner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );
    expect(res).toHaveFailedTx<RegisterBundle>({
        from: notOwner.address,
        to: master.address,
        exitCode: Errors.NotOwner,
    });
    expect(testing.isDeployed(expectedRecord.address)).toEqual(false);
}

get fun `test owner registration deploys record and stores bundle`() {
    val (master, owner, _) = setupTest();
    val record = expectedRecord(master, CODE_HASH_ONE);

    val res = master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );
    expect(res).toHaveSuccessfulTx<RegisterBundle>({
        from: owner.address,
        to: master.address,
    });
    expect(res).toHaveSuccessfulDeploy<AddBundle>({
        from: master.address,
        to: record.address,
    });

    expect(record.masterAddress()).toEqual(master.address);
    expect(record.codeHash()).toEqual(CODE_HASH_ONE);
    expect(record.hasBundle(BUNDLE_HASH_ONE)).toEqual(true);
    expect(record.bundleCount()).toEqual(1);
}

get fun `test duplicate bundle registration is idempotent`() {
    val (master, owner, _) = setupTest();
    val record = expectedRecord(master, CODE_HASH_ONE);

    master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );
    val res = master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );
    expect(res).toHaveSuccessfulTx<AddBundle>({
        from: master.address,
        to: record.address,
    });

    expect(record.hasBundle(BUNDLE_HASH_ONE)).toEqual(true);
    expect(record.bundleCount()).toEqual(1);
}

get fun `test multiple bundles can be registered for same code hash`() {
    val (master, owner, _) = setupTest();
    val record = expectedRecord(master, CODE_HASH_ONE);

    master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );
    master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_TWO,
        { value: ton("0.5") },
    );

    expect(record.hasBundle(BUNDLE_HASH_ONE)).toEqual(true);
    expect(record.hasBundle(BUNDLE_HASH_TWO)).toEqual(true);
    expect(record.bundleCount()).toEqual(2);
}

get fun `test non-master cannot add bundle directly`() {
    val (master, owner, notOwner) = setupTest();
    val record = expectedRecord(master, CODE_HASH_ONE);

    master.sendRegisterBundle(
        owner.address,
        CODE_HASH_ONE,
        BUNDLE_HASH_ONE,
        { value: ton("0.5") },
    );

    val res = record.sendAddBundle(
        notOwner.address,
        BUNDLE_HASH_TWO,
        { value: ton("0.2") },
    );
    expect(res).toHaveFailedTx<AddBundle>({
        from: notOwner.address,
        to: record.address,
        exitCode: Errors.NotMaster,
    });
    expect(record.hasBundle(BUNDLE_HASH_TWO)).toEqual(false);
    expect(record.bundleCount()).toEqual(1);
}

get fun `test direct empty record deploy does not verify code hash`() {
    val (master, _, notOwner) = setupTest();
    val record = expectedRecord(master, CODE_HASH_TWO);

    val res = record.deploy(notOwner.address, { value: ton("0.2") });
    expect(res).toHaveFailedTx({
        from: notOwner.address,
        to: record.address,
        exitCode: Errors.InvalidMessage,
    });
    expect(testing.isDeployed(record.address)).toEqual(true);
    expect(record.isVerified()).toEqual(false);
    expect(record.bundleCount()).toEqual(0);
    expect(record.hasBundle(BUNDLE_HASH_ONE)).toEqual(false);
}

fun setupTest(): (RegistryMaster, Treasury, Treasury) {
    val owner = testing.treasury("owner");
    val notOwner = testing.treasury("notOwner");

    val master = RegistryMaster.fromStorage({
        owner: owner.address,
    });
    val res = master.deploy(owner.address, { value: ton("1") });
    expect(res).toHaveSuccessfulDeploy({
        to: master.address,
    });

    return (master, owner, notOwner);
}

fun expectedRecord(master: RegistryMaster, codeHash: uint256): VerificationRecord {
    return VerificationRecord.fromStorage({
        masterAddress: master.address,
        codeHash,
    });
}
