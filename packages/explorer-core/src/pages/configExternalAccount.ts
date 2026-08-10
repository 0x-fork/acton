import type {BridgeExternalChain} from "../api/config"

export interface ExternalAccountExplorerLink {
  readonly href: string
  readonly name: string
}

interface ExternalAccountExplorer {
  readonly name: string
  readonly mainnetBaseUrl: string
  readonly testnetBaseUrl?: string
}

const EXTERNAL_ACCOUNT_EXPLORERS = {
  BSC: {
    name: "BscScan",
    mainnetBaseUrl: "https://bscscan.com",
    testnetBaseUrl: "https://testnet.bscscan.com",
  },
  Ethereum: {name: "Etherscan", mainnetBaseUrl: "https://etherscan.io"},
  Polygon: {name: "PolygonScan", mainnetBaseUrl: "https://polygonscan.com"},
} as const satisfies Record<BridgeExternalChain, ExternalAccountExplorer>

export function getExternalAccountExplorerLink(
  externalChain: BridgeExternalChain,
  isTestnet: boolean,
  address: string,
): ExternalAccountExplorerLink | undefined {
  const explorer = EXTERNAL_ACCOUNT_EXPLORERS[externalChain]
  const baseUrl = isTestnet
    ? "testnetBaseUrl" in explorer
      ? explorer.testnetBaseUrl
      : undefined
    : explorer.mainnetBaseUrl
  if (baseUrl === undefined) return undefined

  return {
    href: `${baseUrl}/address/${encodeURIComponent(address)}`,
    name: explorer.name,
  }
}
