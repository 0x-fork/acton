export type LookupTarget = {
  readonly kind: "address" | "code_hash"
  readonly value: string
}

const HEX_CODE_HASH = /^[0-9a-fA-F]{64}$/

export function parseLookupTarget(rawValue: string): LookupTarget {
  const value = rawValue.trim()
  if (value.length === 0) {
    throw new Error("Enter a contract address or code hash")
  }

  return {
    kind: HEX_CODE_HASH.test(value) ? "code_hash" : "address",
    value,
  }
}

export function lookupTargetToQuery(target: LookupTarget): string {
  const key = target.kind === "code_hash" ? "code_hash" : "address"
  return `${key}=${encodeURIComponent(target.value)}`
}

export function lookupPath(rawValue: string): string {
  return `/${encodeURIComponent(rawValue.trim())}`
}

export function getPathLookupValue(): string {
  const queryTarget = new URLSearchParams(window.location.search).get("target")
  if (queryTarget?.trim()) {
    return queryTarget.trim()
  }

  const value = window.location.pathname.replace(/^\/+/, "")
  return decodeURIComponent(value)
}

export function shortenMiddle(value: string, left = 10, right = 8): string {
  if (value.length <= left + right + 3) {
    return value
  }
  return `${value.slice(0, left)}...${value.slice(-right)}`
}
