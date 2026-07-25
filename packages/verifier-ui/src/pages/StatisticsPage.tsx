import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  Skeleton,
} from "@acton/ui"
import {CircleAlert, RefreshCw} from "lucide-react"
import {useCallback, useEffect, useMemo, useState} from "react"
import {Pie, PieChart, ResponsiveContainer, Tooltip} from "recharts"

import type {VerificationStatisticsResponse, VerifierApi} from "../lib/api"
import styles from "./StatisticsPage.module.css"

interface StatisticsPageProps {
  readonly api: VerifierApi
}

interface LanguageStatistics {
  readonly fill: string
  readonly label: string
  readonly language: string
  readonly total: number
  readonly versions: readonly VersionRow[]
}

interface VersionRow {
  readonly total: number
  readonly version: string
}

const LANGUAGE_COLORS: Readonly<Record<string, string>> = {
  func: "var(--acton-color-warning)",
  tact: "var(--acton-color-accent)",
  tolk: "var(--acton-color-success)",
}
const LANGUAGE_LABELS: Readonly<Record<string, string>> = {
  func: "FunC",
  tact: "Tact",
  tolk: "Tolk",
}
const FALLBACK_LANGUAGE_COLOR = "var(--acton-color-text-subtle)"
const LEGEND_SKELETON_KEYS = ["first", "second", "third"] as const

function normalizedCount(value: number): number {
  return Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
}

function languageLabel(language: string): string {
  const value = language.trim()
  return (
    LANGUAGE_LABELS[value.toLowerCase()] ??
    (value ? value[0].toUpperCase() + value.slice(1) : "Unknown")
  )
}

function languageColor(language: string): string {
  return LANGUAGE_COLORS[language.trim().toLowerCase()] ?? FALLBACK_LANGUAGE_COLOR
}

function formatCount(value: number): string {
  return normalizedCount(value).toLocaleString()
}

function formatShare(value: number, total: number): string {
  if (total <= 0) return "0%"
  const share = (normalizedCount(value) / total) * 100
  return `${share >= 10 ? share.toFixed(0) : share.toFixed(1)}%`
}

