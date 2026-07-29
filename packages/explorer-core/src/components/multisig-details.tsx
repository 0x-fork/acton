import {useEffect, useMemo, useState} from "react"
import type {MouseEvent, ReactNode} from "react"

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
  ParsedBodySection,
  type ParsedTransactionBody,
  type ParsedValue,
  RawDataBlock,
  SendModeViewer,
  Skeleton,
} from "@acton/ui"
import {decodeCellWithAbi} from "@acton/transaction-ui"
import {Check} from "lucide-react"
import {Address, Cell} from "@ton/core"

import type {ExtendedContractABI} from "../api/compilerAbi"
import {getBundledCompilerAbiCatalog} from "../api/compilerAbiCatalog"
import type {V3Multisig, V3MultisigOrder, V3MultisigOrderAction} from "../api/types"
import {inferAbiByOpcode} from "../cell-inspector/abiInference"
import {useMetadataRegistry} from "../metadata/MetadataRegistryProvider"
import {ExplorerAddressChip} from "./ExplorerAddressChip"
import overviewStyles from "./LockerOverview.module.css"
import styles from "./MultisigDetails.module.css"
import {capitalize, formatGramAmount} from "./scheduleFormatting"

export type MultisigDetailsState =
  | {readonly status: "idle"}
  | {
      readonly status: "loading"
      readonly address: string
      readonly kind: "wallet" | "order"
    }
  | {
      readonly status: "success"
      readonly address: string
      readonly kind: "wallet"
      readonly wallet: V3Multisig
    }
  | {
      readonly status: "success"
      readonly address: string
      readonly kind: "order"
      readonly order: V3MultisigOrder
    }
  | {
      readonly status: "error"
      readonly address: string
      readonly kind: "wallet" | "order"
      readonly message: string
    }

interface MultisigOverviewProps {
  readonly state: MultisigDetailsState
  readonly onRetry: () => void
}

interface MultisigTabProps {
  readonly state: MultisigDetailsState
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}

interface MultisigOrdersTabProps extends MultisigTabProps {
  readonly onOrderClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}

export function MultisigOverview({state, onRetry}: MultisigOverviewProps) {
  if (state.status === "idle" || state.status === "loading") {
    return <MultisigOverviewSkeleton />
  }

  if (state.status === "error") {
    return (
      <section className={overviewStyles.card} aria-label="Multisig details">
        <div className={overviewStyles.error}>
          <div>
            <div className={overviewStyles.errorTitle}>Multisig details are unavailable</div>
            <div className={overviewStyles.errorMessage}>{state.message}</div>
          </div>
          <Button type="button" size="sm" variant="secondary" onClick={onRetry}>
            Retry
          </Button>
        </div>
      </section>
    )
  }

  return state.kind === "wallet" ? (
    <MultisigWalletOverview wallet={state.wallet} />
  ) : (
    <MultisigOrderOverview order={state.order} />
  )
}

