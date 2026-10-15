import {
  Button,
  CountValue,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  formatDateTime,
  formatNumberValue,
  NumberValue,
  Percentage,
  Skeleton,
} from "@acton/ui"
import {CircleAlert, RefreshCw} from "lucide-react"
import {useCallback, useEffect, useMemo, useState} from "react"
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts"

import type {
  VerificationStatisticsHistoryItem,
  VerificationStatisticsHistoryResponse,
  VerificationStatisticsResponse,
  VerifierApi,
} from "../lib/api"
import styles from "./StatisticsPage.module.css"

interface StatisticsPageProps {
  readonly api: VerifierApi
}

interface VersionRow {
  readonly total: number
  readonly version: string
}

interface HistorySeries {
  readonly cumulative: readonly HistoryPoint[]
  readonly monthly: readonly HistoryPoint[]
}

interface HistoryPoint {
  timestamp: number
  total: number
}

const HISTORY_SERIES_COLOR = "var(--acton-color-accent)"
const CHART_TOOLTIP_STYLE = {
  background: "var(--acton-color-surface-raised)",
  border: "1px solid var(--acton-color-border)",
  borderRadius: "10px",
  color: "var(--acton-color-text)",
}
const MONTH_TICK_FORMAT = {display: "month-short", locale: "en", timeZone: "UTC"} as const
const MONTH_TOOLTIP_FORMAT = {display: "month-long", locale: "en", timeZone: "UTC"} as const

function normalizedCount(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
}

function monthStart(timestamp: number): number {
  const date = new Date(timestamp * 1000)
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1)
}

function nextMonth(timestamp: number): number {
  const date = new Date(timestamp)
  return Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 1)
}

function buildHistorySeries(items: readonly VerificationStatisticsHistoryItem[]): HistorySeries {
  const monthlyCounts = new Map<number, number>()

  for (const item of items) {
    if (!Number.isFinite(item.timestamp) || item.timestamp < 0) continue

    const month = monthStart(item.timestamp)
    monthlyCounts.set(month, (monthlyCounts.get(month) ?? 0) + 1)
  }

  const months = [...monthlyCounts.keys()].sort((left, right) => left - right)
  const [firstMonth] = months
  const lastMonth = months.at(-1)
  if (firstMonth === undefined || lastMonth === undefined) {
    return {cumulative: [], monthly: []}
  }

  const cumulative: HistoryPoint[] = []
  const monthly: HistoryPoint[] = []
  let runningTotal = 0

  for (let timestamp = firstMonth; timestamp <= lastMonth; timestamp = nextMonth(timestamp)) {
    const total = monthlyCounts.get(timestamp) ?? 0
    runningTotal += total
    monthly.push({timestamp, total})
    cumulative.push({timestamp, total: runningTotal})
  }

  return {cumulative, monthly}
}

