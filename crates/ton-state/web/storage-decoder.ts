import {Buffer} from "buffer"
import type {ContractABI} from "@ton/tolk-abi-to-typescript"

// TON's cell runtime reads Buffer while its modules initialize.
globalThis.Buffer = Buffer

const {decodeAbiStorageDataBoc} = await import("../../../packages/transaction-ui/src/lib/abiStorage")
const {stringifyAbiJson} = await import("../../../packages/transaction-ui/src/lib/abiValue")

/** Render storage with the same decoder and lossless JSON conventions as Explorer. */
export function decodeStorage(abi: ContractABI, data: string): string {
  return stringifyAbiJson(data ? decodeAbiStorageDataBoc(abi, data) : null)
}
