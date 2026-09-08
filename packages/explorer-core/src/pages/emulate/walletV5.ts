import {
  Address,
  beginCell,
  internal,
  loadOutList,
  loadStateInit,
  storeOutList,
  storeStateInit,
  type MessageRelaxed,
} from "@ton/core"
import {
  createAbiMessageSymbols,
  parseAbiCellArg,
  type AbiMessageBuilderOption,
  type ContractABI,
} from "@acton/transaction-ui/abi"
import {formatGramAmount, parseGramAmount} from "@acton/ui"

export const EMPTY_MESSAGE_BODY = beginCell().endCell().toBoc().toString("hex")
export const MAX_WALLET_V5_MESSAGES = 255

/** Editable send actions retain the original relaxed header so opening the builder is lossless. */
export interface WalletSendDraft {
  readonly id: string
  readonly address: string
  readonly amount: string
  readonly bounce: boolean
  readonly sendMode: string
  readonly bodyBoc: string
  readonly stateInitBoc: string
  readonly original?: MessageRelaxed
}

/** Only the catalog's V5 external schema gets wallet-specific fields; arbitrary cells stay generic. */
export function isWalletV5ExternalMessage(
  abi: ContractABI | undefined,
  option: AbiMessageBuilderOption | undefined,
): boolean {
  if (abi?.contract_name !== "WalletV5r1" || option?.transport !== "external") return false

  try {
    const symbols = createAbiMessageSymbols(abi)
    let tyIdx = option.valueTyIdx
    let ty = symbols.tyByIdx(tyIdx)
    const visited = new Set<number>()
    while (ty.kind === "AliasRef") {
      const target = symbols.aliasTargetOf(tyIdx)
      if (visited.has(target.ty_idx)) return false
      visited.add(target.ty_idx)
      tyIdx = target.ty_idx
      ty = symbols.tyByIdx(tyIdx)
    }

    if (ty.kind !== "StructRef" || ty.struct_name !== "WalletSignedExternalV5r1") return false
    const declaration = symbols.getStruct(ty.struct_name)
    if (declaration.prefix?.prefix_num !== 0x73_69_67_6e || declaration.prefix.prefix_len !== 32)
      return false

    const fields = symbols.structFieldsOf(tyIdx, false)
    const expected = [
      "walletId",
      "validUntil",
      "seqno",
      "outActions",
      "extendedActions",
      "signature",
    ]
    return (
      fields.length === expected.length &&
      fields.every((field, index) => {
        if (field.name !== expected[index]) return false
        const fieldTy = symbols.tyByIdx(field.ty_idx)
        if (index < 3) return fieldTy.kind === "uintN" && fieldTy.n === 32
        if (index === 5) return fieldTy.kind === "bitsN" && fieldTy.n === 512
        return fieldTy.kind === "nullable" && symbols.tyByIdx(fieldTy.inner_ty_idx).kind === "cell"
      })
    )
  } catch {
    return false
  }
}

/** V5 uses an ordered C5 OutList, not a cell containing an array of full messages. */
export function encodeWalletV5Messages(drafts: readonly WalletSendDraft[]): string | null {
  if (drafts.length === 0) return null
  if (drafts.length > MAX_WALLET_V5_MESSAGES)
    throw new Error("Wallet V5 supports up to 255 messages")

  const actions = drafts.map(draft => {
    const amount = parseGramAmount(draft.amount)
    if (amount === undefined || amount < 0n) throw new Error("Enter a valid message amount")
    if (!/^\d+$/.test(draft.sendMode) || Number(draft.sendMode) > 255) {
      throw new Error("Send mode must be an integer from 0 to 255")
    }

    const body = parseAbiCellArg(draft.bodyBoc)
    const initCell = draft.stateInitBoc.trim() ? parseAbiCellArg(draft.stateInitBoc) : undefined
    const initSlice = initCell?.beginParse()
    const init = initSlice ? loadStateInit(initSlice) : undefined
    initSlice?.endParse()

    const message = internal({
      to: Address.parse(draft.address),
      value: amount,
      bounce: draft.bounce,
      body,
      init,
    })
    // Preserve fields that the compact UI does not edit, including extra currencies and source.
    if (draft.original?.info.type === "internal" && message.info.type === "internal") {
      message.info = {
        ...draft.original.info,
        dest: message.info.dest,
        bounce: draft.bounce,
        value: {...draft.original.info.value, coins: amount},
      }
    }

    return {type: "sendMsg" as const, mode: Number(draft.sendMode), outMsg: message}
  })

  return beginCell().store(storeOutList(actions)).endCell().toBoc().toString("hex")
}

/** Unsupported or noncanonical cells remain in raw mode rather than silently losing actions. */
export function decodeWalletV5Messages(value: unknown): readonly WalletSendDraft[] {
  if (value === null || value === undefined) return []
  if (typeof value !== "string") throw new Error("Expected an out-action list cell")
  const cell = parseAbiCellArg(value)
  if (cell.bits.length === 0 && cell.refs.length === 0) return []
  const actions = loadOutList(cell.beginParse())
  if (actions.length > MAX_WALLET_V5_MESSAGES)
    throw new Error("Wallet V5 supports up to 255 messages")

  const drafts = actions.map((action, index): WalletSendDraft => {
    if (action.type !== "sendMsg" || action.outMsg.info.type !== "internal") {
      throw new Error("This action list requires the raw cell editor")
    }
    const message = action.outMsg
    const info = action.outMsg.info
    return {
      id: `loaded-${index}`,
      address: info.dest.toString(),
      amount: formatGramAmount(info.value.coins, {showUnit: false, maximumFractionDigits: 9}),
      bounce: info.bounce,
      sendMode: String(action.mode),
      bodyBoc: message.body.toBoc().toString("hex"),
      stateInitBoc: message.init
        ? beginCell().store(storeStateInit(message.init)).endCell().toBoc().toString("hex")
        : "",
      original: message,
    }
  })

  const encoded = encodeWalletV5Messages(drafts)
  if (!encoded || !parseAbiCellArg(encoded).equals(cell)) {
    throw new Error("This action list requires the raw cell editor to preserve its encoding")
  }
  return drafts
}
