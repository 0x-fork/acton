import "@acton/build"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@contracts/types"

const SPLIT_OVERHEAD = grams("0.03")
const MINIMUM_SPLIT_BALANCE = grams("0.09")
const MAX_TREE_DEPTH = 63

/// Estimates the upper bound of recursive messages before forwarding fees are deducted.
fun estimateMessages(initialBalance: coins): int {
    var balance = initialBalance;
    var messages = 1;
    var nodesAtDepth = 1;
    var depth = 0;

    while (balance > MINIMUM_SPLIT_BALANCE && depth < MAX_TREE_DEPTH) {
        balance = (balance - SPLIT_OVERHEAD) / 2;
        nodesAtDepth = nodesAtDepth * 2;
        messages = messages + nodesAtDepth;
        depth = depth + 1;
    }

    return messages;
}

/// Funds a fresh deterministic root and starts its recursive deployment tree.
fun main() {
    val amountGram = promptInt("Load amount in GRAM:", "10", "10");
    val rawTreeId = promptInt("Fresh tree ID:", "1", "1");
    if (amountGram <= 0) {
        Assert.fail("Amount must be greater than zero");
    }
    if (rawTreeId <= 0 || rawTreeId > 0xFFFFFFFFFFFFFFFF) {
        Assert.fail("Tree ID must be between 1 and 18446744073709551615");
    }

    val treeId = rawTreeId as uint64;
    val deployer = scripts.wallet(promptWallet("Select a funding wallet:"));
    val amount = amountGram * grams("1");
    val state: ContractState = {
        code: build("RecursiveDeployer"),
        data: Storage {
            treeId,
            nodeId: 1,
            expanded: false,
        }.toCell(),
    };
    val rootAddress = AutoDeployAddress { stateInit: state }.calculateAddress();

    if (scripts.fetchAccountState(rootAddress) != null) {
        Assert.fail("This tree ID already has an account; choose a fresh tree ID");
    }

    println("Recursive load");
    println("Wallet: {}", deployer.address);
    println("Root: {}", rootAddress);
    println("Amount: {:gram}", amount);
    println("Tree ID: {}", treeId);
    println("Estimated messages: ~{}", estimateMessages(amount));

    val fundingMessage = createMessage({
        bounce: BounceMode.NoBounce,
        value: amount,
        dest: rootAddress,
    });
    if (net.send(deployer.address, fundingMessage).waitForTrace() == null) {
        Assert.fail("Root funding was not confirmed");
    }
    println("Root funded");

    val trigger = net.createExternalMessage(
        rootAddress,
        Expand {},
        StateInit {
            fixedPrefixLength: null,
            special: null,
            code: state.code,
            data: state.data,
            library: null,
        },
    );
    if (net.sendExternal(trigger).waitForFirstTransaction() == null) {
        Assert.fail("Root activation was not confirmed");
    }

    println("Recursive load started");
}
