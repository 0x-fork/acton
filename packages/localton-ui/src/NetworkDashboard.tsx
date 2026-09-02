import {lazy, Suspense, useEffect, useState} from "react"
import type {ReactNode} from "react"
import {Activity} from "lucide-react"
import {
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  Duration,
  InlineLoader,
  RelativeTime,
  Skeleton,
  TechnicalValue,
} from "@acton/ui"

import {ElectionSection} from "./components/ElectionSection"
import {Metric} from "./components/Metric"
import {NodesSection} from "./components/NodesSection"
import {StatusPill} from "./components/StatusPill"
import type {ObservabilityClient} from "./observability"
import {useObservability} from "./observability"
import styles from "./App.module.css"
import type {NetworkView, NodeView, ShardHead, TpsView} from "./types"

const TpsSection = lazy(() => import("./components/TpsSection"))

export type NetworkDashboardView = "all" | "overview" | "nodes" | "validators"

export interface NetworkDashboardProps {
  readonly client: ObservabilityClient
  /** Host-owned controls rendered after the node table without coupling them to Localton */
  readonly nodesFooter?: ReactNode
  readonly onNetworkChange?: (network: NetworkView) => void
  /** Host-owned row controls; standalone Localton omits this callback */
  readonly renderNodeActions?: (node: NodeView) => ReactNode
  readonly view?: NetworkDashboardView
}

/** Renders collector-backed network pages without owning product navigation or page chrome */
export function NetworkDashboard({
  client,
  nodesFooter,
  onNetworkChange,
  renderNodeActions,
  view = "all",
}: NetworkDashboardProps) {
  const {network, now, tps} = useObservability(client)

  useEffect(() => {
    if (network) onNetworkChange?.(network)
  }, [network, onNetworkChange])

  if (!network) {
    return (
      <div className={styles.embeddedBootState}>
        <InlineLoader
          message="Reading network state"
          subtext="Waiting for the observability service"
        />
      </div>
    )
  }

  return (
    <NetworkDashboardContent
      network={network}
      nodesFooter={nodesFooter}
      now={now}
      renderNodeActions={renderNodeActions}
      tps={tps}
      view={view}
    />
  )
}

interface NetworkDashboardContentProps {
  readonly network: NetworkView
  readonly nodesFooter?: ReactNode
  readonly now: number
  readonly renderNodeActions?: (node: NodeView) => ReactNode
  readonly tps: TpsView | undefined
  readonly view?: NetworkDashboardView
}

/** Presents a supplied snapshot so shells can share polling with their own live status chrome */
export function NetworkDashboardContent({
  network,
  nodesFooter,
  now,
  renderNodeActions,
  tps,
  view = "all",
}: NetworkDashboardContentProps) {
  return (
    <div className={styles.dashboardContent}>
      {view === "all" || view === "overview" ? (
        <>
          <NetworkOverviewSection network={network} showTitle={view === "all"} />
          <DeferredTpsSection series={tps} />
        </>
      ) : null}

      {view === "all" || view === "validators" ? (
        <>
          <ElectionSection election={network.election} now={now} />
          <ValidatorsSection nodes={network.nodes} />
        </>
      ) : null}

      {view === "all" || view === "nodes" ? (
        <>
          <NodesSection
            footer={nodesFooter}
            nodes={network.nodes}
            now={now}
            renderNodeActions={renderNodeActions}
            showLocations={view === "all"}
            showTitle={view === "all"}
          />
          <ObserverDiagnostics network={network} now={now} />
        </>
      ) : null}

      {view === "all" || view === "overview" ? (
        <ShardsSection shards={network.shards} now={now} showTitle />
      ) : null}
    </div>
  )
}

function DeferredTpsSection({series}: {readonly series: TpsView | undefined}) {
  const [ready, setReady] = useState(false)

  useEffect(() => {
    if (typeof globalThis.requestIdleCallback === "function") {
      const request = globalThis.requestIdleCallback(() => setReady(true), {timeout: 800})
      return () => globalThis.cancelIdleCallback(request)
    }

    const request = globalThis.setTimeout(() => setReady(true), 0)
    return () => globalThis.clearTimeout(request)
  }, [])

  if (!ready) return <TpsSkeleton />

  return (
    <Suspense fallback={<TpsSkeleton />}>
      <TpsSection series={series} />
    </Suspense>
  )
}

function TpsSkeleton() {
  return (
    <section className={styles.tpsSkeleton} aria-label="Loading transaction throughput" aria-busy>
      <div className={styles.tpsSkeletonHeading}>
        <h2>Transaction throughput</h2>
      </div>
      <Skeleton shape="rect" width="100%" height="22.5rem" radius="md" />
    </section>
  )
}

