import {Address} from "@ton/core"

import type {NftItem} from "../api/types"
import collections from "./ecosystemCollections.json"

// Address allowlist, reviewed on 2026-09-30. Names, images and copied contract code
// cannot establish membership in an ecosystem collection.
// DNS zones and Fragment: https://github.com/mytonwallet-org/mytonwallet/blob/master/src/config.ts
// Telegram Gifts: https://api.mywallet.io/known-addresses (tonNftSuperCollections)
// Testnet .ton: resolver stored by the config-4 root at
// -1:efe71d13860afaa6aeaeaf636f9168487f80f1031b0bf8d939ae49d3ea7f7da0
const addressesByNetwork = new Map(
  Object.entries(collections).map(([network, groups]) => [
    network,
    new Set(Object.values(groups).flat()),
  ]),
)

/** Canonical keys ignore friendly-address flags but never accept malformed addresses. */
export function nftAddressKey(address: string | undefined | null): string | undefined {
  if (!address) return undefined
  try {
    return Address.parse(address.trim()).toRawString()
  } catch {
    return undefined
  }
}

export function nftCollectionKey(item: Pick<NftItem, "collection_address" | "collection">) {
  return nftAddressKey(item.collection_address ?? item.collection?.address)
}

/** Only addresses reviewed for this network are eligible for automatic image display. */
export function isEcosystemNft(
  item: Pick<NftItem, "collection_address" | "collection" | "is_scam" | "is_nsfw">,
  network: string,
): boolean {
  if (item.is_scam || item.is_nsfw) return false
  const collection = nftCollectionKey(item)
  return collection !== undefined && addressesByNetwork.get(network)?.has(collection) === true
}
