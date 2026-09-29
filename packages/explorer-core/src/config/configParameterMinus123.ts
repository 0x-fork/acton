import type {Cell} from "@ton/core"

export interface TelegramWalletContractBytecode {
  readonly bytecodeHash: string
  readonly repositoryUrl?: string
  readonly revision?: string
}

const TELEGRAM_WALLET_CONTRACT_REPOSITORY_URL =
  "https://github.com/ton-blockchain/tg-wallet-contract"

const TELEGRAM_WALLET_CONTRACT_BYTECODE_REVISIONS: ReadonlyMap<string, string> = new Map([
  // Rev00_Initial bytecode proposed for config[-123]:
  // https://github.com/ton-blockchain/tg-wallet-contract/blob/rev00/contracts/WalletTg/revisions.tolk
  ["e30911420bef1191c09dce58b9df2b4ca4c2d9c383cc3b6a91170349ffa70e2c", "00"],
])

export function parseTelegramWalletContractBytecode(cell: Cell): TelegramWalletContractBytecode {
  const bytecodeHash = cell.hash().toString("hex")
  const revision = TELEGRAM_WALLET_CONTRACT_BYTECODE_REVISIONS.get(bytecodeHash)

  if (revision === undefined) {
    return {bytecodeHash}
  }

  return {
    bytecodeHash,
    repositoryUrl: `${TELEGRAM_WALLET_CONTRACT_REPOSITORY_URL}/tree/rev${revision}`,
    revision,
  }
}
