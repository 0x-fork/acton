import {describe, expect, test} from "bun:test"
import {
  Address,
  beginCell,
  Cell,
  Dictionary,
  loadMessage,
  loadMessageRelaxed,
  storeStateInit,
} from "@ton/core"
import {
  buildAbiMessageBoc,
  buildAbiMessageBody,
  decodeAbiStorageDataBoc,
  listAbiMessageBuilderOptions,
  type ContractABI,
} from "@acton/transaction-ui/abi"

import bundledAbiCatalog from "../../../crates/acton-abi-catalog/data/data-abis.json"
import {
  decodeWalletMessages,
  EMPTY_MESSAGE_BODY,
  encodeWalletMessages,
  getWalletExternalSchema,
  type WalletSendDraft,
} from "../src/pages/emulate/walletMessages"

const address = new Address(0, Buffer.alloc(32, 17)).toString()
const recipient = new Address(0, Buffer.alloc(32, 34)).toString()
const draft: WalletSendDraft = {
  id: "first",
  address: recipient,
  amount: "0.123456789",
  bounce: true,
  sendMode: "3",
  bodyBoc: EMPTY_MESSAGE_BODY,
  stateInitBoc: "",
}

describe("Wallet V4 simulator builder", () => {
  for (const revision of ["WalletV4r1", "WalletV4r2"]) {
    const record = bundledAbiCatalog.contracts.find(
      contract => contract.compilerAbi.contract_name === revision,
    )
    if (!record) throw new Error(`Missing ${revision} ABI fixture`)
    const abi = record.compilerAbi as ContractABI
    const option = listAbiMessageBuilderOptions(abi, "external")[0]

    test(`${revision} encodes the complete signed request with four ordered messages`, () => {
      const internalOption = listAbiMessageBuilderOptions(abi, "internal").find(
        entry => entry.label === `${revision}PluginDestruct`,
      )
      if (!internalOption) throw new Error("Missing plugin destruct message fixture")
      const body = buildAbiMessageBody({abi, option: internalOption, argsJson: '{"queryId":"427"}'})
      const stateInitBoc = beginCell()
        .store(storeStateInit({code: beginCell().storeUint(7, 8).endCell()}))
        .endCell()
        .toBoc()
        .toString("hex")
      const drafts = [0, 1, 3, 128].map((mode, index) => ({
        ...draft,
        id: String(index),
        sendMode: String(mode),
        amount: String(index + 1),
        bounce: index % 2 === 0,
        bodyBoc: index === 0 ? body.toBoc().toString("hex") : EMPTY_MESSAGE_BODY,
        stateInitBoc: index === 1 ? stateInitBoc : "",
      }))
      const messages = encodeWalletMessages("v4", drafts)
      const args = {
        ...JSON.parse(option.sampleJson),
        subwalletId: "698983191",
        validUntil: "1788865000",
        seqno: "41",
        payload: {$: `${revision}SimpleSend`, messages},
      }
      const boc = buildAbiMessageBoc({
        abi,
        option,
        destination: address,
        argsJson: JSON.stringify(args),
      })
      const request = loadMessage(Cell.fromHex(boc).beginParse())
      const payload = request.body.beginParse()
      const header = {
        signature: payload.loadUintBig(512).toString(),
        subwalletId: payload.loadUint(32),
        validUntil: payload.loadUint(32),
        seqno: payload.loadUint(32),
        opcode: payload.loadUint(8),
      }
      const outgoing = Array.from({length: payload.remainingRefs}, () => {
        const mode = payload.loadUint(8)
        const message = loadMessageRelaxed(payload.loadRef().beginParse())
        if (message.info.type !== "internal") throw new Error("Expected internal message")
        return {
          mode,
          to: message.info.dest.toString(),
          value: message.info.value.coins.toString(),
          bounce: message.info.bounce,
          stateInit: Boolean(message.init),
          body: message.body.equals(body) ? "ABI" : "empty",
        }
      })
      payload.endParse()
      expect({
        schema: getWalletExternalSchema(abi, option),
        transport: request.info.type,
        header,
        outgoing,
        roundTrip: encodeWalletMessages("v4", decodeWalletMessages("v4", messages)) === messages,
        reversedModes: decodeWalletMessages(
          "v4",
          encodeWalletMessages("v4", [...drafts].reverse()),
        ).map(message => message.sendMode),
      }).toMatchSnapshot()
    })

    test(`${revision} reads wallet parameters through the shared bits264 dictionary fallback`, () => {
      const pluginKey = beginCell().storeInt(-1, 8).storeUint(42, 256).endCell().bits
      const plugins = Dictionary.empty(Dictionary.Keys.BitString(264), {
        serialize() {},
        parse() {
          return undefined
        },
      })
      plugins.set(pluginKey, undefined)
      const states = [undefined, plugins].map(dictionary => {
        const cell = beginCell()
          .storeUint(41, 32)
          .storeUint(698_983_191, 32)
          .storeUint(5, 256)
          .storeDict(dictionary)
          .endCell()
        const storage = decodeAbiStorageDataBoc(abi, cell.toBoc().toString("hex")) as {
          seqno: bigint
          subwalletId: bigint
          plugins: typeof plugins
        }
        return {
          seqno: String(storage.seqno),
          walletId: String(storage.subwalletId),
          plugins: storage.plugins.keys().map(key => key.toString()),
        }
      })
      expect(states).toMatchSnapshot()
    })

    test(`${revision} leaves unrelated or incompatible ABI schemas generic`, () => {
      const changed = structuredClone(abi)
      const send = changed.declarations.find(
        declaration =>
          declaration.kind === "struct" && declaration.name === `${revision}SimpleSend`,
      )
      if (send?.kind !== "struct") throw new Error("Missing simple send fixture")
      send.fields[0].ty_idx = 3
      expect({
        internal:
          getWalletExternalSchema(abi, listAbiMessageBuilderOptions(abi, "internal")[0]) ?? null,
        renamed: getWalletExternalSchema({...abi, contract_name: "CustomWallet"}, option) ?? null,
        wrongMessagesType: getWalletExternalSchema(changed, option) ?? null,
      }).toMatchSnapshot()
    })
  }

  test("empty lists stay inline and malformed or overflowing lists cannot enter the builder", () => {
    const oneMessage = encodeWalletMessages("v4", [draft])
    if (!oneMessage) throw new Error("Missing outgoing message fixture")
    const cell = Cell.fromHex(oneMessage)
    const trailingBits = beginCell().storeSlice(cell.beginParse()).storeBit(true).endCell()
    const missingMode = beginCell().storeRef(cell.refs[0]).endCell()
    const cases = [
      () =>
        encodeWalletMessages(
          "v4",
          Array.from({length: 5}, () => draft),
        ),
      () => encodeWalletMessages("v4", [{...draft, sendMode: "256"}]),
      () => decodeWalletMessages("v4", trailingBits.toBoc().toString("hex")),
      () => decodeWalletMessages("v4", missingMode.toBoc().toString("hex")),
      () => decodeWalletMessages("v4", null),
    ]
    expect({
      empty: encodeWalletMessages("v4", []) === EMPTY_MESSAGE_BODY,
      decodedEmpty: decodeWalletMessages("v4", EMPTY_MESSAGE_BODY),
      rejected: cases.map(run => {
        try {
          run()
          return false
        } catch {
          return true
        }
      }),
    }).toMatchSnapshot()
  })
})
