import type {TpsPoint, TpsView} from "../types"
import {useTheme} from "@acton/ui"
import {BarChart} from "echarts/charts"
import {
  AriaComponent,
  DataZoomComponent,
  GridComponent,
  MarkLineComponent,
  TooltipComponent,
} from "echarts/components"
import {init, use, type EChartsCoreOption, type EChartsType} from "echarts/core"
import {CanvasRenderer} from "echarts/renderers"
import {useEffect, useRef} from "react"
import styles from "./TpsSection.module.css"

interface TpsSectionProps {
  readonly series: TpsView | undefined
}

const DEFAULT_VISIBLE_SECONDS = 10 * 60

use([
  AriaComponent,
  BarChart,
  CanvasRenderer,
  DataZoomComponent,
  GridComponent,
  MarkLineComponent,
  TooltipComponent,
])

/** Presents recent whole-network transaction throughput from the embedded indexer */
function TpsSection({series}: TpsSectionProps) {
  const points = series?.points ?? []
  const current = points.at(-1)
  const peak = points.reduce((maximum, point) => Math.max(maximum, point.tps), 0)
  const queueSize = series?.queue_size

  return (
    <section id="throughput" className={styles.section} aria-labelledby="throughput-title">
      <div className={styles.heading}>
        <h2 id="throughput-title">Transaction throughput</h2>
      </div>
      <div className={styles.panel}>
        <div className={styles.summary}>
          <ThroughputMetric
            label="Current"
            value={current ? `${formatTps(current.tps)} TPS` : "—"}
          />
          <ThroughputMetric
            label="Peak"
            value={points.length > 0 ? `${formatTps(peak)} TPS` : "—"}
          />
          <ThroughputMetric
            label="Queue"
            value={queueSize === undefined || queueSize === null ? "—" : queueSize.toLocaleString()}
          />
        </div>

        {series?.status === "unavailable" ? (
          <ChartState>TPS indexing is available on the network collector</ChartState>
        ) : points.length === 0 ? (
          <ChartState>Indexing recent blocks</ChartState>
        ) : (
          <TpsChart points={points} />
        )}
      </div>
    </section>
  )
}

