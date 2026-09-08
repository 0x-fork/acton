import {describe, expect, test} from "bun:test"
import {
  Address,
  beginCell,
  Cell,
  Dictionary,
  internal,
  loadMessage,
  loadOutList,
  storeOutList,
  storeStateInit,
} from "@ton/core"
import {
  buildAbiMessageBoc,
  buildAbiMessageBody,
  decodeAbiMessageBuilderDraft,
  listAbiMessageBuilderOptions,
  parseAbiJson,
  type ContractABI,
} from "@acton/transaction-ui/abi"

import bundledAbiCatalog from "../../../crates/acton-abi-catalog/data/data-abis.json"
import {
  decodeWalletV5Messages,
  EMPTY_MESSAGE_BODY,
  encodeWalletV5Messages,
  isWalletV5ExternalMessage,
  type WalletSendDraft,
} from "../src/pages/emulate/walletV5"

const walletRecord = bundledAbiCatalog.contracts.find(
  contract => contract.id === "wallets.WalletV5r1",
)
const v4Record = bundledAbiCatalog.contracts.find(
  contract => contract.id === "wallets/w4r2.WalletV4r2",
)
if (!walletRecord || !v4Record) throw new Error("Wallet ABI fixtures are missing from the catalog")
const walletAbi = walletRecord.compilerAbi as ContractABI
const v4Abi = v4Record.compilerAbi as ContractABI
const address = new Address(0, Buffer.alloc(32, 17)).toString()
const otherAddress = new Address(0, Buffer.alloc(32, 34)).toString()
const signature = beginCell().storeUint(0, 512).endCell().toBoc().toString("hex")

const draft: WalletSendDraft = {
  id: "first",
  address,
  amount: "0.123456789",
  bounce: true,
  sendMode: "3",
  bodyBoc: EMPTY_MESSAGE_BODY,
  stateInitBoc: "",
}