export function MultisigSignersTab({state, onAddressClick}: MultisigTabProps) {
  if (state.status !== "success") {
    return (
      <MultisigTabSkeleton columns={state.status !== "idle" && state.kind === "order" ? 3 : 2} />
    )
  }

  const signers = state.kind === "wallet" ? state.wallet.signers : state.order.signers
  const proposerKeys =
    state.kind === "wallet" ? new Set(state.wallet.proposers.map(addressKey)) : undefined

  return (
    <div className={styles.tabContent}>
      <DataTable
        className={styles.flushTable}
        minWidth={state.kind === "order" ? "38rem" : "32rem"}
      >
        <DataTableTable aria-label="Multisig signers">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell>Address</DataTableHeaderCell>
              {state.kind === "wallet" ? (
                <DataTableHeaderCell columnWidth="9rem">Role</DataTableHeaderCell>
              ) : (
                <>
                  <DataTableHeaderCell columnWidth="8rem">Approval</DataTableHeaderCell>
                  <DataTableHeaderCell align="right" columnWidth="6rem">
                    #
                  </DataTableHeaderCell>
                </>
              )}
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {signers.map((signer, index) => (
              <DataTableRow key={addressKey(signer)} hover>
                <DataTableCell>
                  <ExplorerAddressChip address={signer} onAddressClick={onAddressClick} />
                </DataTableCell>
                {state.kind === "wallet" ? (
                  <DataTableCell tone="muted">
                    {proposerKeys?.has(addressKey(signer)) ? "Signer · proposer" : "Signer"}
                  </DataTableCell>
                ) : (
                  <>
                    <DataTableCell tone={isSignerApproved(state.order, index) ? "strong" : "muted"}>
                      {isSignerApproved(state.order, index) ? "Approved" : "Not approved"}
                    </DataTableCell>
                    <DataTableCell align="right" tone="muted">
                      {index + 1}
                    </DataTableCell>
                  </>
                )}
              </DataTableRow>
            ))}
            {state.kind === "wallet" &&
              state.wallet.proposers
                .filter(
                  proposer => !signers.some(signer => addressKey(signer) === addressKey(proposer)),
                )
                .map(proposer => (
                  <DataTableRow key={addressKey(proposer)} hover>
                    <DataTableCell>
                      <ExplorerAddressChip address={proposer} onAddressClick={onAddressClick} />
                    </DataTableCell>
                    <DataTableCell tone="muted">Proposer</DataTableCell>
                  </DataTableRow>
                ))}
            {signers.length === 0 &&
              (state.kind !== "wallet" || state.wallet.proposers.length === 0) && (
                <DataTableEmpty colSpan={state.kind === "order" ? 3 : 2}>
                  No contributors found
                </DataTableEmpty>
              )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </div>
  )
}

export function MultisigOrdersTab({state, onAddressClick, onOrderClick}: MultisigOrdersTabProps) {
  if (state.status !== "success" || state.kind !== "wallet") {
    return <MultisigTabSkeleton columns={4} />
  }

  const orders = [...state.wallet.orders].sort(compareOrdersDescending)

  return (
    <div className={styles.tabContent}>
      <DataTable className={styles.flushTable} minWidth="44rem">
        <DataTableTable aria-label="Multisig orders">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell>Order</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="8rem">Status</DataTableHeaderCell>
              <DataTableHeaderCell align="right" columnWidth="8rem">
                Approvals
              </DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="13rem">Expires</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {orders.map(order => {
              const status = getOrderStatus(order)
              return (
                <DataTableRow
                  key={addressKey(order.address)}
                  hover
                  interactive={Boolean(onOrderClick)}
                  tabIndex={onOrderClick ? 0 : undefined}
                  onClick={event => {
                    if (!event.defaultPrevented) {
                      onOrderClick?.(order.address, event)
                    }
                  }}
                  onKeyDown={event => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault()
                      onOrderClick?.(order.address)
                    }
                  }}
                >
                  <DataTableCell>
                    <ExplorerAddressChip
                      address={order.address}
                      onAddressClick={onOrderClick ?? onAddressClick}
                    />
                  </DataTableCell>
                  <DataTableCell>
                    <OrderStatus status={status} />
                  </DataTableCell>
                  <DataTableCell align="right" tone="strong">
                    {formatApprovals(order)}
                  </DataTableCell>
                  <DataTableCell tone="muted">
                    {formatTimestamp(order.expiration_date)}
                  </DataTableCell>
                </DataTableRow>
              )
            })}
            {orders.length === 0 && (
              <DataTableEmpty colSpan={4}>No multisig orders found</DataTableEmpty>
            )}
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </div>
  )
}

export function MultisigOrderActionsTab({state, onAddressClick}: MultisigTabProps) {
  const metadataRegistry = useMetadataRegistry()
  const [abiCandidates, setAbiCandidates] = useState<readonly ExtendedContractABI[]>()

  useEffect(() => {
    let active = true
    void Promise.all([
      getBundledCompilerAbiCatalog(),
      metadataRegistry.listCompilerAbis().catch(() => []),
    ]).then(([bundled, registered]) => {
      if (active) {
        setAbiCandidates(uniqueAbiCandidates([...bundled, ...registered.map(entry => entry.abi)]))
      }
    })
    return () => {
      active = false
    }
  }, [metadataRegistry])

  if (state.status !== "success" || state.kind !== "order") {
    return <MultisigActionsSkeleton />
  }

  const actions = state.order.actions ?? []
  return (
    <div className={styles.actionsTab}>
      {actions.map((action, index) => (
        <MultisigOrderActionDetails
          key={`${index}:${action.destination ?? ""}:${action.send_mode}`}
          action={action}
          abiCandidates={abiCandidates}
          index={index}
          onAddressClick={onAddressClick}
        />
      ))}
      {actions.length === 0 && (
        <div className={styles.emptyState}>No parsed actions are available for this order</div>
      )}
    </div>
  )
}

