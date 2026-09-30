import {useCallback, useSyncExternalStore} from "react"
import * as v from "valibot"

import type {NftItem} from "../api/types"
import {isEcosystemNft, nftAddressKey, nftCollectionKey} from "../nfts/ecosystemCollections"
import {createFavoritesStore} from "./favoritesStore"
import {useNetworkInfo} from "./useNetworkInfo"

export type NftImageRevealScope = "once" | "item" | "collection"

/** Indexed identity is sufficient for image consent, including NFT references in history. */
export interface NftImageIdentity
  extends Pick<NftItem, "collection_address" | "collection" | "is_scam" | "is_nsfw"> {
  readonly address?: string
}

interface ImageGrant {
  readonly scope: "item" | "collection"
  readonly address: string
  readonly savedAt: number
}

const CHANGE_EVENT = "acton:nft-images-change"
const persistentGrants = createFavoritesStore<ImageGrant>({
  storagePrefix: "acton:nft-images",
  storageVersion: "v1",
  changeEvent: CHANGE_EVENT,
  recordSchema: v.object({
    scope: v.picklist(["item", "collection"]),
    address: v.string(),
    savedAt: v.number(),
  }),
  normalize: grants => {
    const unique = new Map<string, ImageGrant>()
    for (const grant of grants) {
      const address = nftAddressKey(grant.address)
      if (address) unique.set(`${grant.scope}:${address}`, {...grant, address})
    }
    return [...unique.values()]
  },
})

interface VisibilitySnapshot {
  readonly remembered: readonly ImageGrant[]
  readonly once: ReadonlySet<string>
}

const EMPTY: VisibilitySnapshot = {remembered: persistentGrants.empty, once: new Set()}
const onceByNetwork = new Map<string, ReadonlySet<string>>()
const snapshots = new Map<string, VisibilitySnapshot>()

function readSnapshot(network: string): VisibilitySnapshot {
  const remembered = persistentGrants.read(network)
  const once = onceByNetwork.get(network) ?? EMPTY.once
  const previous = snapshots.get(network)
  if (previous?.remembered === remembered && previous.once === once) return previous

  const snapshot = {remembered, once}
  snapshots.set(network, snapshot)
  return snapshot
}

/**
 * Shares image consent between NFT cards, detail pages and account previews. Temporary grants
 * last until this page reloads; permanent grants are scoped to the browser and network.
 */
export function useNftImageVisibility() {
  const {network, forkNetwork} = useNetworkInfo()
  const namespace = network.id
  const ecosystemNetwork = forkNetwork ?? namespace
  const snapshot = useSyncExternalStore(
    useCallback(listener => persistentGrants.subscribe(namespace, listener), [namespace]),
    useCallback(() => readSnapshot(namespace), [namespace]),
    () => EMPTY,
  )

  const isVisible = useCallback(
    (item: NftImageIdentity) => {
      const address = nftAddressKey(item.address)
      const collection = nftCollectionKey(item)
      return (
        isEcosystemNft(item, ecosystemNetwork) ||
        (address !== undefined && snapshot.once.has(address)) ||
        snapshot.remembered.some(grant =>
          grant.scope === "item" ? grant.address === address : grant.address === collection,
        )
      )
    },
    [snapshot, ecosystemNetwork],
  )

  const reveal = useCallback(
    (item: NftImageIdentity, scope: NftImageRevealScope) => {
      const address = scope === "collection" ? nftCollectionKey(item) : nftAddressKey(item.address)
      if (!address) return

      if (scope === "once") {
        onceByNetwork.set(namespace, new Set([...(onceByNetwork.get(namespace) ?? []), address]))
        globalThis.dispatchEvent?.(new CustomEvent(CHANGE_EVENT, {detail: {namespace}}))
      } else {
        persistentGrants.merge(namespace, [{scope, address, savedAt: Date.now()}])
      }
    },
    [namespace],
  )

  return {isVisible, reveal, ecosystemNetwork}
}