function ThroughputMetric({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <div className={styles.metric}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function ChartState({children}: {readonly children: string}) {
  return (
    <div className={styles.chartState}>
      <span className={styles.pulse} aria-hidden="true" />
      <span>{children}</span>
    </div>
  )
}

function TpsChart({points}: {readonly points: readonly TpsPoint[]}) {
  const elementRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<EChartsType | undefined>(undefined)
  const extentRef = useRef({from: 0, to: 0})
  const renderKeyRef = useRef("")
  const zoomRef = useRef<ZoomRange | undefined>(undefined)
  const {theme} = useTheme()

  useEffect(() => {
    const element = elementRef.current
    if (!element) return

    const chart = init(element, undefined, {renderer: "canvas"})
    chartRef.current = chart
    const resizeObserver = new ResizeObserver(() => chart.resize())
    resizeObserver.observe(element)

    chart.on("datazoom", rawEvent => {
      const event = rawEvent as DataZoomEvent
      const payload = Array.isArray(event.batch) ? event.batch[0] : event
      const start = Number(payload.start ?? 0)
      const end = Number(payload.end ?? 100)
      const extent = extentRef.current
      const duration = extent.to - extent.from

      zoomRef.current = {
        automatic: false,
        followsLatest: end >= 99.5,
        from: extent.from + (duration * start) / 100,
        to: extent.from + (duration * end) / 100,
      }
    })

    return () => {
      resizeObserver.disconnect()
      chart.dispose()
      chartRef.current = undefined
    }
  }, [])

  useEffect(() => {
    const chart = chartRef.current
    const element = elementRef.current
    const firstPoint = points[0]
    const lastPoint = points.at(-1)
    if (!chart || !element || !firstPoint || !lastPoint) return

    const renderKey = `${theme}:${firstPoint.timestamp}:${lastPoint.timestamp}:${points.length}`
    if (renderKey === renderKeyRef.current) return
    renderKeyRef.current = renderKey

    const dataFrom = firstPoint.timestamp * 1000
    const to = lastPoint.timestamp * 1000
    const from = Math.min(dataFrom, to - DEFAULT_VISIBLE_SECONDS * 1000)
    extentRef.current = {from, to}
    const zoom = resolveZoomRange(zoomRef.current, from, to)
    zoomRef.current = zoom
    const computed = getComputedStyle(element)
    const color = (name: string) => computed.getPropertyValue(name).trim()

    chart.setOption({
      animation: false,
      aria: {
        enabled: true,
        description: "Transaction throughput over time",
      },
      dataZoom: [
        {
          endValue: zoom.to,
          filterMode: "filter",
          moveOnMouseMove: true,
          moveOnMouseWheel: "shift",
          startValue: zoom.from,
          throttle: 16,
          type: "inside",
          zoomOnMouseWheel: "ctrl",
        },
        {
          backgroundColor: "transparent",
          borderColor: color("--acton-color-border"),
          bottom: 2,
          brushSelect: false,
          dataBackground: {
            areaStyle: {color: color("--tps-bar-color")},
            lineStyle: {color: color("--tps-bar-active-color")},
          },
          endValue: zoom.to,
          fillerColor: color("--tps-selection-color"),
          handleSize: "80%",
          handleStyle: {
            borderColor: color("--tps-bar-active-color"),
            color: color("--acton-color-surface-raised"),
          },
          height: 24,
          moveHandleStyle: {color: color("--tps-bar-active-color")},
          selectedDataBackground: {
            areaStyle: {color: color("--tps-bar-color")},
            lineStyle: {color: color("--tps-bar-active-color")},
          },
          showDetail: false,
          startValue: zoom.from,
          textStyle: {color: color("--acton-color-text-subtle"), fontSize: 10},
          type: "slider",
        },
      ],
      grid: {bottom: 48, containLabel: false, left: 50, right: 0, top: 10},
      series: [
        {
          barCategoryGap: "8%",
          barMaxWidth: 24,
          data: points.map(point => [point.timestamp * 1000, point.tps, point.transactions]),
          itemStyle: {color: color("--tps-bar-color")},
          markLine: {
            animation: false,
            data: [{type: "average"}],
            label: {
              color: color("--tps-reference-color"),
              formatter: ({value}: {value?: number | string}) => formatTps(Number(value ?? 0)),
              fontSize: 11,
              fontWeight: 600,
              position: "start",
            },
            lineStyle: {color: color("--tps-reference-color"), type: "dashed"},
            silent: true,
            symbol: ["none", "none"],
          },
          name: "Throughput",
          type: "bar",
        },
      ],
      tooltip: {
        appendToBody: false,
        axisPointer: {type: "shadow"},
        backgroundColor: "transparent",
        borderWidth: 0,
        confine: true,
        extraCssText: "box-shadow: none",
        formatter: (params: unknown) => formatChartTooltip(params),
        padding: 0,
        trigger: "axis",
      },
      xAxis: {
        axisLabel: {
          color: color("--acton-color-text-subtle"),
          fontSize: 11,
          formatter: (value: number) => formatTime(value / 1000),
          hideOverlap: true,
        },
        axisLine: {show: false},
        axisTick: {show: false},
        max: to,
        min: from,
        minInterval: 5000,
        splitLine: {
          lineStyle: {color: color("--acton-color-border"), type: "dashed"},
          show: true,
        },
        type: "time",
      },
      yAxis: {
        axisLabel: {
          color: color("--acton-color-text-subtle"),
          fontSize: 11,
          formatter: (value: number) => formatAxis(value),
        },
        axisLine: {show: false},
        axisTick: {show: false},
        min: 0,
        splitLine: {show: false},
        splitNumber: 2,
        type: "value",
      },
    } satisfies EChartsCoreOption)
  }, [points, theme])

  return (
    <div className={styles.chart}>
      <div
        ref={elementRef}
        className={styles.chartCanvas}
        role="img"
        aria-label="Transaction throughput over time"
      />
    </div>
  )
}

interface ZoomRange {
  readonly automatic: boolean
  readonly followsLatest: boolean
  readonly from: number
  readonly to: number
}

interface DataZoomEvent {
  readonly batch?: readonly DataZoomEvent[]
  readonly end?: number
  readonly start?: number
}

function resolveZoomRange(previous: ZoomRange | undefined, from: number, to: number): ZoomRange {
  if (!previous || previous.automatic) {
    return {
      automatic: true,
      followsLatest: true,
      from: Math.max(from, to - DEFAULT_VISIBLE_SECONDS * 1000),
      to,
    }
  }

  const duration = Math.max(5000, previous.to - previous.from)
  if (previous.followsLatest) {
    return {automatic: false, followsLatest: true, from: Math.max(from, to - duration), to}
  }

  const nextFrom = Math.max(from, previous.from)
  const nextTo = Math.min(to, Math.max(nextFrom + 5000, previous.to))
  return {automatic: false, followsLatest: false, from: nextFrom, to: nextTo}
}

function formatChartTooltip(params: unknown) {
  const item = Array.isArray(params) ? params[0] : params
  if (!item || typeof item !== "object" || !("value" in item) || !Array.isArray(item.value)) {
    return ""
  }

  const timestamp = Number(item.value[0]) / 1000
  const tps = Number(item.value[1])

  return `<div class="${styles.tooltip}">
    <strong class="${styles.tooltipTime}">${formatTooltipTime(timestamp)}</strong>
    <div class="${styles.tooltipValue}">
      <span class="${styles.tooltipDot}"></span>
      <span>Throughput:</span>
      <strong>${formatTps(tps)} TPS</strong>
    </div>
  </div>`
}

function formatTps(value: number) {
  return value.toLocaleString(undefined, {maximumFractionDigits: value < 10 ? 2 : 1})
}

function formatAxis(value: number) {
  return value.toLocaleString(undefined, {maximumFractionDigits: value < 10 ? 1 : 0})
}

function formatTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function formatTooltipTime(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString([], {
    month: "long",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

export default TpsSection
