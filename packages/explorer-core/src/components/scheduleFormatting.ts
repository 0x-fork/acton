export const SECONDS_PER_DAY = 86_400

export function formatGramAmount(value: bigint, maximumFractionDigits = 2): string {
  const fractionDigits = Math.max(0, Math.min(9, Math.trunc(maximumFractionDigits)))
  const negative = value < 0n
  const absolute = negative ? -value : value
  const roundingStep = 10n ** BigInt(9 - fractionDigits)
  const rounded =
    fractionDigits === 9 ? absolute : ((absolute + roundingStep / 2n) / roundingStep) * roundingStep
  const whole = rounded / 1_000_000_000n
  const fraction = (rounded % 1_000_000_000n)
    .toString()
    .padStart(9, "0")
    .slice(0, fractionDigits)
    .replace(/0+$/, "")
  return `${negative ? "-" : ""}${whole.toLocaleString()}${fraction ? `.${fraction}` : ""} GRAM`
}

export function capitalize<T extends string>(value: T): Capitalize<T> {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}` as Capitalize<T>
}
