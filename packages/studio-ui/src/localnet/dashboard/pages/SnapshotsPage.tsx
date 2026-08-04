import {Button, Dialog, Input, useToast} from "@acton/ui"
import {Archive, RotateCcw, Trash2} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {FC} from "react"

import {
  createStudioEnvironmentSnapshot,
  deleteStudioEnvironmentSnapshot,
  fetchStudioEnvironmentSnapshotOperation,
  fetchStudioEnvironmentSnapshots,
  restoreStudioEnvironmentSnapshot,
} from "../../../studioApi"
import type {
  EnvironmentSnapshot,
  EnvironmentSnapshotOperation,
  EnvironmentSnapshotOperationPhase,
  StudioEnvironment,
} from "../../../studioApi"

import pageStyles from "../DashboardPage.module.css"
import styles from "./SnapshotsPage.module.css"

interface SnapshotsPageProps {
  readonly environment: StudioEnvironment
}

type DialogState =
  | {readonly kind: "create"}
  | {readonly kind: "restore" | "delete"; readonly snapshot: EnvironmentSnapshot}

const CREATE_PHASES: readonly Phase[] = [
  {id: "preparing", label: "Prepare"},
  {id: "stopping", label: "Stop network"},
  {id: "creatingArchive", label: "Create archive"},
  {id: "starting", label: "Start network"},
]

const RESTORE_PHASES: readonly Phase[] = [
  {id: "preparing", label: "Prepare"},
  {id: "stopping", label: "Stop network"},
  {id: "restoringState", label: "Restore state"},
  {id: "resettingIndexer", label: "Rebuild index"},
  {id: "starting", label: "Start network"},
]

interface Phase {
  readonly id: EnvironmentSnapshotOperationPhase
  readonly label: string
}

