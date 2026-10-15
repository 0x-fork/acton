import type {ContractABI} from "@ton/tolk-abi-to-typescript"
import {humanizeIdentifier} from "@acton/ui"

export function getContractTypeLabels(
  compilerAbi?: ContractABI,
  interfaces?: readonly string[],
): string[] {
  const contractName = compilerAbi?.contract_name?.trim()
  const abiContractName = contractName === "WalletTg" ? "Telegram Wallet" : contractName
  const interfaceLabels = (interfaces ?? [])
    .filter(value => !(contractName === "DnsDomainItem" && value.trim().toLowerCase() === "dns"))
    .map(value => getInterfaceLabel(value))
    .filter((value): value is string => value !== undefined)

  const labels = abiContractName ? [abiContractName, ...interfaceLabels] : interfaceLabels
  const seen = new Set<string>()
  const uniqueLabels = labels.filter(label => {
    const key = contractTypeLabelKey(label)
    if (seen.has(key)) {
      return false
    }
    seen.add(key)
    return true
  })
  return uniqueLabels.length > 0 ? uniqueLabels : ["Unknown"]
}

function contractTypeLabelKey(value: string): string {
  const key = value
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "")
    .replace(/interface$/, "")

  return key === "nftv1item" ? "nftitem" : key
}

function getInterfaceLabel(value: string): string | undefined {
  const normalizedInterface = value.trim().toLowerCase()
  if (!normalizedInterface) {
    return undefined
  }

  switch (normalizedInterface) {
    case "jetton_master": {
      return "Jetton master interface"
    }
    case "jetton_wallet": {
      return "Jetton wallet interface"
    }
    case "nft_item":
    case "nft_item_simple": {
      return "NFT item interface"
    }
    case "nft_collection": {
      return "NFT collection interface"
    }
    case "multisig_v2": {
      return "Multisig wallet v2"
    }
    case "multisig_order_v2": {
      return "Multisig order v2"
    }
    case "tg_wallet":
    case "tg wallet": {
      return "Telegram Wallet"
    }
    default: {
      return humanizeIdentifier(normalizedInterface)
    }
  }
}
