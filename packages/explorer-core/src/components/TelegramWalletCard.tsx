import {useEffect, useMemo, useRef, useState} from "react"
import type {PointerEvent} from "react"
import {encode} from "uqr"

import {useNetworkInfo} from "../hooks/useNetworkInfo"
import {normalizeAddress} from "./utils"
import gemImage from "../assets/gram-gem-white.webp"
import blueGemImage from "../assets/gram-gem-blue.webp"
import styles from "./TelegramWalletCard.module.css"

const stars = [
  [87.12, 47],
  [312.6, 156],
  [29.72, 22],
  [249.06, 195],
  [121.97, 62],
  [227.53, 138],
  [248.03, 164],
  [69.69, 20],
  [127.09, 36],
  [56.37, 67],
  [177, 24],
  [219, 53],
  [284, 28],
  [338, 48],
  [330, 112],
  [292, 132],
  [181, 151],
  [142, 172],
  [47, 145],
  [95, 112],
] as const

interface TelegramWalletCardProps {
  readonly address: string
  readonly balanceNanograms: string
}

/** Draws the source card's circular QR modules and rounded finder markers. */
function TelegramWalletQr({address}: {readonly address: string}) {
  const rootRef = useRef<HTMLSpanElement>(null)
  const [shown, setShown] = useState(() => matchMedia("(prefers-reduced-motion: reduce)").matches)
  const value = `ton://transfer/${address}`
  const qr = useMemo(() => {
    const {size, data} = encode(value, {ecc: "H", border: 0})
    const holeSize = Math.ceil(size * 0.22) | 1
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
  }, [value])

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
    <span ref={rootRef} className={`${styles.qr} ${shown ? styles.qrShown : ""}`}>
      <svg viewBox={`-1 -1 ${qr.size + 2} ${qr.size + 2}`} role="img" aria-label={value}>
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

/** Presents the on-chain balance as a Telegram Wallet card on account pages. */
export function TelegramWalletCard({address, balanceNanograms}: TelegramWalletCardProps) {
  const {addressFormat, network} = useNetworkInfo()
  const [usdPrice, setUsdPrice] = useState<number | undefined>()
  const [flipped, setFlipped] = useState(false)
  const cardRef = useRef<HTMLDivElement>(null)
  const motionRef = useRef({x: -1.8, y: 1.2, targetX: 0, targetY: 0, frame: 0, intro: true})
  const cardAddress = normalizeAddress(address, {...addressFormat, bounceable: false})
  const addressChunks = cardAddress.match(/.{1,4}/g) ?? []
  const firstAddressLine = addressChunks.slice(0, 6).join(" ")
  const secondAddressLine = addressChunks.slice(6).join(" ")
  const whole = new Intl.NumberFormat("en-US", {maximumFractionDigits: 0}).format(
    BigInt(balanceNanograms) / 1_000_000_000n,
  )
  const fraction = String((BigInt(balanceNanograms) % 1_000_000_000n) / 10_000_000n)
    .padStart(2, "0")
    .replace(/0+$/, "")
  const balance = `${whole}${fraction ? `.${fraction}` : ""}`

  const setTiltTarget = (x: number, y: number) => {
    const motion = motionRef.current
    motion.targetX = x
    motion.targetY = y
    if (motion.frame) return

    const animate = () => {
      motion.frame = 0
      const easing = motion.intro ? 0.09 : 0.2
      motion.x += (motion.targetX - motion.x) * easing
      motion.y += (motion.targetY - motion.y) * easing
      if (
        Math.abs(motion.targetX - motion.x) >= 0.002 ||
        Math.abs(motion.targetY - motion.y) >= 0.002
      ) {
        motion.frame = requestAnimationFrame(animate)
      } else {
        motion.x = motion.targetX
        motion.y = motion.targetY
      }
      cardRef.current?.style.setProperty("--tx", motion.x.toFixed(4))
      cardRef.current?.style.setProperty("--ty", motion.y.toFixed(4))
    }

    motion.frame = requestAnimationFrame(animate)
  }

  useEffect(() => {
    if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setTiltTarget(0, 0)
    }

    return () => {
      cancelAnimationFrame(motionRef.current.frame)
      motionRef.current.frame = 0
    }
  }, [])

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (event.pointerType !== "mouse" || matchMedia("(prefers-reduced-motion: reduce)").matches)
      return
    const rect = event.currentTarget.getBoundingClientRect()
    const clamp = (value: number) => Math.max(-1, Math.min(1, value))
    motionRef.current.intro = false
    setTiltTarget(
      clamp(((event.clientX - rect.left) / rect.width) * 2 - 1),
      clamp(((event.clientY - rect.top) / rect.height) * 2 - 1),
    )
  }

  useEffect(() => {
    if (network.id !== "mainnet") return

    const controller = new AbortController()
    void fetch("https://gram.org/api/price", {signal: controller.signal})
      .then(response => response.json())
      .then((data: {price?: unknown}) => {
        if (typeof data.price === "number" && Number.isFinite(data.price)) {
          setUsdPrice(data.price)
        }
      })
      .catch(() => {})

    return () => controller.abort()
  }, [network.id])

  const fiatValue =
    usdPrice === undefined
      ? undefined
      : ((Number(balanceNanograms) / 1_000_000_000) * usdPrice).toLocaleString("en-US", {
          style: "currency",
          currency: "USD",
        })

  return (
    <div className={styles.frame} data-account-context-card="telegram-wallet">
      <div className={`${styles.wrap} ${flipped ? styles.flipped : ""}`}>
        <div className={styles.turn}>
          <div
            ref={cardRef}
            className={styles.card}
            role="group"
            aria-label={`Telegram Wallet with ${balance} GRAM`}
            onPointerMove={handlePointerMove}
            onPointerLeave={() => {
              motionRef.current.intro = false
              setTiltTarget(0, 0)
            }}
          >
            <div className={styles.shine} aria-hidden="true" />
            <div className={styles.grain} aria-hidden="true" />
            <div className={styles.stars} aria-hidden="true">
              {stars.map(([x, y], index) => (
                <i
                  key={`${x}-${y}`}
                  className={styles.star}
                  style={{
                    left: `${(x / 370) * 100}%`,
                    top: `${(y / 220) * 100}%`,
                    animationDuration: `${(4.2 + ((index * 0.41) % 1) * 2.6) * 0.5}s`,
                    animationDelay: `${-((index * 0.73) % 2.1).toFixed(2)}s`,
                  }}
                />
              ))}
            </div>
            <div className={styles.balance}>
              <span className={styles.gem}>
                <img src={gemImage} alt="" draggable={false} />
              </span>
              <span className={styles.figure}>
                <span>{whole}</span>
                {fraction && <span className={styles.fraction}>.{fraction}</span>}
                <span className={styles.unit}>GRAM</span>
              </span>
            </div>
            {fiatValue && <span className={styles.fiat}>{fiatValue}</span>}
            <span className={styles.name}>Telegram Wallet</span>
            <button
              type="button"
              className={styles.chip}
              aria-label="Show wallet QR code"
              onClick={() => setFlipped(true)}
            >
              <svg viewBox="0 0 50 38" aria-hidden="true">
                {["#e6edf5", "#757c83"].map((color, index) => (
                  <g
                    key={color}
                    fill={color}
                    stroke={color}
                    transform={index === 0 ? "translate(0 1)" : undefined}
                  >
                    {[
                      [15.75, 9.75],
                      [26, 9.75],
                      [15.75, 20],
                    ].map(([x, y]) => (
                      <rect
                        key={`${x}-${y}`}
                        x={x + 0.69}
                        y={y + 0.69}
                        width="6.87"
                        height="6.87"
                        rx="1.75"
                        fill="none"
                        strokeWidth="1.375"
                      />
                    ))}
                    {[
                      [26.5, 21],
                      [31.62, 21],
                      [29.1, 23.3],
                      [26.5, 26.15],
                      [31.62, 26.15],
                    ].map(([x, y]) => (
                      <rect
                        key={`${x}-${y}`}
                        x={x}
                        y={y}
                        width="2.12"
                        height="2.1"
                        rx="0.45"
                        stroke="none"
                      />
                    ))}
                  </g>
                ))}
              </svg>
            </button>
            <span className={styles.address} aria-hidden="true">
              <span>{firstAddressLine}</span>
              <span>{secondAddressLine}</span>
            </span>
          </div>
          <button
            type="button"
            className={styles.back}
            aria-label="Return to Telegram Wallet card"
            onClick={() => setFlipped(false)}
          >
            <TelegramWalletQr address={cardAddress} />
          </button>
        </div>
      </div>
    </div>
  )
}