export function StatisticsPage({api}: StatisticsPageProps) {
  const [statistics, setStatistics] = useState<VerificationStatisticsResponse>()
  const [history, setHistory] = useState<VerificationStatisticsHistoryResponse>()
  const [error, setError] = useState<string>()
  const [isLoading, setIsLoading] = useState(true)

  const loadStatistics = useCallback(() => {
    setIsLoading(true)
    setError(undefined)

    Promise.all([api.fetchStatistics(), api.fetchStatisticsHistory()])
      .then(([statisticsResponse, historyResponse]) => {
        setStatistics(statisticsResponse)
        setHistory(historyResponse)
      })
      .catch(error => {
        setStatistics(undefined)
        setHistory(undefined)
        setError(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        setIsLoading(false)
      })
  }, [api])

  useEffect(() => {
    loadStatistics()
  }, [loadStatistics])

  const total = normalizedCount(statistics?.total ?? 0)
  const tolkStatistics = statistics?.languages.find(
    language => language.language.trim().toLowerCase() === "tolk",
  )
  const versions: readonly VersionRow[] = [...(tolkStatistics?.versions ?? [])]
    .sort((left, right) => normalizedCount(right.total) - normalizedCount(left.total))
    .map(version => ({
      total: normalizedCount(version.total),
      version: version.version || "Unknown",
    }))
  const historySeries = useMemo(() => buildHistorySeries(history?.items ?? []), [history])

  return (
    <section className={styles.container}>
      <header className={styles.hero}>
        <div>
          <h1 className={styles.title}>Verification statistics</h1>
          <p className={styles.subtitle}>Registry growth and compiler version coverage</p>
        </div>
        {!error && (
          <div className={styles.heroTotal} aria-busy={isLoading}>
            <span className={styles.summaryLabel}>Verified contracts</span>
            {isLoading ? (
              <Skeleton width="8rem" height="3rem" radius="md" />
            ) : (
              <strong className={styles.totalValue}>
                <NumberValue value={total} />
              </strong>
            )}
          </div>
        )}
      </header>

      {error ? (
        <section className={styles.errorPanel} role="alert">
          <CircleAlert size={22} aria-hidden="true" />
          <div className={styles.errorCopy}>
            <strong>Statistics are unavailable</strong>
            <span>{error}</span>
          </div>
          <Button
            size="sm"
            variant="outline"
            leadingIcon={<RefreshCw size={15} />}
            onClick={loadStatistics}
          >
            Retry
          </Button>
        </section>
      ) : (
        <>
          <section className={styles.historySection}>
            <header className={styles.historyHeading}>
              <div>
                <h2>Verification history</h2>
                <p>Registry growth and monthly verification activity.</p>
              </div>
            </header>

            <div className={styles.historyGrid} aria-busy={isLoading}>
              <article className={styles.historyPanel}>
                <header className={styles.historyPanelHeading}>
                  <span className={styles.summaryLabel}>Registry growth</span>
                  <h3>Verified contracts over time</h3>
                </header>
                <div className={styles.historyChart}>
                  {isLoading ? (
                    <Skeleton width="100%" height="100%" radius="md" />
                  ) : historySeries.cumulative.length > 0 ? (
                    <ResponsiveContainer width="100%" height="100%">
                      <AreaChart
                        accessibilityLayer
                        data={historySeries.cumulative}
                        margin={{top: 10, right: 8, bottom: 0, left: 0}}
                      >
                        <CartesianGrid
                          vertical={false}
                          stroke="var(--acton-color-border)"
                          strokeDasharray="3 3"
                        />
                        <XAxis
                          axisLine={false}
                          dataKey="timestamp"
                          minTickGap={32}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={timestamp => formatDateTime(timestamp, MONTH_TICK_FORMAT)}
                          tickLine={false}
                        />
                        <YAxis
                          allowDecimals={false}
                          axisLine={false}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={value => formatNumberValue(value)}
                          tickLine={false}
                          width={46}
                        />
                        <Tooltip
                          contentStyle={CHART_TOOLTIP_STYLE}
                          itemStyle={{color: "var(--acton-color-text)"}}
                          labelFormatter={value =>
                            formatDateTime(Number(value), MONTH_TOOLTIP_FORMAT)
                          }
                          formatter={(value, name) => [
                            formatNumberValue(Array.isArray(value) ? value[0] : value),
                            name,
                          ]}
                        />
                        <Area
                          dataKey="total"
                          fill={HISTORY_SERIES_COLOR}
                          fillOpacity={0.16}
                          isAnimationActive={false}
                          name="Verified contracts"
                          stroke={HISTORY_SERIES_COLOR}
                          strokeWidth={2}
                          type="monotone"
                        />
                      </AreaChart>
                    </ResponsiveContainer>
                  ) : (
                    <span className={styles.historyEmpty}>No verification history indexed yet</span>
                  )}
                </div>
              </article>

              <article className={styles.historyPanel}>
                <header className={styles.historyPanelHeading}>
                  <span className={styles.summaryLabel}>Monthly activity</span>
                  <h3>Verifications by month</h3>
                </header>
                <div className={styles.historyChart}>
                  {isLoading ? (
                    <Skeleton width="100%" height="100%" radius="md" />
                  ) : historySeries.monthly.length > 0 ? (
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart
                        accessibilityLayer
                        data={historySeries.monthly}
                        margin={{top: 10, right: 8, bottom: 0, left: 0}}
                      >
                        <CartesianGrid
                          vertical={false}
                          stroke="var(--acton-color-border)"
                          strokeDasharray="3 3"
                        />
                        <XAxis
                          axisLine={false}
                          dataKey="timestamp"
                          minTickGap={32}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={timestamp => formatDateTime(timestamp, MONTH_TICK_FORMAT)}
                          tickLine={false}
                        />
                        <YAxis
                          allowDecimals={false}
                          axisLine={false}
                          tick={{fill: "var(--acton-color-text-muted)", fontSize: 12}}
                          tickFormatter={value => formatNumberValue(value)}
                          tickLine={false}
                          width={46}
                        />
                        <Tooltip
                          contentStyle={CHART_TOOLTIP_STYLE}
                          cursor={{fill: "var(--acton-color-surface-hover)"}}
                          itemStyle={{color: "var(--acton-color-text)"}}
                          labelFormatter={value =>
                            formatDateTime(Number(value), MONTH_TOOLTIP_FORMAT)
                          }
                          formatter={(value, name) => [
                            formatNumberValue(Array.isArray(value) ? value[0] : value),
                            name,
                          ]}
                        />
                        <Bar
                          dataKey="total"
                          fill={HISTORY_SERIES_COLOR}
                          isAnimationActive={false}
                          name="Verifications"
                        />
                      </BarChart>
                    </ResponsiveContainer>
                  ) : (
                    <span className={styles.historyEmpty}>No verification history indexed yet</span>
                  )}
                </div>
              </article>
            </div>
          </section>

          <section className={styles.compilerSection}>
            <header className={styles.compilerHeading}>
              <h2>Tolk compiler versions</h2>
              <span>
                {isLoading ? "Loading" : <CountValue singular="version" value={versions.length} />}
              </span>
            </header>

            {isLoading ? (
              <DataTable title="Tolk" meta="Loading" minWidth="36rem">
                <DataTableTable aria-label="Loading Tolk compiler statistics">
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    <DataTableSkeletonRows
                      columns={3}
                      rows={6}
                      widths={["8rem", "4rem", "4rem"]}
                      alignments={["left", "right", "right"]}
                    />
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : tolkStatistics ? (
              <DataTable
                title="Tolk"
                meta={
                  <>
                    <CountValue singular="contract" value={normalizedCount(tolkStatistics.total)} />{" "}
                    · <CountValue singular="version" value={versions.length} />
                  </>
                }
                minWidth="36rem"
              >
                <DataTableTable aria-label="Tolk verified contracts by compiler version">
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    {versions.length === 0 ? (
                      <DataTableEmpty colSpan={3}>No versions indexed</DataTableEmpty>
                    ) : (
                      versions.map(row => (
                        <DataTableRow key={row.version} hover>
                          <DataTableCell mono>{row.version}</DataTableCell>
                          <DataTableCell align="right" tone="strong">
                            <NumberValue value={row.total} />
                          </DataTableCell>
                          <DataTableCell align="right" tone="muted">
                            <Percentage maximumFractionDigits={1} total={total} value={row.total} />
                          </DataTableCell>
                        </DataTableRow>
                      ))
                    )}
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : (
              <DataTable title="Tolk" minWidth="36rem">
                <DataTableTable aria-label="Tolk verified contracts by compiler version">
                  <DataTableBody>
                    <DataTableEmpty colSpan={3}>
                      No Tolk compiler statistics indexed yet
                    </DataTableEmpty>
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            )}
          </section>
        </>
      )}
    </section>
  )
}