function NetworkOverviewSection({
  network,
  showTitle,
}: {
  readonly network: NetworkView
  readonly showTitle: boolean
}) {
  return (
    <section
      id="overview"
      className={styles.sectionStack}
      aria-label={showTitle ? undefined : "Network overview"}
      aria-labelledby={showTitle ? "overview-title" : undefined}
    >
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2 id="overview-title">Network overview</h2>
        </div>
      ) : null}
      <div className={styles.metricStrip}>
        <Metric
          label="Online nodes"
          value={`${network.totals.online_nodes} / ${network.totals.nodes}`}
          tone={network.totals.online_nodes === network.totals.nodes ? "good" : "warning"}
        />
        <Metric
          label="Synchronized"
          value={`${network.totals.synchronized_nodes} / ${network.totals.nodes}`}
          tone={network.totals.synchronized_nodes === network.totals.nodes ? "good" : "warning"}
        />
        <Metric
          label="Active validators"
          value={`${network.totals.active_validators} / ${network.totals.configured_validators}`}
          tone={
            network.totals.active_validators === network.totals.configured_validators
              ? "good"
              : "warning"
          }
        />
        <Metric
          label="Masterchain"
          value={network.chain ? `#${network.chain.seqno.toLocaleString()}` : "Waiting"}
        />
        <Metric label="Current shards" value={String(network.chain?.shard_count ?? 0)} />
      </div>
      {network.chain ? null : (
        <div className={styles.notice}>
          <Activity size={16} aria-hidden="true" />
          <span>Waiting for TON network data</span>
        </div>
      )}
    </section>
  )
}

const VALIDATOR_LABELS: Record<NodeView["validator_status"], string> = {
  not_configured: "Not configured",
  validating: "Validating",
  leaving: "Leaving after round",
  joining: "Joining next set",
  waiting: "Waiting for election",
  inactive: "Not participating",
  unknown: "Set unavailable",
}

function ValidatorLifecycle({state}: {readonly state: NodeView["validator_status"]}) {
  return (
    <span className={styles.validatorState} data-state={state}>
      <span aria-hidden="true" />
      {VALIDATOR_LABELS[state]}
    </span>
  )
}

function ProductionState({node}: {readonly node: NodeView}) {
  const produced = node.produced_masterchain_blocks + node.produced_shard_blocks
  const state = node.active_validator
    ? produced > 0
      ? "producing"
      : "silent"
    : produced > 0
      ? "recent"
      : "inactive"
  const label =
    state === "producing"
      ? "Producing"
      : state === "silent"
        ? "No blocks observed"
        : state === "recent"
          ? "Produced recently"
          : "Not active"

  return (
    <span
      className={styles.productionState}
      data-state={state}
      title={`${node.produced_masterchain_blocks.toLocaleString()} masterchain and ${node.produced_shard_blocks.toLocaleString()} shard blocks in the rolling window`}
    >
      {label}
    </span>
  )
}

