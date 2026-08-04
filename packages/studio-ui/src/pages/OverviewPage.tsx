import {Skeleton} from "@acton/ui"
import {ArrowRight, Boxes, CircleGauge, FolderKanban, FlaskConical} from "lucide-react"
import type {CSSProperties, MouseEvent, ReactNode} from "react"

import {environmentStatusLabels, formatEnvironmentType} from "../environmentPresentation"
import type {
  EnvironmentStatus,
  StudioConnectionState,
  StudioEnvironment,
  TestRunSummary,
} from "../studioApi"
import type {StudioPath} from "../studioPages"
import {
  formatTestRunDuration,
  testRunStatusLabel,
  testRunSummary,
  testRunTime,
} from "../testRunPresentation"

import styles from "./OverviewPage.module.css"

interface OverviewPageProps {
  readonly connectionState: StudioConnectionState
  readonly environments: readonly StudioEnvironment[]
  readonly environmentsError?: string
  readonly environmentsLoading: boolean
  readonly projectName?: string
  readonly projectPath?: string
  readonly testRuns: readonly TestRunSummary[]
  readonly testRunsError?: string
  readonly testRunsLoading: boolean
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment: (environment: StudioEnvironment) => void
  readonly onSelectTestRun: (runId: string) => void
}

const chartRunCount = 10
const recentRunCount = 4
const visibleEnvironmentCount = 4
const environmentStatuses: readonly EnvironmentStatus[] = [
  "running",
  "starting",
  "stopping",
  "stopped",
  "failed",
]

