export interface PrecompiledContractMetadata {
  readonly description: string
  readonly sourceUrl?: string
  readonly title: string
  readonly verifiedContractUrl?: string
}

const PRECOMPILED_CONTRACT_METADATA: Readonly<Record<string, PrecompiledContractMetadata>> = {
  "0x89468f02c78e570802e39979c8516fc38df07ea76a48357e0536f2ba7b3ee37b": {
    title: "Stablecoin jetton wallet",
    description:
      "An exotic library reference to the jetton wallet from TON's stablecoin contract. Validators can execute this code using a native implementation while recording the fixed gas usage shown here.",
    sourceUrl: "https://github.com/ton-blockchain/stablecoin-contract",
    verifiedContractUrl:
      "https://actonscan.com/verified/8f452d7a4dfd74066b682365177259ed05734435be76b5fd4bd5d8af2b7c3d68",
  },
}

const UNKNOWN_PRECOMPILED_CONTRACT_METADATA: PrecompiledContractMetadata = {
  title: "Precompiled contract",
  description:
    "Validators may execute a native implementation instead of TVM, while the transaction records the fixed gas usage shown here.",
}

export function getPrecompiledContractMetadata(codeHash: string): PrecompiledContractMetadata {
  return (
    PRECOMPILED_CONTRACT_METADATA[codeHash.toLowerCase()] ?? UNKNOWN_PRECOMPILED_CONTRACT_METADATA
  )
}