function MultisigWalletOverview({wallet}: {readonly wallet: V3Multisig}) {
  const totalContributors = new Set([
    ...wallet.signers.map(addressKey),
    ...wallet.proposers.map(addressKey),
  ]).size
  const threshold = wallet.threshold ?? 0
  const signerCount = wallet.signers.length
  const pendingOrders = wallet.orders.filter(order => getOrderStatus(order) === "pending").length

  return (
    <section className={overviewStyles.card} aria-labelledby="multisig-overview-title">
      <div className={overviewStyles.header}>
        <h2 id="multisig-overview-title" className={overviewStyles.title}>
          Multisig wallet
        </h2>
        <p className={overviewStyles.description}>
          Requires {threshold.toLocaleString()} of {signerCount.toLocaleString()} signers to approve
          an order before execution.
        </p>
      </div>
      <div className={overviewStyles.metrics}>
        <OverviewMetric label="Threshold" value={`${threshold} of ${signerCount}`} />
        <OverviewMetric label="Contributors" value={totalContributors.toLocaleString()} />
        <OverviewMetric label="Orders" value={wallet.orders.length.toLocaleString()} />
        <OverviewMetric label="Pending" value={pendingOrders.toLocaleString()} />
      </div>
    </section>
  )
}

function MultisigOrderOverview({order}: {readonly order: V3MultisigOrder}) {
  const status = getOrderStatus(order)
  const approvals = order.approvals_num ?? 0
  const threshold = order.threshold ?? 0
  const actionCount = order.actions?.length ?? 0

  return (
    <section className={overviewStyles.card} aria-labelledby="multisig-order-overview-title">
      <div className={overviewStyles.header}>
        <h2 id="multisig-order-overview-title" className={overviewStyles.title}>
          Multisig order #{order.order_seqno ?? "—"}
        </h2>
        <OrderStatus status={status} />
        <p className={overviewStyles.description}>
          {actionCount === 1 ? "One action" : `${actionCount} actions`} requested from a{" "}
          {order.signers.length}-signer multisig wallet.
        </p>
      </div>
      <div className={overviewStyles.metrics}>
        <OverviewMetric label="Approvals" value={`${approvals} of ${threshold}`} />
        <OverviewMetric label="Signers" value={order.signers.length.toLocaleString()} />
        <OverviewMetric label="Expires" value={formatTimestamp(order.expiration_date)} />
        <OverviewMetric label="Actions" value={actionCount.toLocaleString()} />
      </div>
      <ApprovalProgress
        approved={approvals}
        total={threshold}
        label="Approvals"
        description={`${approvals} of ${threshold} collected`}
      />
    </section>
  )
}

function OverviewMetric({label, value}: {readonly label: string; readonly value: ReactNode}) {
  return (
    <div className={overviewStyles.metric}>
      <div className={overviewStyles.metricLabel}>{label}</div>
      <div className={overviewStyles.metricValue}>{value}</div>
    </div>
  )
}

function OrderStatus({status}: {readonly status: ReturnType<typeof getOrderStatus>}) {
  return (
    <span className={`${styles.status} ${styles[`status${capitalize(status)}`]}`}>
      {status === "executed" && <Check size={14} aria-hidden="true" />}
      {capitalize(status)}
    </span>
  )
}

function ApprovalProgress({
  approved,
  total,
  label,
  description,
}: {
  readonly approved: number
  readonly total: number
  readonly label: string
  readonly description: string
}) {
  const safeTotal = Math.max(0, total)
  const safeApproved = Math.min(safeTotal, Math.max(0, approved))
  return (
    <div className={overviewStyles.progressSection}>
      <div className={overviewStyles.progressHeader}>
        <span className={overviewStyles.progressLabel}>{label}</span>
        <span className={overviewStyles.progressValue}>{description}</span>
      </div>
      <div
        className={overviewStyles.progressSegments}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={safeTotal}
        aria-valuenow={safeApproved}
        style={{gridTemplateColumns: `repeat(${Math.max(1, safeTotal)}, minmax(0, 1fr))`}}
      >
        {Array.from({length: Math.max(1, safeTotal)}, (_, index) => (
          <span
            key={index}
            className={`${overviewStyles.progressSegment} ${
              index < safeApproved
                ? overviewStyles.progressSegmentUnlocked
                : overviewStyles.progressSegmentLocked
            }`}
            aria-hidden="true"
          />
        ))}
      </div>
    </div>
  )
}