export function OverviewPage({
  connectionState,
  environments,
  environmentsError,
  environmentsLoading,
  projectName,
  projectPath,
  testRuns,
  testRunsError,
  testRunsLoading,
  onNavigate,
  onOpenEnvironment,
  onSelectTestRun,
}: OverviewPageProps) {
  const connectionLabel =
    connectionState === "connected"
      ? "Connected"
      : connectionState === "connecting"
        ? "Connecting"
        : "Not connected"
  const connectionDescription =
    connectionState === "connected"
      ? "Studio server is available"
      : connectionState === "connecting"
        ? "Looking for Studio server"
        : "Start Studio with acton studio start"
  const connectionDotClass =
    connectionState === "connected"
      ? styles.statusDotConnected
      : connectionState === "disconnected"
        ? styles.statusDotDisconnected
        : ""
  const workspaceDescription =
    projectPath ??
    (connectionState === "connected"
      ? projectName
        ? "Current project"
        : "No project selected"
      : connectionState === "connecting"
        ? "Connecting to Studio server"
        : "Waiting for Studio server")
  const runningEnvironmentCount = environments.filter(
    environment => environment.status === "running",
  ).length
  const chartRuns = testRuns.slice(0, chartRunCount).reverse()
  const maxTestsInRun = Math.max(1, ...chartRuns.map(run => run.stats.total))
  const completedRuns = testRuns.filter(run => run.status !== "queued" && run.status !== "running")
  const totalTests = completedRuns.reduce((total, run) => total + run.stats.total, 0)
  const passedTests = completedRuns.reduce((total, run) => total + run.stats.passed, 0)
  const passRate = totalTests === 0 ? undefined : Math.round((passedTests / totalTests) * 100)

  const navigateFromAnchor = (event: MouseEvent<HTMLAnchorElement>, path: StudioPath) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }

    event.preventDefault()
    onNavigate(path)
  }

  return (
    <div className={styles.page}>
      <section className={styles.signalStrip} aria-label="Workspace status">
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <FolderKanban size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong>{projectName || "No project open"}</strong>
            <small className={projectPath ? styles.technicalValue : undefined}>
              {workspaceDescription}
            </small>
          </span>
        </div>
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <CircleGauge size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong className={styles.signalValue}>
              <span className={`${styles.statusDot} ${connectionDotClass}`} />
              {connectionLabel}
            </strong>
            <small>{connectionDescription}</small>
          </span>
        </div>
        <div className={styles.signal}>
          <span className={styles.signalIcon}>
            <Boxes size={17} aria-hidden="true" />
          </span>
          <span className={styles.signalCopy}>
            <strong>
              {environmentsLoading
                ? "Loading"
                : `${runningEnvironmentCount} / ${environments.length}`}
            </strong>
            <small>{environmentsLoading ? "Loading environments" : "Running environments"}</small>
          </span>
        </div>
      </section>

      <div className={styles.dashboardGrid}>
        <section className={styles.panel} aria-labelledby="test-activity-title">
          <div className={styles.sectionHeader}>
            <div>
              <h2 id="test-activity-title">Test activity</h2>
              <p>Results from the latest workspace runs</p>
            </div>
            <a href="/tests" onClick={event => navigateFromAnchor(event, "/tests")}>
              View tests
              <ArrowRight size={15} aria-hidden="true" />
            </a>
          </div>

          {testRunsLoading ? (
            <LoadingChart />
          ) : testRunsError && testRuns.length === 0 ? (
            <PanelMessage title="Could not load test runs" description={testRunsError} />
          ) : testRuns.length === 0 ? (
            <PanelMessage
              icon={<FlaskConical size={20} aria-hidden="true" />}
              title="No test runs yet"
              description="Run project tests to see results and duration trends"
            />
          ) : (
            <>
              <div className={styles.testSummary}>
                <div>
                  <strong>{passRate === undefined ? "—" : `${passRate}%`}</strong>
                  <span>Pass rate</span>
                </div>
                <div>
                  <strong>{totalTests.toLocaleString()}</strong>
                  <span>Test executions</span>
                </div>
                <div>
                  <strong>{testRuns.length.toLocaleString()}</strong>
                  <span>Runs recorded</span>
                </div>
              </div>

              <div className={styles.chartArea}>
                <div className={styles.chartScale} aria-hidden="true">
                  <span>{maxTestsInRun}</span>
                  <span>0</span>
                </div>
                <ol className={styles.runChart} aria-label="Tests in recent runs">
                  {chartRuns.map(run => (
                    <li key={run.id} className={styles.runChartItem}>
                      <button
                        type="button"
                        className={styles.runBarButton}
                        aria-label={`${testRunTime(run.startedAt)}, ${testRunSummary(run)}`}
                        title={`${testRunTime(run.startedAt)} · ${testRunSummary(run)}`}
                        onClick={() => onSelectTestRun(run.id)}
                      >
                        <span
                          className={styles.runBar}
                          data-status={run.status}
                          style={
                            {
                              "--run-height": `${Math.max(6, (run.stats.total / maxTestsInRun) * 100)}%`,
                            } as CSSProperties
                          }
                        >
                          {run.stats.total > 0 ? (
                            <>
                              <span
                                className={styles.runBarPassed}
                                style={{flexGrow: run.stats.passed}}
                              />
                              <span
                                className={styles.runBarFailed}
                                style={{flexGrow: run.stats.failed}}
                              />
                              <span
                                className={styles.runBarSkipped}
                                style={{flexGrow: run.stats.skipped + run.stats.todo}}
                              />
                            </>
                          ) : (
                            <span className={styles.runBarEmpty} />
                          )}
                        </span>
                      </button>
                    </li>
                  ))}
                </ol>
              </div>

              <div className={styles.chartLegend} aria-label="Chart legend">
                <span data-tone="passed">Passed</span>
                <span data-tone="failed">Failed</span>
                <span data-tone="skipped">Skipped or todo</span>
              </div>

              <div className={styles.recentRuns}>
                {testRuns.slice(0, recentRunCount).map(run => (
                  <button
                    key={run.id}
                    type="button"
                    className={styles.runRow}
                    onClick={() => onSelectTestRun(run.id)}
                  >
                    <span className={styles.runStatus} data-status={run.status}>
                      {testRunStatusLabel(run.status)}
                    </span>
                    <span className={styles.runDescription}>{testRunSummary(run)}</span>
                    <span className={styles.runDuration}>
                      {run.stats.durationMs > 0 ? formatTestRunDuration(run.stats.durationMs) : "—"}
                    </span>
                    <span className={styles.runTime}>{testRunTime(run.startedAt)}</span>
                    <ArrowRight size={15} aria-hidden="true" />
                  </button>
                ))}
              </div>
            </>
          )}
        </section>

        <section className={styles.panel} aria-labelledby="environments-title">
          <div className={styles.sectionHeader}>
            <div>
              <h2 id="environments-title">Environments</h2>
              <p>Workspace network status</p>
            </div>
            <a
              href="/virtual-environments"
              onClick={event => navigateFromAnchor(event, "/virtual-environments")}
            >
              View all
              <ArrowRight size={15} aria-hidden="true" />
            </a>
          </div>

          {environmentsLoading ? (
            <LoadingEnvironments />
          ) : environmentsError && environments.length === 0 ? (
            <PanelMessage title="Could not load environments" description={environmentsError} />
          ) : environments.length === 0 ? (
            <PanelMessage
              icon={<Boxes size={20} aria-hidden="true" />}
              title="No virtual environments"
              description="Create a simulated or full localnet for this workspace"
            />
          ) : (
            <>
              <div className={styles.environmentSummary}>
                <strong>
                  {runningEnvironmentCount}
                  <span> / {environments.length}</span>
                </strong>
                <small>Running now</small>
                <div
                  className={styles.environmentRail}
                  aria-label="Environment status distribution"
                >
                  {environmentStatuses.map(status => {
                    const count = environments.filter(
                      environment => environment.status === status,
                    ).length
                    if (count === 0) return null
                    return (
                      <span
                        key={status}
                        data-status={status}
                        style={{flexGrow: count}}
                        title={`${environmentStatusLabels[status]}: ${count}`}
                      />
                    )
                  })}
                </div>
                <div className={styles.environmentLegend}>
                  {environmentStatuses.map(status => {
                    const count = environments.filter(
                      environment => environment.status === status,
                    ).length
                    if (count === 0) return null
                    return (
                      <span key={status} data-status={status}>
                        {environmentStatusLabels[status]} {count}
                      </span>
                    )
                  })}
                </div>
              </div>

              <div className={styles.environmentList}>
                {environments.slice(0, visibleEnvironmentCount).map(environment => (
                  <button
                    key={environment.id}
                    type="button"
                    className={styles.environmentRow}
                    onClick={() => onOpenEnvironment(environment)}
                  >
                    <span
                      className={styles.environmentStatusDot}
                      data-status={environment.status}
                    />
                    <span className={styles.environmentCopy}>
                      <strong>{environment.name}</strong>
                      <small>{formatEnvironmentMetadata(environment)}</small>
                    </span>
                    <span
                      className={styles.environmentStatusLabel}
                      data-status={environment.status}
                    >
                      {environmentStatusLabels[environment.status]}
                    </span>
                    <ArrowRight size={15} aria-hidden="true" />
                  </button>
                ))}
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  )
}

function LoadingChart() {
  return (
    <div className={styles.loadingPanel} role="status" aria-label="Loading test activity">
      <div className={styles.loadingMetrics}>
        <Skeleton width="5rem" height="2rem" />
        <Skeleton width="5rem" height="2rem" />
        <Skeleton width="5rem" height="2rem" />
      </div>
      <Skeleton shape="rect" width="100%" height="10rem" radius="md" />
      <Skeleton width="100%" height="3rem" />
      <Skeleton width="100%" height="3rem" />
    </div>
  )
}

function LoadingEnvironments() {
  return (
    <div className={styles.loadingPanel} role="status" aria-label="Loading environments">
      <Skeleton width="7rem" height="2rem" />
      <Skeleton shape="rect" width="100%" height="0.5rem" radius="round" />
      <Skeleton width="100%" height="3.5rem" />
      <Skeleton width="100%" height="3.5rem" />
      <Skeleton width="100%" height="3.5rem" />
    </div>
  )
}

function PanelMessage({
  description,
  icon,
  title,
}: {
  readonly description: string
  readonly icon?: ReactNode
  readonly title: string
}) {
  return (
    <div className={styles.panelMessage}>
      {icon ? <span className={styles.panelMessageIcon}>{icon}</span> : null}
      <strong>{title}</strong>
      <p>{description}</p>
    </div>
  )
}

function formatEnvironmentMetadata(environment: StudioEnvironment) {
  const type = formatEnvironmentType(environment.config)
  return type === environment.network.label ? type : `${type} · ${environment.network.label}`
}
