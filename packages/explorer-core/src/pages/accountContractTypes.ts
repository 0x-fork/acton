import type {AccountStateTokenInfo} from "../api/types"

const ACCOUNT_CONTRACT_HINTS = {
  jetton_master: {
    interfaces: ["jetton_master"],
    tokenInfoType: "jetton_masters",
  },
  jetton_wallet: {
    interfaces: ["jetton_wallet"],
    tokenInfoType: "jetton_wallets",
  },
  nft_collection: {
    interfaces: ["nft_collection"],
    tokenInfoType: "nft_collections",
  },
  nft_item: {
    interfaces: ["nft_item", "nft_item_simple"],
    tokenInfoType: "nft_items",
  },
} as const

type AccountContractType = keyof typeof ACCOUNT_CONTRACT_HINTS

export function hasAccountInterface(interfaces: readonly string[], expected: string): boolean {
  return interfaces.some(iface => iface.trim().toLowerCase() === expected)
}

function hasTokenInfoType(tokenInfo: readonly AccountStateTokenInfo[], expected: string): boolean {
  return tokenInfo.some(info => info.type === expected)
}

export function hasAccountContractHint(
  interfaces: readonly string[],
  tokenInfo: readonly AccountStateTokenInfo[],
  expected: AccountContractType,
): boolean {
  const hint = ACCOUNT_CONTRACT_HINTS[expected]
  const hasInterface = hint.interfaces.some(expectedInterface =>
    hasAccountInterface(interfaces, expectedInterface),
  )

  return hasInterface || hasTokenInfoType(tokenInfo, hint.tokenInfoType)
}
