import type {HTMLAttributes, ReactNode} from "react"

import {CopyInlineAction} from "../InlineActions"
import {Tooltip, type TooltipPlacement} from "../Tooltip"
import styles from "./GramAmount.module.css"

const NANOGRAMS_PER_GRAM = 1_000_000_000n
const NANOGRAM_DECIMALS = 9

export type GramAmountValue = bigint | number | string
export type GramAmountRoundingMode = "half-expand" | "truncate"
export type GramAmountSignDisplay = "auto" | "always" | "except-zero" | "never"

/** Controls how an integer nanogram value is presented as GRAM */
export interface GramAmountFormatOptions {
  /** Text returned when the value is missing or is not an integer nanogram value */
  readonly fallback?: string
  /** Locale used for decimal and optional grouping separators */
  readonly locale?: string
  /** Visible fractional GRAM digits, from 0 to the nanogram precision of 9 */
  readonly maximumFractionDigits?: number
  /** Trailing fractional digits preserved in the visible value */
  readonly minimumFractionDigits?: number
  /** How discarded nanograms affect the last visible fractional digit */
  readonly roundingMode?: GramAmountRoundingMode
  /** Show a lower bound instead of zero when compact precision hides a non-zero value */
  readonly showLessThanMinimum?: boolean
  /** Include the `GRAM` suffix; disable it for inputs that provide their own suffix */
  readonly showUnit?: boolean
  /** Controls the sign without changing the raw nanogram value */
  readonly signDisplay?: GramAmountSignDisplay
  /** Add locale-aware separators to the whole GRAM value */
  readonly useGrouping?: boolean
}

/** Props for a semantic GRAM value with an exact-value tooltip */
export interface GramAmountProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "title" | "value">,
    Omit<GramAmountFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  /** Show exact GRAM and nanogram values with copy actions */
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  /** Integer nanograms, not a decimal GRAM value */
  readonly value: GramAmountValue | null | undefined
}

interface FormattedGramAmount {
  readonly nanograms: bigint
  readonly text: string
}

/**
 * Formats an integer nanogram value as GRAM without converting it through a
 * JavaScript number. The default output keeps all significant nanogram digits
 * and appends the `GRAM` unit.
 */
export function formatGramAmount(
  value: GramAmountValue | null | undefined,
  options: GramAmountFormatOptions = {},
): string {
  return formatGramAmountValue(value, options)?.text ?? options.fallback ?? "—"
}

export function GramAmount({
  value,
  locale,
  maximumFractionDigits,
  minimumFractionDigits,
  roundingMode,
  showLessThanMinimum,
  showUnit,
  signDisplay,
  useGrouping,
  fallback = "—",
  tooltip = true,
  tooltipPlacement = "top",
  className,
  tabIndex,
  ...props
}: GramAmountProps) {
  const options = {
    locale,
    maximumFractionDigits,
    minimumFractionDigits,
    roundingMode,
    showLessThanMinimum,
    showUnit,
    signDisplay,
    useGrouping,
  }
  const formatted = formatGramAmountValue(value, options)
  if (!formatted) return fallback

  const amount = (
    <data
      data-visual-dynamic="gram-amount"
      data-visual-placeholder="<gram>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
      value={formatted.nanograms.toString()}
    >
      {formatted.text}
    </data>
  )

  return tooltip ? (
    <Tooltip
      content={<GramAmountTooltip nanograms={formatted.nanograms} locale={locale} />}
      placement={tooltipPlacement}
      width="wide"
    >
      {amount}
    </Tooltip>
  ) : (
    amount
  )
}

