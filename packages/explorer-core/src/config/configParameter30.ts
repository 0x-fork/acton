import type {ParsedValue, ParsedValueMapEntry} from "@acton/ui"

export interface SimplexNoncriticalParameterMetadata {
  readonly name: string
  readonly description?: string
}

// IDs and names are pinned to the TON node's noncritical Simplex parameter registry:
// https://github.com/ton-blockchain/ton/blob/686b56a9b4f0b905386ad2a5ff865eca2506457e/ton/ton-types.h#L529-L545
// Descriptions for IDs 0-14 follow the official ConfigParam 30 reference:
// https://docs.ton.org/foundations/config#param-30-consensus-extension
const SIMPLEX_NONCRITICAL_PARAMETER_METADATA: ReadonlyMap<
  number,
  SimplexNoncriticalParameterMetadata
> = new Map([
  [
    0,
    {
      name: "Target rate",
      description:
        "Target slot or block interval used for leader pacing, block production timing, and skip scheduling.",
    },
  ],
  [
    1,
    {
      name: "First block timeout",
      description:
        "Base timeout before skip voting starts for the first missing block in a leader window.",
    },
  ],
  [
    2,
    {
      name: "First block timeout multiplier",
      description:
        "Multiplier applied to the first-block timeout after a leader window that had skips.",
    },
  ],
  [
    3,
    {
      name: "First block timeout cap",
      description: "Maximum value allowed for adaptive first-block timeout growth.",
    },
  ],
  [
    4,
    {
      name: "Candidate resolve timeout",
      description: "Initial timeout for candidate or notarization resolution requests.",
    },
  ],
  [
    5,
    {
      name: "Candidate resolve timeout multiplier",
      description: "Backoff multiplier applied between candidate resolution retries.",
    },
  ],
  [
    6,
    {
      name: "Candidate resolve timeout cap",
      description: "Maximum value allowed for candidate resolution timeout growth.",
    },
  ],
  [
    7,
    {
      name: "Candidate resolve cooldown",
      description: "Cooldown between candidate resolution attempts.",
    },
  ],
  [
    8,
    {
      name: "Standstill timeout",
      description: "No-progress timeout before standstill recovery or rebroadcast logic begins.",
    },
  ],
  [
    9,
    {
      name: "Standstill max egress bytes per second",
      description: "Egress rate limit applied during standstill rebroadcast.",
    },
  ],
  [
    10,
    {
      name: "Max leader-window desync",
      description: "Maximum tolerated future leader-window distance for inbound Simplex traffic.",
    },
  ],
  [
    11,
    {
      name: "Bad signature ban duration",
      description: "Temporary peer ban duration after receiving an invalid signature.",
    },
  ],
  [
    12,
    {
      name: "Candidate resolve rate limit",
      description: "Per-peer rate limit for candidate resolution requests.",
    },
  ],
  [
    13,
    {
      name: "Min block interval",
      description:
        "Minimum interval between a parent block timestamp and the next locally generated block.",
    },
  ],
  [
    14,
    {
      name: "No empty blocks on error timeout",
      description:
        "How long empty-block fallback remains allowed after the last finalized block when collation fails or times out.",
    },
  ],
  [
    15,
    {
      name: "Certificate gossip neighbors",
    },
  ],
])

export function getNoncriticalParameterMetadata(
  id: number,
): SimplexNoncriticalParameterMetadata | undefined {
  return SIMPLEX_NONCRITICAL_PARAMETER_METADATA.get(id)
}

export function toNoncriticalParameterEntry(
  key: unknown,
  value: unknown,
  toParsedValue: (value: unknown, fieldName?: string) => ParsedValue,
): ParsedValueMapEntry | undefined {
  if (typeof key !== "number") return undefined

  const metadata = getNoncriticalParameterMetadata(key)
  if (!metadata) return undefined

  return {
    key: {kind: "scalar", value: metadata.name},
    keyInfo: {
      id: key,
      ...(metadata.description === undefined ? {} : {description: metadata.description}),
    },
    value: toParsedValue(value, "noncritical_params"),
  }
}
