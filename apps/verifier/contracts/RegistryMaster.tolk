import "errors"
import "messages"
import "storage"
import "verification-record-utils"

contract RegistryMaster {
    author: "Petr Makhnev"
    version: "0.1.0"
    description: "Owner-managed registry root for verified code hashes"
    storage: RegistryMasterStorage
    incomingMessages: RegistryMasterMessage
}

type RegistryMasterMessage = RegisterBundle

fun onInternalMessage(in: InMessage) {
    val msg = lazy RegistryMasterMessage.fromSlice(in.body);

    match (msg) {
        RegisterBundle => {
            val storage = lazy RegistryMasterStorage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;

            val recordDestination = verificationRecordDeployAddress(
                contract.getAddress(),
                msg.codeHash,
            );
            val addBundleMessage = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: recordDestination,
                value: 0,
                body: AddBundle { sourceBundleHash: msg.sourceBundleHash },
            });
            addBundleMessage.send(
                SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE | SEND_MODE_BOUNCE_ON_ACTION_FAIL,
            );
        }
        else => {
            assert (in.body.isEmpty()) throw Errors.InvalidMessage;
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun owner(): address {
    val storage = lazy RegistryMasterStorage.load();

    return storage.owner;
}

get fun verificationAddress(codeHash: uint256): address {
    return calculateVerificationRecordAddress(contract.getAddress(), codeHash);
}
