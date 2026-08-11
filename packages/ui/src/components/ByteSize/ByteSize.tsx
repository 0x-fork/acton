import type {HTMLAttributes, ReactNode} from "react"

import {CopyInlineAction} from "../InlineActions"
import {formatNumberValue} from "../NumberValue"
import {Tooltip, type TooltipPlacement} from "../Tooltip"

import styles from "./ByteSize.module.css"

const BYTE_BASE = 1024
const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const
export type ByteSizeUnit = "bytes" | "kilobytes" | "megabytes" | "gigabytes" | "terabytes"

const BYTE_UNIT_MULTIPLIERS = {
  bytes: BYTE_BASE ** 0,
  kilobytes: BYTE_BASE ** 1,
  megabytes: BYTE_BASE ** 2,
  gigabytes: BYTE_BASE ** 3,
  terabytes: BYTE_BASE ** 4,
} as const satisfies Record<ByteSizeUnit, number>

export interface ByteSizeFormatOptions {
  readonly fallback?: string
  readonly locale?: string
  readonly maximumFractionDigits?: number
  readonly unit?: ByteSizeUnit
}

export interface ByteSizeProps
  extends Omit<HTMLAttributes<HTMLDataElement>, "children" | "value">,
    Omit<ByteSizeFormatOptions, "fallback"> {
  readonly fallback?: ReactNode
  readonly tooltip?: boolean
  readonly tooltipPlacement?: TooltipPlacement
  readonly value: number | null | undefined
}

/** Formats a byte count with a binary unit */
export function formatByteSize(
  value: number | null | undefined,
  options: ByteSizeFormatOptions = {},
): string {
  if (value === null || value === undefined || !Number.isFinite(value) || value < 0) {
    return options.fallback ?? "—"
  }

  const resolvedUnit = options.unit ?? "bytes"
  const bytes = Math.trunc(value * BYTE_UNIT_MULTIPLIERS[resolvedUnit])
  if (bytes < BYTE_BASE) return `${formatNumberValue(bytes, {locale: options.locale})} B`

  const unitIndex = Math.min(
    BYTE_UNITS.length - 1,
    Math.floor(Math.log(bytes) / Math.log(BYTE_BASE)),
  )
  const scaled = bytes / BYTE_BASE ** unitIndex
  const maximumFractionDigits = options.maximumFractionDigits ?? (scaled < 10 ? 1 : 0)
  return `${formatNumberValue(scaled, {
    locale: options.locale,
    maximumFractionDigits,
  })} ${BYTE_UNITS[unitIndex]}`
}

export function ByteSize({
  value,
  fallback = "—",
  locale,
  maximumFractionDigits,
  unit,
  tooltip = true,
  tooltipPlacement = "top",
  title,
  className,
  tabIndex,
  ...props
}: ByteSizeProps) {
  if (!isByteSizeValue(value)) return fallback

  const formatted = formatByteSize(value, {fallback: "", locale, maximumFractionDigits, unit})
  if (!formatted) return fallback

  const size = (
    <data
      data-visual-dynamic="byte-size"
      data-visual-placeholder="<size>"
      {...props}
      className={
        [className, tooltip ? styles.trigger : undefined].filter(Boolean).join(" ") || undefined
      }
      tabIndex={tooltip ? (tabIndex ?? 0) : tabIndex}
      value={value}
    >
      {formatted}
    </data>
  )

  return tooltip ? (
    <Tooltip
      content={<ByteSizeTooltip heading={title} unit={unit} value={value} />}
      placement={tooltipPlacement}
      width="wide"
    >
      {size}
    </Tooltip>
  ) : (
    size
  )
}

function ByteSizeTooltip({
  heading,
  unit,
  value,
}: {
  readonly heading?: string
  readonly unit?: ByteSizeUnit
  readonly value: number
}) {
  const rawValue = value.toString()
  const resolvedUnit = unit ?? "bytes"

  return (
    <span className={styles.tooltip}>
      {heading ? <strong>{heading}</strong> : undefined}
      <span className={styles.tooltipRow}>
        <span>Raw value</span>
        <span className={styles.tooltipCopyValue}>
          <code>{rawValue}</code>
          <CopyInlineAction
            copiedLabel="Raw value copied"
            label="Copy raw value"
            size="compact"
            value={rawValue}
          />
        </span>
      </span>
      <span className={styles.tooltipRow}>
        <span>Unit</span>
        <span>{resolvedUnit}</span>
      </span>
    </span>
  )
}

function isByteSizeValue(value: number | null | undefined): value is number {
  return value !== null && value !== undefined && Number.isFinite(value) && value >= 0
}
