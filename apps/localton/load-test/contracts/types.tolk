struct Storage {
    treeId: uint64
    nodeId: uint64
    expanded: bool
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

/// Trigger carried by the root external message and every child deployment message.
struct (0x15A4E72D) Expand {}
