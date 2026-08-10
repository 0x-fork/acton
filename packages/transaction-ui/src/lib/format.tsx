import {Address} from "@ton/core"
import {shortenMiddle} from "@acton/ui"

export function formatContractLetter(index: number): string {
  if (!Number.isSafeInteger(index) || index < 0) {
    return "?"
  }

  let remaining = index
  let letter = ""
  do {
    letter = String.fromCharCode(65 + (remaining % 26)) + letter
    remaining = Math.floor(remaining / 26) - 1
  } while (remaining >= 0)

  return letter
}

export function formatAddress(address: string): string {
  if (!address) return "unknown"
  try {
    const parsed = Address.parse(address)
    const displayAddress = parsed.toString({testOnly: true})
    return shortenMiddle(displayAddress, {start: 6, end: 6, separator: "..."})
  } catch {
    if (address.length <= 12) return address
    return shortenMiddle(address, {start: 6, end: 6, separator: "..."})
  }
}
