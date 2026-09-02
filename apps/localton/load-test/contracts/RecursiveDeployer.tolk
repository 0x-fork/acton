import "@stdlib/gas-payments"
import "types"

contract RecursiveDeployer {
    author: "Petr Makhnev"
    version: "1.0.0"
    description: "Deploys a binary tree of copies of itself until the balance is exhausted"
    storage: Storage
    incomingMessages: Expand
    incomingExternal: Expand
}

/// Balance kept out of child values to pay for the current transaction and leave the parent alive.
const SPLIT_OVERHEAD = grams("0.03")

/// A child below this balance becomes a leaf and does not create more contracts.
const MIN_CHILD_BALANCE = grams("0.03")

/// Largest identifier that can still produce two distinct `uint64` child identifiers.
const MAX_PARENT_NODE_ID = 0x7FFFFFFFFFFFFFFF

/// Expands this node exactly once.
///
/// The stored `expanded` bit makes repeated internal or external delivery idempotent. Each child
/// receives half of the balance left after `SPLIT_OVERHEAD`; regular send mode deducts forwarding
/// fees from the child values. The fixed threshold and halving guarantee finite recursion.
fun expandOnce() {
    var storage = lazy Storage.load();
    if (storage.expanded) {
        return;
    }

    storage.expanded = true;
    storage.save();

    val balance = contract.getOriginalBalance();
    val minimumSplitBalance = SPLIT_OVERHEAD + MIN_CHILD_BALANCE * 2;
    if (balance <= minimumSplitBalance || storage.nodeId > MAX_PARENT_NODE_ID) {
        return;
    }

    val childValue = (balance - SPLIT_OVERHEAD) / 2;
    val code = contract.getCode();
    deployChild(code, storage.treeId, storage.nodeId * 2, childValue);
    deployChild(code, storage.treeId, storage.nodeId * 2 + 1, childValue);
}

/// Enqueues deployment of one child with the same code and a fresh node identifier.
fun deployChild(code: cell, treeId: uint64, nodeId: uint64, value: coins) {
    val stateInit: ContractState = {
        code,
        data: Storage {
            treeId,
            nodeId,
            expanded: false,
        }.toCell(),
    };
    val deploy = createMessage({
        bounce: BounceMode.NoBounce,
        value,
        dest: { stateInit },
        body: Expand {},
    });
    deploy.send(SEND_MODE_REGULAR);
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy Expand.fromSlice(in.body);
    match (msg) {
        Expand => expandOnce(),
    }
}

/// Accepts the one unsigned external trigger used to start a pre-funded root node.
///
/// This is intentionally suitable only for an isolated load-test network. Duplicate delivery is
/// harmless because `expandOnce` persists its idempotency bit before creating any children.
fun onExternalMessage(body: slice) {
    val msg = lazy Expand.fromSlice(body);
    match (msg) {
        Expand => {
            val storage = lazy Storage.load();
            if (storage.expanded) {
                return;
            }
            acceptExternalMessage();
            expandOnce();
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun nodeInfo(): Storage {
    return Storage.load();
}
