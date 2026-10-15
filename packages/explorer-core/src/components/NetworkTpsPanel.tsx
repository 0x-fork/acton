import {formatSchedulePeriod, RelativeTime, Skeleton} from "@acton/ui"
import {useEffect, useRef, useState} from "react"
import type {FC} from "react"

import type {LoadNetworkTps, NetworkTpsSnapshot} from "../api/networkStats"

import styles from "./NetworkTpsPanel.module.css"

const REFRESH_INTERVAL_MS = 5000
const tpsFormatter = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
})
const countFormatter = new Intl.NumberFormat("en-US")

interface NetworkTpsPanelProps {
  readonly loadNetworkTps: LoadNetworkTps
}

export const NetworkTpsPanel: FC<NetworkTpsPanelProps> = ({loadNetworkTps}) => {
  const [snapshot, setSnapshot] = useState<NetworkTpsSnapshot>()
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    let active = true
    let timeoutId: ReturnType<typeof setTimeout> | undefined
    let controller: AbortController | undefined

    const load = async () => {
      controller = new AbortController()
      try {
        const next = await loadNetworkTps(controller.signal)
        if (active) {
          setSnapshot(next)
          setFailed(false)
        }
      } catch (error) {
        if (active && !(error instanceof DOMException && error.name === "AbortError")) {
          setFailed(true)
        }
      } finally {
        if (active) {
          timeoutId = globalThis.setTimeout(() => void load(), REFRESH_INTERVAL_MS)
        }
      }
    }

    void load()
    return () => {
      active = false
      controller?.abort()
      if (timeoutId !== undefined) globalThis.clearTimeout(timeoutId)
    }
  }, [loadNetworkTps])

  if ((!snapshot && failed) || (snapshot && snapshot.latest_masterchain_seqno === undefined)) {
    return null
  }

  return (
    <section className={styles.frame} aria-label="Network TPS">
      <header className={styles.header}>
        <span>Network TPS</span>
        {failed && snapshot ? (
          <span className={styles.meta}>Updates temporarily unavailable</span>
        ) : snapshot?.latest_masterchain_seqno === undefined ? null : (
          <span className={styles.meta}>
            {snapshot.status === "syncing" ? "Syncing" : "Updated"} at masterchain block{" "}
            {countFormatter.format(snapshot.latest_masterchain_seqno)}
            {snapshot.latest_block_time === undefined ? null : (
              <>
                {" · "}
                <RelativeTime value={snapshot.latest_block_time} unit="seconds" />
              </>
            )}
          </span>
        )}
      </header>
      {snapshot ? (
        <div className={styles.grid}>
          {snapshot.windows.map(window => (
            <div className={styles.metric} key={window.window_seconds}>
              <span className={styles.period}>{formatSchedulePeriod(window.window_seconds)}</span>
              <strong className={styles.value}>
                <AnimatedNumber value={window.tps} format={formatTps} />
              </strong>
              <span className={styles.detail}>
                <AnimatedNumber value={window.transactions} format={formatCount} /> transactions
                {window.complete ? "" : " · partial window"}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className={styles.grid} aria-label="Loading network TPS">
          {[60, 300, 900].map(windowSeconds => (
            <div className={styles.metric} key={windowSeconds}>
              <Skeleton width="3.5rem" />
              <Skeleton width="5.5rem" height="1.75rem" />
              <Skeleton width="8rem" />
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

function AnimatedNumber({value, format}: {value: number; format: (value: number) => string}) {
  const [displayed, setDisplayed] = useState(value)
  const displayedRef = useRef(value)

  useEffect(() => {
    if (globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      displayedRef.current = value
      setDisplayed(value)
      return
    }

    const from = displayedRef.current
    if (from === value) return

    let frame: number
    let startedAt: number | undefined
    const animate = (now: number) => {
      startedAt ??= now
      const progress = Math.min((now - startedAt) / 650, 1)
      const eased = 1 - (1 - progress) ** 3
      const next = from + (value - from) * eased

      displayedRef.current = next
      setDisplayed(next)

      if (progress < 1) frame = globalThis.requestAnimationFrame(animate)
    }

    frame = globalThis.requestAnimationFrame(animate)
    return () => globalThis.cancelAnimationFrame(frame)
  }, [value])

  return format(displayed)
}

function formatTps(value: number): string {
  return tpsFormatter.format(value)
}

function formatCount(value: number): string {
  return countFormatter.format(Math.round(value))
}