export function StatisticsPage({api}: StatisticsPageProps) {
  const [statistics, setStatistics] = useState<VerificationStatisticsResponse>()
  const [error, setError] = useState<string>()
  const [isLoading, setIsLoading] = useState(true)

  const loadStatistics = useCallback(() => {
    setIsLoading(true)
    setError(undefined)

    api
      .fetchStatistics()
      .then(response => {
        setStatistics(response)
      })
      .catch(error => {
        setStatistics(undefined)
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
  const languages = useMemo<readonly LanguageStatistics[]>(
    () =>
      [...(statistics?.languages ?? [])]
        .map(language => {
          const languageTotal = normalizedCount(language.total)
          return {
            fill: languageColor(language.language),
            label: languageLabel(language.language),
            language: language.language,
            total: languageTotal,
            versions: [...language.versions]
              .sort((left, right) => normalizedCount(right.total) - normalizedCount(left.total))
              .map(version => ({
                total: normalizedCount(version.total),
                version: version.version || "Unknown",
              })),
          }
        })
        .sort((left, right) => right.total - left.total),
    [statistics],
  )
  const versionCount = languages.reduce((count, language) => count + language.versions.length, 0)
  const compilerLanguages = [...languages].sort(
    (left, right) =>
      Number(right.language.toLowerCase() === "tolk") -
      Number(left.language.toLowerCase() === "tolk"),
  )

  return (
    <section className={styles.container}>
      <header className={styles.hero}>
        <h1 className={styles.title}>Verification statistics</h1>
        <p className={styles.subtitle}>
          Language and compiler coverage across indexed verified contracts.
        </p>
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
          <section
            className={styles.summary}
            aria-busy={isLoading}
            aria-label={isLoading ? "Loading verification statistics" : undefined}
          >
            <div className={styles.totalPanel}>
              <span className={styles.summaryLabel}>Verified contracts</span>
              {isLoading ? (
                <Skeleton width="11rem" height="4.5rem" radius="md" />
              ) : (
                <strong className={styles.totalValue}>{formatCount(total)}</strong>
              )}
              <span className={styles.totalCaption}>
                {isLoading
                  ? "Reading the verifier registry"
                  : `Across ${languages.length.toLocaleString()} compiler languages`}
              </span>
            </div>

            <div className={styles.distributionPanel}>
              <div className={styles.sectionHeading}>
                <div>
                  <span className={styles.summaryLabel}>Language distribution</span>
                  <h2>Verified source by language</h2>
                </div>
              </div>

              <div className={styles.distributionContent}>
                <div className={styles.chart}>
                  {isLoading ? (
                    <Skeleton shape="circle" width="10rem" height="10rem" />
                  ) : languages.length > 0 ? (
                    <>
                      <ResponsiveContainer width="100%" height="100%">
                        <PieChart accessibilityLayer>
                          <Pie
                            data={languages}
                            dataKey="total"
                            nameKey="label"
                            innerRadius="66%"
                            outerRadius="94%"
                            paddingAngle={2}
                            stroke="none"
                            isAnimationActive={false}
                          />
                          <Tooltip
                            contentStyle={{
                              background: "var(--acton-color-surface-raised)",
                              border: "1px solid var(--acton-color-border)",
                              borderRadius: "10px",
                              color: "var(--acton-color-text)",
                            }}
                            itemStyle={{color: "var(--acton-color-text)"}}
                            formatter={value => [
                              formatCount(typeof value === "number" ? value : Number(value)),
                              "Contracts",
                            ]}
                          />
                        </PieChart>
                      </ResponsiveContainer>
                      <div className={styles.chartCenter} aria-hidden="true">
                        <strong>{formatCount(total)}</strong>
                        <span>Total</span>
                      </div>
                    </>
                  ) : (
                    <span className={styles.emptyChart}>No data</span>
                  )}
                </div>

                <div className={styles.languageLegend}>
                  {isLoading
                    ? LEGEND_SKELETON_KEYS.map(key => (
                        <div className={styles.legendSkeleton} key={key}>
                          <Skeleton width="8rem" />
                          <Skeleton width="4rem" />
                        </div>
                      ))
                    : languages.map(language => (
                        <div className={styles.legendRow} key={language.language}>
                          <div className={styles.legendIdentity}>
                            <span
                              className={styles.legendDot}
                              style={{backgroundColor: language.fill}}
                              aria-hidden="true"
                            />
                            <span>{language.label}</span>
                          </div>
                          <div className={styles.legendValue}>
                            <strong>{formatCount(language.total)}</strong>
                            <span>{formatShare(language.total, total)}</span>
                          </div>
                        </div>
                      ))}
                </div>
              </div>
            </div>
          </section>

          <section className={styles.compilerSection}>
            <header className={styles.compilerHeading}>
              <h2>Versions by language</h2>
              <span>{isLoading ? "Loading" : `${versionCount.toLocaleString()} versions`}</span>
            </header>

            {isLoading ? (
              <DataTable title="Compiler versions" meta="Loading" minWidth="36rem">
                <DataTableTable aria-label="Loading compiler statistics">
                  <DataTableHead>
                    <DataTableRow>
                      <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Language share</DataTableHeaderCell>
                      <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                    </DataTableRow>
                  </DataTableHead>
                  <DataTableBody>
                    <DataTableSkeletonRows
                      columns={4}
                      rows={6}
                      widths={["8rem", "4rem", "4rem", "4rem"]}
                      alignments={["left", "right", "right", "right"]}
                    />
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : languages.length === 0 ? (
              <DataTable title="Compiler versions" minWidth="36rem">
                <DataTableTable aria-label="Verified contracts by compiler version">
                  <DataTableBody>
                    <DataTableEmpty colSpan={4}>No compiler statistics indexed yet</DataTableEmpty>
                  </DataTableBody>
                </DataTableTable>
              </DataTable>
            ) : (
              <div className={styles.versionTables}>
                {compilerLanguages.map(language => (
                  <DataTable
                    key={language.language}
                    title={
                      <span className={styles.languageTitle}>
                        <span
                          className={styles.legendDot}
                          style={{backgroundColor: language.fill}}
                          aria-hidden="true"
                        />
                        {language.label}
                      </span>
                    }
                    meta={`${formatCount(language.total)} contracts · ${language.versions.length.toLocaleString()} versions`}
                    minWidth="36rem"
                  >
                    <DataTableTable
                      aria-label={`${language.label} verified contracts by compiler version`}
                    >
                      <DataTableHead>
                        <DataTableRow>
                          <DataTableHeaderCell columnWidth="40%">Version</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Contracts</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Language share</DataTableHeaderCell>
                          <DataTableHeaderCell align="right">Registry share</DataTableHeaderCell>
                        </DataTableRow>
                      </DataTableHead>
                      <DataTableBody>
                        {language.versions.length === 0 ? (
                          <DataTableEmpty colSpan={4}>No versions indexed</DataTableEmpty>
                        ) : (
                          language.versions.map(row => (
                            <DataTableRow key={`${language.language}:${row.version}`} hover>
                              <DataTableCell mono>{row.version}</DataTableCell>
                              <DataTableCell align="right" tone="strong">
                                {formatCount(row.total)}
                              </DataTableCell>
                              <DataTableCell align="right" tone="muted">
                                {formatShare(row.total, language.total)}
                              </DataTableCell>
                              <DataTableCell align="right" tone="muted">
                                {formatShare(row.total, total)}
                              </DataTableCell>
                            </DataTableRow>
                          ))
                        )}
                      </DataTableBody>
                    </DataTableTable>
                  </DataTable>
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </section>
  )
}
