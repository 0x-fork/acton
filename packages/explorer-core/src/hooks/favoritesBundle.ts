import {Address} from "@ton/core"

import type {RegisteredAddressName} from "../metadata/types"
import {parseFavoriteAccounts, type FavoriteAccount} from "./useFavoriteAccounts"
import {parseFavoriteBlocks, type FavoriteBlock} from "./useFavoriteBlocks"
import {parseFavoriteTransactions, type FavoriteTransaction} from "./useFavoriteTransactions"

export const FAVORITES_BUNDLE_FORMAT = "acton-favorites"
export const FAVORITES_BUNDLE_VERSION = 1

export interface FavoritesBundle {
  readonly format: typeof FAVORITES_BUNDLE_FORMAT
  readonly version: typeof FAVORITES_BUNDLE_VERSION
  readonly exportedAt: string
  readonly network: string
  readonly accounts: readonly FavoriteAccount[]
  readonly blocks: readonly FavoriteBlock[]
  readonly transactions: readonly FavoriteTransaction[]
  readonly addressNames: readonly RegisteredAddressName[]
}

export interface CreateFavoritesBundleOptions {
  readonly network: string
  readonly accounts: readonly FavoriteAccount[]
  readonly blocks: readonly FavoriteBlock[]
  readonly transactions: readonly FavoriteTransaction[]
  readonly addressNames: readonly RegisteredAddressName[]
}

export function createFavoritesBundle(options: CreateFavoritesBundleOptions): FavoritesBundle {
  return {
    format: FAVORITES_BUNDLE_FORMAT,
    version: FAVORITES_BUNDLE_VERSION,
    exportedAt: new Date().toISOString(),
    network: options.network,
    accounts: [...options.accounts],
    blocks: [...options.blocks],
    transactions: [...options.transactions],
    addressNames: normalizeAddressNames(options.addressNames),
  }
}

export function parseFavoritesBundle(raw: string): FavoritesBundle {
  let value: unknown
  try {
    value = JSON.parse(raw) as unknown
  } catch {
    throw new Error("The selected file is not valid JSON")
  }

  if (!isRecord(value)) {
    throw new Error("The JSON root must be an object")
  }
  if (value.format !== FAVORITES_BUNDLE_FORMAT || value.version !== FAVORITES_BUNDLE_VERSION) {
    throw new Error("This file is not a supported Acton favorites bundle")
  }

  const network = typeof value.network === "string" ? value.network.trim() : ""
  if (!network) {
    throw new Error("The favorites bundle does not specify a network")
  }

  return {
    format: FAVORITES_BUNDLE_FORMAT,
    version: FAVORITES_BUNDLE_VERSION,
    exportedAt: typeof value.exportedAt === "string" ? value.exportedAt : "",
    network,
    accounts: parseFavoriteList(value.accounts, parseFavoriteAccounts, "accounts"),
    blocks: parseFavoriteList(value.blocks, parseFavoriteBlocks, "blocks"),
    transactions: parseFavoriteList(value.transactions, parseFavoriteTransactions, "transactions"),
    addressNames: parseAddressNames(value.addressNames),
  }
}

function parseFavoriteList<T>(
  value: unknown,
  parse: (raw: string | null) => readonly T[],
  label: string,
): readonly T[] {
  if (value === undefined) {
    return []
  }
  if (!Array.isArray(value)) {
    throw new Error(`The ${label} section must be an array`)
  }
  return parse(JSON.stringify(value))
}

function parseAddressNames(value: unknown): readonly RegisteredAddressName[] {
  if (value === undefined) {
    return []
  }
  if (!Array.isArray(value)) {
    throw new Error("The address names section must be an array")
  }

  const namesByAddress = new Map<string, RegisteredAddressName>()
  for (const candidate of value) {
    if (
      !isRecord(candidate) ||
      typeof candidate.address !== "string" ||
      typeof candidate.name !== "string"
    ) {
      continue
    }

    const address = candidate.address.trim()
    const name = candidate.name.trim()
    if (!address || !name) {
      continue
    }

    const key = normalizeAddressKey(address)
    if (!namesByAddress.has(key)) {
      namesByAddress.set(key, {address, name})
    }
  }

  return [...namesByAddress.values()]
}

function normalizeAddressNames(
  entries: readonly RegisteredAddressName[],
): readonly RegisteredAddressName[] {
  const namesByAddress = new Map<string, RegisteredAddressName>()
  for (const entry of entries) {
    const address = entry.address.trim()
    const name = entry.name.trim()
    if (!address || !name) {
      continue
    }

    const key = normalizeAddressKey(address)
    namesByAddress.set(key, {address, name})
  }
  return [...namesByAddress.values()]
}

function normalizeAddressKey(address: string): string {
  try {
    return Address.parse(address).toRawString()
  } catch {
    return address
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