function ValidatorsSection({nodes}: {readonly nodes: readonly NodeView[]}) {
  const validators = nodes.filter(node => node.roles.includes("validator"))

  return (
    <section id="validators" className={styles.sectionStack} aria-labelledby="validators-title">
      <div className={styles.sectionHeading}>
        <h2 id="validators-title">Validator production</h2>
      </div>
      <DataTable minWidth="68rem">
        <DataTableTable>
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell>Validator</DataTableHeaderCell>
              <DataTableHeaderCell>Participation</DataTableHeaderCell>
              <DataTableHeaderCell>Production</DataTableHeaderCell>
              <DataTableHeaderCell>Public key</DataTableHeaderCell>
              <DataTableHeaderCell align="right">MC blocks</DataTableHeaderCell>
              <DataTableHeaderCell align="right">Shard blocks</DataTableHeaderCell>
              <DataTableHeaderCell>ADNL</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {validators.length === 0 ? (
              <DataTableEmpty colSpan={7}>No validators have reported yet</DataTableEmpty>
            ) : (
              validators.map(node => (
                <DataTableRow key={`${node.observer_id}:${node.name}`}>
                  <DataTableCell>
                    <strong>{node.name}</strong>
                  </DataTableCell>
                  <DataTableCell>
                    <ValidatorLifecycle state={node.validator_status} />
                  </DataTableCell>
                  <DataTableCell>
                    <ProductionState node={node} />
                  </DataTableCell>
                  <DataTableCell>
                    <TechnicalValue
                      value={node.validator_public_key ?? undefined}
                      copyLabel="validator public key"
                    />
                  </DataTableCell>
                  <DataTableCell align="right">
                    <span className={styles.tabular}>
                      {node.produced_masterchain_blocks.toLocaleString()}
                    </span>
                  </DataTableCell>
                  <DataTableCell align="right">
                    <span className={styles.tabular}>
                      {node.produced_shard_blocks.toLocaleString()}
                    </span>
                  </DataTableCell>
                  <DataTableCell>
                    <TechnicalValue
                      value={node.validator_adnl ?? undefined}
                      copyLabel="validator ADNL"
                    />
                  </DataTableCell>
                </DataTableRow>
              ))
            )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </section>
  )
}

function ShardsSection({
  shards,
  now,
  showTitle,
}: {
  readonly shards: readonly ShardHead[]
  readonly now: number
  readonly showTitle: boolean
}) {
  return (
    <section
      id="shards"
      className={styles.sectionStack}
      aria-label={showTitle ? undefined : "Shard topology"}
      aria-labelledby={showTitle ? "shards-title" : undefined}
    >
      {showTitle ? (
        <div className={styles.sectionHeading}>
          <h2 id="shards-title">Shard topology</h2>
        </div>
      ) : null}
      <DataTable minWidth="62rem">
        <DataTableTable>
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell>Workchain</DataTableHeaderCell>
              <DataTableHeaderCell>Shard</DataTableHeaderCell>
              <DataTableHeaderCell align="right">Seqno</DataTableHeaderCell>
              <DataTableHeaderCell>Block age</DataTableHeaderCell>
              <DataTableHeaderCell>Split or merge</DataTableHeaderCell>
              <DataTableHeaderCell>Root hash</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {shards.length === 0 ? (
              <DataTableEmpty colSpan={6}>No shard frontier is available</DataTableEmpty>
            ) : (
              shards.map(shard => (
                <DataTableRow key={`${shard.workchain}:${shard.shard}`}>
                  <DataTableCell>
                    <span className={styles.tabular}>{shard.workchain}</span>
                  </DataTableCell>
                  <DataTableCell mono>{shard.shard}</DataTableCell>
                  <DataTableCell align="right">
                    <span className={styles.tabular}>{shard.seqno.toLocaleString()}</span>
                  </DataTableCell>
                  <DataTableCell>
                    <Duration value={Math.max(0, now - shard.gen_utime)} display="elapsed" />
                  </DataTableCell>
                  <DataTableCell>
                    {shard.want_split || shard.before_split ? (
                      <span className={styles.topologyChange}>Split pending</span>
                    ) : shard.want_merge || shard.before_merge ? (
                      <span className={styles.topologyChange}>Merge pending</span>
                    ) : (
                      <span className={styles.muted}>Stable</span>
                    )}
                  </DataTableCell>
                  <DataTableCell>
                    <TechnicalValue value={shard.root_hash} copyLabel="shard block root hash" />
                  </DataTableCell>
                </DataTableRow>
              ))
            )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </section>
  )
}

function ObserverDiagnostics({
  network,
  now,
}: {
  readonly network: NetworkView
  readonly now: number
}) {
  return (
    <section className={styles.sectionStack} aria-label="Collector diagnostics">
      <div className={styles.sectionHeading}>
        <h2>Collector diagnostics</h2>
      </div>
      <DataTable minWidth="44rem">
        <DataTableTable>
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell>Status</DataTableHeaderCell>
              <DataTableHeaderCell>Observer</DataTableHeaderCell>
              <DataTableHeaderCell>Endpoint</DataTableHeaderCell>
              <DataTableHeaderCell>Last report</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {network.observers.map(observer => (
              <DataTableRow key={observer.observer_id}>
                <DataTableCell>
                  <StatusPill online={observer.online} />
                </DataTableCell>
                <DataTableCell>
                  <TechnicalValue value={observer.observer_id} copyLabel="observer ID" />
                </DataTableCell>
                <DataTableCell>
                  <div className={styles.observerEndpoint}>
                    <TechnicalValue
                      value={observer.endpoint}
                      copyLabel="observability endpoint"
                      shorten={false}
                    />
                    <span>{observer.software}</span>
                  </div>
                </DataTableCell>
                <DataTableCell>
                  <RelativeTime value={observer.generated_at} now={now} unit="seconds" />
                </DataTableCell>
              </DataTableRow>
            ))}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </section>
  )
}