export const SnapshotsPage: FC<SnapshotsPageProps> = ({environment}) => {
  const {showToast} = useToast()
  const [snapshots, setSnapshots] = useState<readonly EnvironmentSnapshot[]>([])
  const [operation, setOperation] = useState<EnvironmentSnapshotOperation | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string>()
  const [dialog, setDialog] = useState<DialogState>()
  const [snapshotName, setSnapshotName] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [deletingId, setDeletingId] = useState<string>()
  const [now, setNow] = useState(Date.now())
  const operationWasActive = useRef(false)

  const loadSnapshots = useCallback(
    async (signal?: AbortSignal) => {
      const result = await fetchStudioEnvironmentSnapshots(environment.id, signal)
      setSnapshots(result)
    },
    [environment.id],
  )

  const load = useCallback(
    async (signal?: AbortSignal) => {
      setLoading(true)
      setLoadError(undefined)
      try {
        const [nextSnapshots, nextOperation] = await Promise.all([
          fetchStudioEnvironmentSnapshots(environment.id, signal),
          fetchStudioEnvironmentSnapshotOperation(environment.id, signal),
        ])
        setSnapshots(nextSnapshots)
        setOperation(nextOperation)
      } catch (error) {
        if (!signal?.aborted) setLoadError(errorMessage(error, "Failed to load snapshots"))
      } finally {
        if (!signal?.aborted) setLoading(false)
      }
    },
    [environment.id],
  )

  useEffect(() => {
    const controller = new AbortController()
    void load(controller.signal)
    return () => controller.abort()
  }, [load])

  const active =
    operation !== null && operation.phase !== "completed" && operation.phase !== "failed"

  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    const poll = async () => {
      try {
        setOperation(
          await fetchStudioEnvironmentSnapshotOperation(environment.id, controller.signal),
        )
      } catch {
        // A later poll can recover from a short Studio or Docker interruption.
      }
    }
    const timer = globalThis.setInterval(() => void poll(), 1000)
    return () => {
      controller.abort()
      globalThis.clearInterval(timer)
    }
  }, [active, environment.id])

  useEffect(() => {
    if (!active) return
    setNow(Date.now())
    const timer = globalThis.setInterval(() => setNow(Date.now()), 1000)
    return () => globalThis.clearInterval(timer)
  }, [active])

  useEffect(() => {
    if (operationWasActive.current && operation?.phase === "completed") {
      void loadSnapshots().catch(() => undefined)
      showToast({
        variant: "success",
        title: operation.kind === "create" ? "Snapshot created" : "Snapshot restored",
        description:
          operation.kind === "create"
            ? "The archive is ready"
            : "The environment is available again",
      })
    }
    if (operationWasActive.current && operation?.phase === "failed") {
      showToast({
        variant: "error",
        title: operation.kind === "create" ? "Snapshot not created" : "Snapshot not restored",
        description: operation.error ?? "The snapshot operation failed",
      })
    }
    operationWasActive.current = active
  }, [active, loadSnapshots, operation, showToast])

  const submitDialog = useCallback(async () => {
    if (!dialog) return
    setSubmitting(true)
    try {
      if (dialog.kind === "create") {
        const name = snapshotName.trim()
        setOperation(await createStudioEnvironmentSnapshot(environment.id, name || undefined))
        setSnapshotName("")
      } else if (dialog.kind === "restore") {
        setOperation(await restoreStudioEnvironmentSnapshot(environment.id, dialog.snapshot.id))
      } else {
        setDeletingId(dialog.snapshot.id)
        await deleteStudioEnvironmentSnapshot(environment.id, dialog.snapshot.id)
        await loadSnapshots()
        showToast({
          variant: "success",
          title: "Snapshot deleted",
          description: `${snapshotLabel(dialog.snapshot)} was removed`,
        })
      }
      setDialog(undefined)
    } catch (error) {
      showToast({
        variant: "error",
        title:
          dialog.kind === "create"
            ? "Snapshot not started"
            : dialog.kind === "restore"
              ? "Restore not started"
              : "Snapshot not deleted",
        description: errorMessage(error, "The snapshot request failed"),
      })
    } finally {
      setDeletingId(undefined)
      setSubmitting(false)
    }
  }, [dialog, environment.id, loadSnapshots, showToast, snapshotName])

  return (
    <section
      className={`${pageStyles.settingsSection} ${styles.page}`}
      aria-labelledby="snapshot-list-title"
    >
      <header className={styles.header}>
        <div>
          <h2 id="snapshot-list-title">Saved snapshots</h2>
          <p>Save the current chain state and restore it later</p>
        </div>
        <Button
          size="sm"
          variant="primary"
          leadingIcon={<Archive size={14} aria-hidden="true" />}
          disabled={active || loading}
          onClick={() => setDialog({kind: "create"})}
        >
          Create snapshot
        </Button>
      </header>

      <div className={styles.notice}>
        <Archive size={17} aria-hidden="true" />
        <div>
          <strong>Snapshots can be large</strong>
          <span>
            Creation and restore can take several minutes. Studio may stop this environment while it
            works. Restore also rebuilds the index before the environment is available again.
          </span>
        </div>
      </div>

      <div className={styles.panel}>
        {active && operation ? <OperationProgress operation={operation} now={now} /> : undefined}

        {operation?.phase === "failed" ? (
          <div className={styles.operationError} role="alert">
            <strong>{operation.kind === "create" ? "Creation failed" : "Restore failed"}</strong>
            <span>{operation.error ?? "The snapshot operation failed"}</span>
          </div>
        ) : undefined}

        {loading ? (
          <div className={styles.emptyState}>Loading snapshots</div>
        ) : loadError ? (
          <div className={styles.loadError} role="alert">
            <span>{loadError}</span>
            <Button size="sm" variant="outline" onClick={() => void load()}>
              Retry
            </Button>
          </div>
        ) : snapshots.length === 0 ? (
          <div className={styles.emptyState}>No snapshots yet</div>
        ) : (
          <div className={styles.list}>
            {snapshots.map(snapshot => (
              <div className={styles.snapshotRow} key={snapshot.id}>
                <div className={styles.snapshotMain}>
                  <strong>{snapshotLabel(snapshot)}</strong>
                  <span>
                    {formatDate(snapshot.createdAt)} · {formatBytes(snapshot.archiveSizeBytes)}
                    {snapshot.stateSizeBytes > snapshot.archiveSizeBytes
                      ? ` compressed · ${formatBytes(snapshot.stateSizeBytes)} state`
                      : ""}
                  </span>
                </div>
                <div className={styles.actions}>
                  <Button
                    size="sm"
                    variant="ghost"
                    leadingIcon={<RotateCcw size={14} aria-hidden="true" />}
                    disabled={active || deletingId !== undefined}
                    onClick={() => setDialog({kind: "restore", snapshot})}
                  >
                    Restore
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    title={`Delete ${snapshotLabel(snapshot)}`}
                    disabled={active || deletingId !== undefined}
                    loading={deletingId === snapshot.id}
                    onClick={() => setDialog({kind: "delete", snapshot})}
                  >
                    <Trash2 size={14} aria-hidden="true" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <SnapshotDialog
        state={dialog}
        name={snapshotName}
        loading={submitting}
        onNameChange={setSnapshotName}
        onClose={() => setDialog(undefined)}
        onConfirm={() => void submitDialog()}
      />
    </section>
  )
}

const OperationProgress: FC<{
  readonly now: number
  readonly operation: EnvironmentSnapshotOperation
}> = ({now, operation}) => {
  const phases = operation.kind === "create" ? CREATE_PHASES : RESTORE_PHASES
  const currentIndex = Math.max(
    0,
    phases.findIndex(phase => phase.id === operation.phase),
  )
  return (
    <div className={styles.operation} role="status" aria-live="polite">
      <div className={styles.operationHeader}>
        <div>
          <strong>
            {operation.kind === "create" ? "Creating snapshot" : "Restoring snapshot"}
          </strong>
          <span>{operationDetail(operation.phase)}</span>
        </div>
        <span className={styles.elapsed}>{formatElapsed(operation.startedAt, now)}</span>
      </div>
      <ol
        className={styles.phases}
        style={{gridTemplateColumns: `repeat(${phases.length}, minmax(0, 1fr))`}}
      >
        {phases.map((phase, index) => (
          <li
            key={phase.id}
            className={
              index < currentIndex
                ? styles.phaseComplete
                : index === currentIndex
                  ? styles.phaseCurrent
                  : undefined
            }
          >
            <span className={styles.phaseDot} aria-hidden="true" />
            <span>{phase.label}</span>
          </li>
        ))}
      </ol>
      <p>You can leave this page. Studio continues the operation in the background.</p>
    </div>
  )
}

interface SnapshotDialogProps {
  readonly state: DialogState | undefined
  readonly name: string
  readonly loading: boolean
  readonly onNameChange: (value: string) => void
  readonly onClose: () => void
  readonly onConfirm: () => void
}

const SnapshotDialog: FC<SnapshotDialogProps> = ({
  state,
  name,
  loading,
  onNameChange,
  onClose,
  onConfirm,
}) => {
  const title = state
    ? state.kind === "create"
      ? "Create snapshot"
      : state.kind === "restore"
        ? `Restore ${snapshotLabel(state.snapshot)}`
        : `Delete ${snapshotLabel(state.snapshot)}`
    : "Snapshot"
  const description =
    state?.kind === "restore"
      ? "Studio will stop the environment, replace its chain state, rebuild the index, and start the environment again. This can take several minutes."
      : state?.kind === "delete"
        ? "This permanently deletes the snapshot archive."
        : "If the environment is running, Studio will stop it, create a compressed archive, and start it again. This can take several minutes."

  return (
    <Dialog
      open={state !== undefined}
      onOpenChange={open => {
        if (!open && !loading) onClose()
      }}
      title={title}
      description={description}
      dismissible={!loading}
      maxWidth="30rem"
    >
      {state?.kind === "create" ? (
        <Input
          label="Name"
          description="Optional. Use a name that identifies this point in the chain."
          maxLength={80}
          value={name}
          onChange={event => onNameChange(event.target.value)}
        />
      ) : undefined}
      <div className={styles.dialogActions}>
        <Button variant="secondary" disabled={loading} onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant={state?.kind === "delete" ? "danger" : "primary"}
          loading={loading}
          onClick={onConfirm}
        >
          {state?.kind === "restore"
            ? "Restore snapshot"
            : state?.kind === "delete"
              ? "Delete snapshot"
              : "Create snapshot"}
        </Button>
      </div>
    </Dialog>
  )
}

function snapshotLabel(snapshot: EnvironmentSnapshot): string {
  return snapshot.name ?? snapshot.id
}

function formatDate(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {dateStyle: "medium", timeStyle: "short"}).format(
    timestamp * 1000,
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ["KB", "MB", "GB", "TB"]
  let value = bytes / 1024
  let unit = units[0]
  for (const next of units.slice(1)) {
    if (value < 1024) break
    value /= 1024
    unit = next
  }
  return `${value < 10 ? value.toFixed(1) : value.toFixed(0)} ${unit}`
}

function formatElapsed(startedAt: string, now: number): string {
  const elapsedSeconds = Math.max(0, Math.floor((now - Date.parse(startedAt)) / 1000))
  const minutes = Math.floor(elapsedSeconds / 60)
  const seconds = elapsedSeconds % 60
  return minutes > 0 ? `${minutes}m ${seconds.toString().padStart(2, "0")}s` : `${seconds}s`
}

function operationDetail(phase: EnvironmentSnapshotOperationPhase): string {
  switch (phase) {
    case "preparing":
      return "Preparing the operation"
    case "stopping":
      return "Waiting for network processes to stop"
    case "creatingArchive":
      return "Compressing the persistent chain state"
    case "restoringState":
      return "Replacing the persistent chain state"
    case "resettingIndexer":
      return "Removing derived data so the index can be rebuilt"
    case "starting":
      return "Waiting for the network and APIs to become ready"
    case "completed":
      return "Operation complete"
    case "failed":
      return "Operation failed"
    default:
      return "Working"
  }
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.trim() ? error.message : fallback
}
