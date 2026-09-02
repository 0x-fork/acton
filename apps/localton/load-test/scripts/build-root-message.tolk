import "@acton/boc"
import "@acton/build"
import "@acton/env"
import "@acton/fs"
import "@acton/io"
import "@acton/emulation/network"
import "@contracts/types"

fun main() {
    val rawTreeId = env<int>("TREE_ID") ?? 1;
    if (rawTreeId <= 0 || rawTreeId > 0xFFFFFFFFFFFFFFFF) {
        println("TREE_ID must be between 1 and 18446744073709551615");
        throw 100;
    }
    val treeId = rawTreeId as uint64;
    val state: ContractState = {
        code: build("RecursiveDeployer"),
        data: Storage {
            treeId,
            nodeId: 1,
            expanded: false,
        }.toCell(),
    };
    val address = AutoDeployAddress { stateInit: state }.calculateAddress();
    val message = net.createExternalMessage(
        address,
        Expand {},
        StateInit {
            fixedPrefixLength: null,
            special: null,
            code: state.code,
            data: state.data,
            library: null,
        },
    );

    if (!fs.writeBytes("root-message.boc", boc.encode(message.messageCell, { crc32: true }))) {
        throw 100;
    }

    println("Root address: {}", address);
    println("Tree ID: {}", treeId);
    println("External message: root-message.boc");
}
