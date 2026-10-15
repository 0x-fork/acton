import {useEffect, useMemo, useRef, useState} from "react"
import {encode} from "uqr"

import blueGemImage from "../assets/gram-gem-blue.webp"
import styles from "./StyledQrCode.module.css"

interface StyledQrCodeProps {
  readonly value: string
  readonly size: number | string
  readonly label?: string
  readonly errorCorrection?: "M" | "H"
  readonly appearance?: "light" | "adaptive"
}

/** Renders scannable address and transfer QR codes, keeping transfer cards light by default. */
export function StyledQrCode({
  value,
  size,
  label = value,
  errorCorrection = "H",
  appearance = "light",
}: StyledQrCodeProps) {
  const rootRef = useRef<HTMLSpanElement>(null)
  const [shown, setShown] = useState(() => matchMedia("(prefers-reduced-motion: reduce)").matches)
  const qr = useMemo(() => {
    const {size, data} = encode(value, {ecc: errorCorrection, border: 0})
    const holeSize = Math.ceil(size * (errorCorrection === "H" ? 0.22 : 0.16)) | 1
    const holeStart = (size - holeSize) / 2
    const finders = [
      [0, 0],
      [size - 7, 0],
      [0, size - 7],
    ] as const
    const dots: string[] = []

    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const inFinder = finders.some(
          ([left, top]) => x >= left && x < left + 7 && y >= top && y < top + 7,
        )
        const inLogo =
          x >= holeStart && x < holeStart + holeSize && y >= holeStart && y < holeStart + holeSize
        if (data[y][x] && !inFinder && !inLogo) {
          dots.push(`M${x + 0.5} ${y + 0.08}a.42.42 0 1 0 .001 0`)
        }
      }
    }

    return {size, holeSize, finders, dots: dots.join("")}
  }, [value, errorCorrection])

  useEffect(() => {
    if (shown || !rootRef.current) return

    let frame = 0
    const observer = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting) {
        observer.disconnect()
        frame = requestAnimationFrame(() => setShown(true))
      }
    })
    observer.observe(rootRef.current)

    return () => {
      observer.disconnect()
      cancelAnimationFrame(frame)
    }
  }, [shown])

  return (
    <span
      ref={rootRef}
      className={`${styles.qr} ${appearance === "adaptive" ? styles.qrAdaptive : ""} ${shown ? styles.qrShown : ""}`}
      style={{width: size, height: size}}
    >
      <svg viewBox={`-1 -1 ${qr.size + 2} ${qr.size + 2}`} role="img" aria-label={label}>
        <path d={qr.dots} fill="currentColor" />
        {qr.finders.map(([x, y]) => (
          <g key={`${x}-${y}`}>
            <rect
              x={x + 0.5}
              y={y + 0.5}
              width="6"
              height="6"
              rx="1.9"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            />
            <rect x={x + 2} y={y + 2} width="3" height="3" rx="0.9" fill="currentColor" />
          </g>
        ))}
      </svg>
      <span
        className={styles.qrLogo}
        style={{
          width: `${(qr.holeSize / (qr.size + 2)) * 100}%`,
          height: `${(qr.holeSize / (qr.size + 2)) * 100}%`,
        }}
      >
        <img src={blueGemImage} alt="" />
      </span>
    </span>
  )
}