function MultisigOrderActionDetails({
  action,
  abiCandidates,
  index,
  onAddressClick,
}: {
  readonly action: V3MultisigOrderAction
  readonly abiCandidates: readonly ExtendedContractABI[] | undefined
  readonly index: number
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}) {
  const parsedBody = useMemo(
    () =>
      decodeActionBody(action.body_raw, abiCandidates ?? []) ?? createToncenterParsedBody(action),
    [abiCandidates, action],
  )
  return (
    <article className={styles.actionRow}>
      <h3 className={styles.actionRowTitle}>Action #{index}</h3>
      <div className={styles.actionRowContent}>
        <div className={styles.actionSummary}>
          <ActionSummaryItem label="Action type">
            {formatActionType(action.parsed_body_type)}
          </ActionSummaryItem>
          <ActionSummaryItem label="Destination">
            {action.destination ? (
              <ExplorerAddressChip address={action.destination} onAddressClick={onAddressClick} />
            ) : (
              "None"
            )}
          </ActionSummaryItem>
          <ActionSummaryItem label="Value">{formatActionValue(action.value)}</ActionSummaryItem>
          <ActionSummaryItem label="Send mode">
            <SendModeViewer mode={action.send_mode} />
          </ActionSummaryItem>
        </div>
        {abiCandidates === undefined ? (
          <div className={styles.actionBodyLoading}>
            <Skeleton width="8rem" />
          </div>
        ) : parsedBody ? (
          <div className={styles.actionBody}>
            <ParsedBodySection
              parsedBody={parsedBody}
              onContractClick={onAddressClick}
              title="Parsed body"
            />
          </div>
        ) : (
          <div className={styles.actionBody}>
            <RawDataBlock
              collapsible
              defaultExpanded={false}
              title="Raw body"
              value={formatRawValue(action.body_raw)}
              copyLabel="action body"
              maxHeight="16rem"
              variant="embedded"
            />
          </div>
        )}
      </div>
      {action.error && <div className={styles.actionError}>{action.error}</div>}
    </article>
  )
}

function ActionSummaryItem({
  label,
  children,
}: {
  readonly label: string
  readonly children: ReactNode
}) {
  return (
    <div className={styles.actionSummaryItem}>
      <div className={styles.actionSummaryLabel}>{label}</div>
      <div className={styles.actionSummaryValue}>{children}</div>
    </div>
  )
}

function MultisigOverviewSkeleton() {
  return (
    <section className={overviewStyles.card} aria-label="Loading multisig details" aria-busy="true">
      <div className={overviewStyles.header}>
        <Skeleton width="9rem" />
        <div className={overviewStyles.description}>
          <Skeleton width="100%" />
        </div>
      </div>
      <div className={overviewStyles.metrics}>
        {Array.from({length: 4}, (_, index) => (
          <div className={overviewStyles.metric} key={index}>
            <Skeleton width="5rem" />
            <Skeleton width="7rem" />
          </div>
        ))}
      </div>
    </section>
  )
}

function MultisigTabSkeleton({columns}: {readonly columns: number}) {
  return (
    <div className={styles.tabContent} aria-label="Loading multisig data" aria-busy="true">
      <DataTable className={styles.flushTable} minWidth="32rem">
        <DataTableTable>
          <DataTableBody>
            <DataTableSkeletonRows columns={columns} rows={3} />
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    </div>
  )
}

function MultisigActionsSkeleton() {
  return (
    <div className={styles.actionsTab} aria-label="Loading multisig actions" aria-busy="true">
      <div className={styles.actionRow}>
        <div className={styles.actionRowTitle}>
          <Skeleton width="5rem" />
        </div>
        <div className={styles.actionRowContent}>
          <Skeleton width="100%" height="3rem" radius="sm" />
        </div>
      </div>
    </div>
  )
}

function getOrderStatus(order: V3MultisigOrder): "executed" | "expired" | "pending" {
  if (order.sent_for_execution) {
    return "executed"
  }
  if (order.expiration_date !== null && order.expiration_date * 1000 <= Date.now()) {
    return "expired"
  }
  return "pending"
}

function isSignerApproved(order: V3MultisigOrder, index: number): boolean {
  if (order.approvals_mask === null || index < 0) {
    return false
  }
  try {
    return (BigInt(order.approvals_mask) & (1n << BigInt(index))) !== 0n
  } catch {
    return false
  }
}

