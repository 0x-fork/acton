import "errors"
import "messages"
import "storage"

contract VerificationRecord {
    author: "Petr Makhnev"
    version: "0.1.0"
    description: "Verification record for one code hash"
    storage: VerificationRecordStorage
    incomingMessages: VerificationRecordMessage
}

type VerificationRecordMessage = AddBundle

fun onInternalMessage(in: InMessage) {
    val msg = lazy VerificationRecordMessage.fromSlice(in.body);

    match (msg) {
        AddBundle => {
            var storage = lazy VerificationRecordStorage.load();
            assert (in.senderAddress == storage.masterAddress) throw Errors.NotMaster;

            if (storage.bundles.addIfNotExists(msg.sourceBundleHash, ())) {
                storage.bundleCount += 1;
                storage.save();
            }
        }
        else => throw Errors.InvalidMessage,
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun masterAddress(): address {
    val storage = lazy VerificationRecordStorage.load();

    return storage.masterAddress;
}

get fun codeHash(): uint256 {
    val storage = lazy VerificationRecordStorage.load();

    return storage.codeHash;
}

get fun hasBundle(sourceBundleHash: uint256): bool {
    val storage = lazy VerificationRecordStorage.load();

    return storage.bundles.exists(sourceBundleHash);
}

get fun bundleCount(): uint32 {
    val storage = lazy VerificationRecordStorage.load();

    return storage.bundleCount;
}

get fun isVerified(): bool {
    val storage = lazy VerificationRecordStorage.load();

    return storage.bundleCount > 0;
}
