import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/big_array"
import "@contracts/types"

struct RecursiveNode {
    address: address
    state: ContractState
}

fun RecursiveNode.fromStorage(storage: Storage): RecursiveNode {
    val state: ContractState = { code: build("RecursiveDeployer"), data: storage.toCell() };
    val address = AutoDeployAddress { stateInit: state }.calculateAddress();
    return { address, state };
}

fun RecursiveNode.externalExpand(self): ExternalInMessage {
    return net.createExternalMessage(
        self.address,
        Expand {},
        StateInit {
            fixedPrefixLength: null,
            special: null,
            code: self.state.code,
            data: self.state.data,
            library: null,
        },
    );
}

fun RecursiveNode.info(self): Storage {
    return net.runGetMethod(self.address, "nodeInfo");
}

get fun `test external deployment creates two leaf contracts`() {
    val root = RecursiveNode.fromStorage({ treeId: 1, nodeId: 1, expanded: false });
    testing.topUp(root.address, grams("0.15"));

    val result = net.sendExternal(root.externalExpand());
    expect(result).toBeAccepted();
    val trace = result.unwrap();

    expect(trace.size()).toEqual(3);
    expect(trace).toHaveAllSuccessfulTxs();
    expect(root.info().expanded).toEqual(true);

    val left = RecursiveNode.fromStorage({ treeId: 1, nodeId: 2, expanded: false });
    val right = RecursiveNode.fromStorage({ treeId: 1, nodeId: 3, expanded: false });
    expect(testing.getAccountState(left.address)).toBeNotNull();
    expect(testing.getAccountState(right.address)).toBeNotNull();
    expect(left.info().expanded).toEqual(true);
    expect(right.info().expanded).toEqual(true);
}

get fun `test balance threshold terminates recursion at root`() {
    val root = RecursiveNode.fromStorage({ treeId: 1, nodeId: 1, expanded: false });
    testing.topUp(root.address, grams("0.08"));

    val result = net.sendExternal(root.externalExpand());
    expect(result).toBeAccepted();
    val trace = result.unwrap();

    expect(trace.size()).toEqual(1);
    expect(trace).toHaveAllSuccessfulTxs();
    expect(root.info().expanded).toEqual(true);
}

get fun `test children recursively create deeper generations`() {
    val root = RecursiveNode.fromStorage({ treeId: 1, nodeId: 1, expanded: false });
    testing.topUp(root.address, grams("0.5"));

    val result = net.sendExternal(root.externalExpand());
    expect(result).toBeAccepted();
    val trace = result.unwrap();

    expect(trace.size()).toEqual(15);
    expect(trace).toHaveAllSuccessfulTxs();

    val deepestLeft = RecursiveNode.fromStorage({ treeId: 1, nodeId: 8, expanded: false });
    val deepestRight = RecursiveNode.fromStorage({ treeId: 1, nodeId: 15, expanded: false });
    expect(testing.getAccountState(deepestLeft.address)).toBeNotNull();
    expect(testing.getAccountState(deepestRight.address)).toBeNotNull();
    expect(deepestLeft.info().expanded).toEqual(true);
    expect(deepestRight.info().expanded).toEqual(true);
}

get fun `test duplicate external trigger is not accepted`() {
    val root = RecursiveNode.fromStorage({ treeId: 1, nodeId: 1, expanded: false });
    testing.topUp(root.address, grams("0.15"));

    val first = net.sendExternal(root.externalExpand());
    expect(first).toBeAccepted();

    val duplicate = net.sendExternal(root.externalExpand());
    expect(duplicate).toBeNotAccepted();
}