function compareOrdersDescending(left: V3MultisigOrder, right: V3MultisigOrder): number {
  return compareIntegerStrings(right.order_seqno, left.order_seqno)
}

function compareIntegerStrings(left: string | null, right: string | null): number {
  try {
    const leftValue = BigInt(left ?? "-1")
    const rightValue = BigInt(right ?? "-1")
    return leftValue < rightValue ? -1 : leftValue > rightValue ? 1 : 0
  } catch {
    return (left ?? "").localeCompare(right ?? "")
  }
}

function formatApprovals(order: V3MultisigOrder): string {
  return `${order.approvals_num ?? 0} of ${order.threshold ?? 0}`
}

function formatTimestamp(timestamp: number | null): string {
  if (timestamp === null || !Number.isFinite(timestamp) || timestamp <= 0) {
    return "—"
  }
  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(new Date(timestamp * 1000))
}

function formatActionType(value: string): string {
  const normalized = value.trim().toLowerCase()
  if (normalized === "ton_transfer") {
    return "TON transfer"
  }
  if (!normalized || normalized === "unknown") {
    return "Contract call"
  }
  return normalized
    .split("_")
    .filter(Boolean)
    .map(part => capitalize(part))
    .join(" ")
}

function formatActionValue(value: string | null): string {
  if (value === null) {
    return "—"
  }
  try {
    return formatGramAmount(BigInt(value), 9)
  } catch {
    return value
  }
}

function formatRawValue(value: unknown): string {
  return typeof value === "string"
    ? value
    : (JSON.stringify(value, undefined, 2) ?? String(value ?? ""))
}

function decodeActionBody(
  bodyRaw: unknown,
  candidates: readonly ExtendedContractABI[],
): ParsedTransactionBody | undefined {
  if (typeof bodyRaw !== "string" || candidates.length === 0) {
    return undefined
  }

  let cell: Cell
  try {
    cell = Cell.fromBase64(bodyRaw)
  } catch {
    return undefined
  }

  const builtIn = decodeCellWithAbi(cell, candidates[0], candidates.slice(1))
  if (builtIn?.category === "comment") {
    return {name: builtIn.name, value: builtIn.value}
  }

  const inferred = inferAbiByOpcode(
    cell,
    candidates.map(abi => ({abi})),
  )
  if (!inferred.abi) {
    return undefined
  }
  const decoded = decodeCellWithAbi(cell, inferred.abi, candidates)
  return decoded?.category === "message" ? {name: decoded.name, value: decoded.value} : undefined
}

function createToncenterParsedBody(
  action: V3MultisigOrderAction,
): ParsedTransactionBody | undefined {
  if (!action.parsed_body) {
    return undefined
  }
  return {
    name: formatActionType(action.parsed_body_type),
    value: parsedValueFromJson(action.parsed_body),
  }
}

function parsedValueFromJson(value: unknown): ParsedValue {
  if (value === null || value === undefined) {
    return {kind: "null"}
  }
  if (typeof value === "boolean") {
    return {kind: "boolean", value}
  }
  if (typeof value === "string") {
    try {
      Address.parse(value)
      return {kind: "address", value}
    } catch {
      return {kind: "scalar", value}
    }
  }
  if (typeof value === "number" || typeof value === "bigint") {
    return {kind: "scalar", value: String(value)}
  }
  if (Array.isArray(value)) {
    return {kind: "array", items: value.map(parsedValueFromJson)}
  }
  if (typeof value === "object") {
    return {
      kind: "object",
      entries: Object.entries(value).map(([key, entryValue]) => ({
        key,
        value: parsedValueFromJson(entryValue),
      })),
    }
  }
  return {kind: "scalar", value: String(value)}
}

function uniqueAbiCandidates(
  candidates: readonly ExtendedContractABI[],
): readonly ExtendedContractABI[] {
  const byKey = new Map<string, ExtendedContractABI>()
  for (const candidate of candidates) {
    const hashes = candidate.code_hashes.map(hash => hash.toLowerCase()).sort()
    const key =
      hashes.length > 0
        ? hashes.join(",")
        : `${candidate.compiler_abi.contract_name}:${candidate.compiler_abi.declarations.length}`
    if (!byKey.has(key)) {
      byKey.set(key, candidate)
    }
  }
  return [...byKey.values()]
}

function addressKey(address: string): string {
  return address.trim().toLowerCase()
}
