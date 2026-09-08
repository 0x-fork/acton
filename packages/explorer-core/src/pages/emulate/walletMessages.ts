import {
  Address,
  beginCell,
  internal,
  loadMessageRelaxed,
  loadOutList,
  loadStateInit,
  storeOutList,
  storeMessageRelaxed,
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
export const WALLET_MESSAGE_LIMITS = {v4: 4, v5: 255} as const

/** Selects the wallet's wire format; V4 revisions share the same simple-send encoding. */
export type WalletMessageVersion = keyof typeof WALLET_MESSAGE_LIMITS

/** Validated catalog struct names scope custom fields without replacing unrelated nested ABI fields. */
export interface WalletExternalSchema {
  readonly version: WalletMessageVersion
  readonly signedStruct: string
  readonly messagesStruct: string
  readonly messagesField: "messages" | "outActions"
}

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

/** Enable wallet controls only for recognized external schemas, including their field types and prefixes. */
export function getWalletExternalSchema(
  abi: ContractABI | undefined,
  option: AbiMessageBuilderOption | undefined,
): WalletExternalSchema | undefined {
  if (
    !abi ||
    !["WalletV5r1", "WalletV4r1", "WalletV4r2"].includes(abi.contract_name) ||
    option?.transport !== "external"
  )
    return undefined

  try {
    const symbols = createAbiMessageSymbols(abi)
    let tyIdx = option.valueTyIdx
    let ty = symbols.tyByIdx(tyIdx)
    const visited = new Set<number>()
    while (ty.kind === "AliasRef") {
      const target = symbols.aliasTargetOf(tyIdx)
      if (visited.has(target.ty_idx)) return undefined
      visited.add(target.ty_idx)
      tyIdx = target.ty_idx
      ty = symbols.tyByIdx(tyIdx)
    }

    if (ty.kind !== "StructRef") return undefined
    const declaration = symbols.getStruct(ty.struct_name)
    const fields = symbols.structFieldsOf(tyIdx, false)

    if (abi.contract_name !== "WalletV5r1") {
      if (ty.struct_name !== `${abi.contract_name}SignedExternal` || declaration.prefix)
        return undefined
      const expected = ["signature", "subwalletId", "validUntil", "seqno", "payload"]
      if (
        fields.length !== expected.length ||
        !fields.every((field, index) => {
          if (field.name !== expected[index]) return false
          const fieldTy = symbols.tyByIdx(field.ty_idx)
          if (index === 0) return fieldTy.kind === "bitsN" && fieldTy.n === 512
          if (index < 4) return fieldTy.kind === "uintN" && fieldTy.n === 32
          return fieldTy.kind === "AliasRef" && fieldTy.alias_name === `${abi.contract_name}Payload`
        })
      )
        return undefined

      const payload = symbols.tyByIdx(symbols.aliasTargetOf(fields[4].ty_idx).ty_idx)
      if (payload.kind !== "union") return undefined
      const send = payload.variants.find(
        variant => variant.prefix_num === 0 && variant.prefix_len === 8,
      )
      if (!send) return undefined
      const sendTy = symbols.tyByIdx(send.variant_ty_idx)
      if (sendTy.kind !== "StructRef" || sendTy.struct_name !== `${abi.contract_name}SimpleSend`)
        return undefined
      const sendDeclaration = symbols.getStruct(sendTy.struct_name)
      const sendFields = symbols.structFieldsOf(send.variant_ty_idx, false)
      if (
        sendDeclaration.prefix?.prefix_num !== 0 ||
        sendDeclaration.prefix.prefix_len !== 8 ||
        sendFields.length !== 1 ||
        sendFields[0].name !== "messages" ||
        symbols.tyByIdx(sendFields[0].ty_idx).kind !== "remaining"
      )
        return undefined

      return {
        version: "v4",
        signedStruct: ty.struct_name,
        messagesStruct: sendTy.struct_name,
        messagesField: "messages",
      }
    }

    if (ty.struct_name !== "WalletSignedExternalV5r1") return undefined
    if (declaration.prefix?.prefix_num !== 0x73_69_67_6e || declaration.prefix.prefix_len !== 32)
      return undefined

    const expected = [
      "walletId",
      "validUntil",
      "seqno",
      "outActions",
      "extendedActions",
      "signature",
    ]
    const matches =
      fields.length === expected.length &&
      fields.every((field, index) => {
        if (field.name !== expected[index]) return false
        const fieldTy = symbols.tyByIdx(field.ty_idx)
        if (index < 3) return fieldTy.kind === "uintN" && fieldTy.n === 32
        if (index === 5) return fieldTy.kind === "bitsN" && fieldTy.n === 512
        return fieldTy.kind === "nullable" && symbols.tyByIdx(fieldTy.inner_ty_idx).kind === "cell"
      })
    return matches
      ? {
          version: "v5",
          signedStruct: ty.struct_name,
          messagesStruct: ty.struct_name,
          messagesField: "outActions",
        }
      : undefined
  } catch {
    return undefined
  }
}

/** V4 stores inline mode/ref pairs; V5 stores an ordered C5 OutList behind a nullable reference. */
export function encodeWalletMessages(
  version: WalletMessageVersion,
  drafts: readonly WalletSendDraft[],
): string | null {
  if (drafts.length === 0) return version === "v5" ? null : EMPTY_MESSAGE_BODY
  if (drafts.length > WALLET_MESSAGE_LIMITS[version])
    throw new Error(
      `Wallet ${version.toUpperCase()} supports up to ${WALLET_MESSAGE_LIMITS[version]} messages`,
    )

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

  const builder = beginCell()
  if (version === "v4") {
    for (const action of actions) {
      builder
        .storeUint(action.mode, 8)
        .storeRef(beginCell().store(storeMessageRelaxed(action.outMsg)).endCell())
    }
  } else {
    builder.store(storeOutList(actions))
  }
  return builder.endCell().toBoc().toString("hex")
}

/** Unsupported or noncanonical cells remain in raw mode rather than silently losing actions. */
export function decodeWalletMessages(
  version: WalletMessageVersion,
  value: unknown,
): readonly WalletSendDraft[] {
  if (version === "v5" && (value === null || value === undefined)) return []
  if (typeof value !== "string") throw new Error("Expected an out-action list cell")
  const cell = parseAbiCellArg(value)
  if (cell.bits.length === 0 && cell.refs.length === 0) return []
  const slice = cell.beginParse()
  const actions = version === "v5" ? loadOutList(slice) : []
  if (version === "v4") {
    while (slice.remainingRefs > 0) {
      const mode = slice.loadUint(8)
      const message = slice.loadRef().beginParse()
      actions.push({type: "sendMsg", mode, outMsg: loadMessageRelaxed(message)})
    }
    slice.endParse()
  }
  if (actions.length > WALLET_MESSAGE_LIMITS[version])
    throw new Error(
      `Wallet ${version.toUpperCase()} supports up to ${WALLET_MESSAGE_LIMITS[version]} messages`,
    )

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

  const encoded = encodeWalletMessages(version, drafts)
  if (!encoded || !parseAbiCellArg(encoded).equals(cell)) {
    throw new Error("This action list requires the raw cell editor to preserve its encoding")
  }
  return drafts
}