function formatGramAmountValue(
  value: GramAmountValue | null | undefined,
  options: GramAmountFormatOptions,
): FormattedGramAmount | undefined {
  const nanograms = gramAmountValueToBigInt(value)
  if (nanograms === undefined) return undefined

  const maximumFractionDigits = clampFractionDigits(options.maximumFractionDigits ?? 9)
  const minimumFractionDigits = Math.min(
    clampFractionDigits(options.minimumFractionDigits ?? 0),
    maximumFractionDigits,
  )
  const negative = nanograms < 0n
  const absolute = negative ? -nanograms : nanograms
  const roundingStep = 10n ** BigInt(NANOGRAM_DECIMALS - maximumFractionDigits)
  const roundedAbsolute =
    maximumFractionDigits === NANOGRAM_DECIMALS
      ? absolute
      : options.roundingMode === "truncate"
        ? (absolute / roundingStep) * roundingStep
        : ((absolute + roundingStep / 2n) / roundingStep) * roundingStep

  if (
    options.showLessThanMinimum &&
    absolute > 0n &&
    roundedAbsolute === 0n &&
    maximumFractionDigits < NANOGRAM_DECIMALS
  ) {
    const smallest = decimalGramText(roundingStep, maximumFractionDigits, maximumFractionDigits)
    const comparison = negative ? ">-" : "<"
    return {
      nanograms,
      text: `${comparison}${smallest}${options.showUnit === false ? "" : " GRAM"}`,
    }
  }

  const sign = amountSign(nanograms, options.signDisplay)
  const whole = roundedAbsolute / NANOGRAMS_PER_GRAM
  const fraction = (roundedAbsolute % NANOGRAMS_PER_GRAM)
    .toString()
    .padStart(NANOGRAM_DECIMALS, "0")
    .slice(0, maximumFractionDigits)
    .replace(/0+$/, "")
    .padEnd(minimumFractionDigits, "0")
  const wholeText = options.useGrouping
    ? new Intl.NumberFormat(options.locale, {maximumFractionDigits: 0}).format(whole)
    : whole.toString()
  const decimalSeparator = fraction ? localeDecimalSeparator(options.locale) : ""
  const unit = options.showUnit === false ? "" : " GRAM"

  return {
    nanograms,
    text: `${sign}${wholeText}${decimalSeparator}${fraction}${unit}`,
  }
}

function decimalGramText(
  nanograms: bigint,
  maximumFractionDigits: number,
  minimumFractionDigits: number,
): string {
  const whole = nanograms / NANOGRAMS_PER_GRAM
  const fraction = (nanograms % NANOGRAMS_PER_GRAM)
    .toString()
    .padStart(NANOGRAM_DECIMALS, "0")
    .slice(0, maximumFractionDigits)
    .replace(/0+$/, "")
    .padEnd(minimumFractionDigits, "0")
  return fraction ? `${whole}.${fraction}` : whole.toString()
}

function amountSign(value: bigint, signDisplay: GramAmountSignDisplay = "auto"): string {
  if (signDisplay === "never") return ""
  if (value < 0n) return "-"
  if (signDisplay === "always" || (signDisplay === "except-zero" && value !== 0n)) return "+"
  return ""
}

function gramAmountValueToBigInt(value: GramAmountValue | null | undefined): bigint | undefined {
  if (typeof value === "bigint") return value
  if (typeof value === "number") {
    return Number.isSafeInteger(value) ? BigInt(value) : undefined
  }
  if (typeof value !== "string" || !/^[+-]?\d+$/.test(value.trim())) return undefined

  try {
    return BigInt(value.trim())
  } catch {
    return undefined
  }
}

function clampFractionDigits(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.max(0, Math.min(NANOGRAM_DECIMALS, Math.trunc(value)))
}

function localeDecimalSeparator(locale: string | undefined): string {
  return (
    new Intl.NumberFormat(locale).formatToParts(1.1).find(part => part.type === "decimal")?.value ??
    "."
  )
}

function GramAmountTooltip({
  nanograms,
  locale,
}: {
  readonly nanograms: bigint
  readonly locale?: string
}) {
  const exactGram = formatGramAmount(nanograms, {locale, showUnit: false})
  const rawNanograms = nanograms.toString()

  return (
    <span className={styles.tooltip}>
      <span className={styles.tooltipRow}>
        <span>GRAM</span>
        <span className={styles.tooltipCopyValue}>
          <code>{exactGram}</code>
          <CopyInlineAction
            copiedLabel="GRAM amount copied"
            label="Copy GRAM amount"
            size="compact"
            value={exactGram}
          />
        </span>
      </span>
      <span className={styles.tooltipRow}>
        <span>Nanograms</span>
        <span className={styles.tooltipCopyValue}>
          <code>{rawNanograms}</code>
          <CopyInlineAction
            copiedLabel="Nanogram amount copied"
            label="Copy nanogram amount"
            size="compact"
            value={rawNanograms}
          />
        </span>
      </span>
    </span>
  )
}