describe("Wallet V5 simulator builder", () => {
  test("recognizes the complete V5 external schema, leaving other wallets and similarly named fields alone", () => {
    const external = listAbiMessageBuilderOptions(walletAbi, "external")[0]
    const changed = structuredClone(walletAbi)
    const declaration = changed.declarations.find(
      entry => entry.kind === "struct" && entry.name === "WalletSignedExternalV5r1",
    )
    if (declaration?.kind !== "struct") throw new Error("Expected the V5 signed request struct")
    declaration.fields[0].ty_idx = 7

    expect({
      external: isWalletV5ExternalMessage(walletAbi, external),
      internal: isWalletV5ExternalMessage(
        walletAbi,
        listAbiMessageBuilderOptions(walletAbi, "internal")[0],
      ),
      v4: isWalletV5ExternalMessage(v4Abi, listAbiMessageBuilderOptions(v4Abi, "external")[0]),
      wrongType: isWalletV5ExternalMessage(changed, external),
      renamed: isWalletV5ExternalMessage({...walletAbi, contract_name: "CustomWallet"}, external),
    }).toMatchSnapshot()
  })

  test("serializes an external V5 request with ordered messages, ABI payload, StateInit and a zero signature", () => {
    const nested = listAbiMessageBuilderOptions(walletAbi, "internal").find(
      option => option.label === "WalletExtensionActionV5r1",
    )
    if (!nested) throw new Error("Expected the V5 extension message")
    const body = buildAbiMessageBody({
      abi: walletAbi,
      option: nested,
      argsJson: '{"queryId":"427","outActions":null,"extendedActions":null}',
    })
    const init = beginCell()
      .store(
        storeStateInit({
          code: beginCell().storeUint(3, 8).endCell(),
          data: beginCell().storeUint(19, 16).endCell(),
        }),
      )
      .endCell()
    const drafts = [
      {
        ...draft,
        bodyBoc: beginCell()
          .storeUint(0, 32)
          .storeStringTail("Simulator test")
          .endCell()
          .toBoc()
          .toString("hex"),
      },
      {
        ...draft,
        id: "second",
        address: otherAddress,
        amount: "2.5",
        bounce: false,
        bodyBoc: body.toBoc().toString("hex"),
        stateInitBoc: init.toBoc().toString("base64"),
      },
    ]
    const outActions = encodeWalletV5Messages(drafts)
    if (!outActions) throw new Error("Expected nonempty wallet actions")
    const option = listAbiMessageBuilderOptions(walletAbi, "external")[0]
    const boc = buildAbiMessageBoc({
      abi: walletAbi,
      option,
      destination: address,
      argsJson: JSON.stringify({
        walletId: "2147483409",
        validUntil: "1788858300",
        seqno: "427",
        outActions,
        extendedActions: null,
        signature,
      }),
    })
    const message = loadMessage(Cell.fromHex(boc).beginParse())
    const payload = message.body.beginParse()
    const header = {
      opcode: payload.loadUint(32).toString(16),
      walletId: payload.loadUint(32),
      validUntil: payload.loadUint(32),
      seqno: payload.loadUint(32),
    }
    const actionsCell = payload.loadMaybeRef()
    if (!actionsCell) throw new Error("Expected outgoing actions in the signed request")
    const actions = loadOutList(actionsCell.beginParse())
    const extended = payload.loadBit()
    const signatureBits = payload.loadUintBig(512).toString()
    payload.endParse()

    const decodedBody = decodeAbiMessageBuilderDraft(walletAbi, "internal", body)
    if (!decodedBody) throw new Error("Expected the nested ABI payload to decode")

    expect({
      transport: message.info.type,
      header,
      extended,
      signatureBits,
      actions: actions.map(action => {
        if (action.type !== "sendMsg" || action.outMsg.info.type !== "internal")
          throw new Error("Expected an internal send action")
        return {
          mode: action.mode,
          to: action.outMsg.info.dest.toString(),
          value: action.outMsg.info.value.coins.toString(),
          bounce: action.outMsg.info.bounce,
          stateInit: Boolean(action.outMsg.init),
        }
      }),
      decodedPayload: parseAbiJson(decodedBody.argsJson),
      roundTrip: encodeWalletV5Messages(decodeWalletV5Messages(outActions)) === outActions,
      reversed: decodeWalletV5Messages(encodeWalletV5Messages([...drafts].reverse())).map(
        item => item.address,
      ),
    }).toMatchSnapshot()
  })

  test("keeps relaxed headers and extra currencies when opening an existing list", () => {
    const extra = Dictionary.empty(Dictionary.Keys.Uint(32), Dictionary.Values.BigVarUint(5))
    extra.set(17, 12345n)
    const message = internal({to: address, value: 9876543210n})
    if (message.info.type !== "internal") throw new Error("Expected internal message")
    message.info.src = Address.parse(otherAddress)
    message.info.createdLt = 451n
    message.info.value.other = extra
    const cell = beginCell()
      .store(storeOutList([{type: "sendMsg", mode: 3, outMsg: message}]))
      .endCell()
    const drafts = decodeWalletV5Messages(cell.toBoc().toString("base64"))
    const encoded = encodeWalletV5Messages(drafts)
    if (!encoded) throw new Error("Expected nonempty wallet actions")
    const rebuilt = Cell.fromHex(encoded)

    expect({
      identical: rebuilt.equals(cell),
      amount: drafts[0].amount,
      empty: decodeWalletV5Messages(EMPTY_MESSAGE_BODY),
      absent: decodeWalletV5Messages(null),
    }).toMatchSnapshot()
  })

  test("rejects incomplete drafts and preserves unsupported raw actions", () => {
    const reserve = beginCell()
      .store(storeOutList([{type: "reserve", mode: 0, currency: {coins: 1n}}]))
      .endCell()
    const cases = [
      () => encodeWalletV5Messages([{...draft, address: "invalid"}]),
      () => encodeWalletV5Messages([{...draft, amount: "-1"}]),
      () => encodeWalletV5Messages([{...draft, bodyBoc: ""}]),
      () => encodeWalletV5Messages([{...draft, sendMode: "256"}]),
      () => encodeWalletV5Messages(Array.from({length: 256}, () => draft)),
      () => decodeWalletV5Messages(reserve.toBoc().toString("hex")),
    ]
    expect(
      cases.map(run => {
        try {
          run()
          return false
        } catch {
          return true
        }
      }),
    ).toMatchSnapshot()
    expect(
      decodeWalletV5Messages(encodeWalletV5Messages(Array.from({length: 255}, () => draft))),
    ).toHaveLength(255)
  })
})
