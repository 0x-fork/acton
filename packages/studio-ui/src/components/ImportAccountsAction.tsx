import {Button, Dialog, DialogActions, InlineButton, useToast} from "@acton/ui"
import {Download} from "lucide-react"
import {createContext, useContext, useEffect, useRef, useState} from "react"
import {Link} from "react-router"

import {formatAdminOperationProgress} from "../localnet/adminOperation"
import {
  type AdminOperation,
  type ImportAccountsRequest,
  type StudioEnvironment,
  StudioRequestError,
  fetchStudioAdminOperation,
  importStudioAccounts,
} from "../studioApi"
import {
  AccountImportEditor,
  type ImportedAccountForm,
  availableImportSources,
  preferredImportSource,
} from "./AccountImportEditor"
import styles from "./ImportAccountsAction.module.css"

interface ImportAccountsActionProps {
  readonly environment: StudioEnvironment
  readonly environments: readonly StudioEnvironment[]
  readonly basePath: string
  readonly onCompleted: () => Promise<void>
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
}

/** Connects the dashboard action to a dialog that survives workspace restarts. */
export const ImportAccountsActionContext = createContext<{
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
} | null>(null)

/** Uses the same inline action as the other controls in the environment toolbar. */
export function ImportAccountsButton() {
  const action = useContext(ImportAccountsActionContext)
  if (!action) return null

  return (
    <InlineButton
      leadingIcon={<Download size={15} aria-hidden="true" />}
      aria-haspopup="dialog"
      aria-expanded={action.open}
      onClick={() => action.onOpenChange(true)}
    >
      Import accounts
    </InlineButton>
  )
}

/** Keeps import progress outside the workspace, which unmounts during a hardfork. */
export function ImportAccountsAction({
  environment,
  environments,
  basePath,
  onCompleted,
  open,
  onOpenChange,
}: ImportAccountsActionProps) {
  const {showToast, updateToast} = useToast()
  const storageKey = `actonStudioEnvironment:${environment.id}:accountImport`
  const [request, setRequest] = useState<ImportAccountsRequest | null>(() => {
    try {
      const saved = JSON.parse(
        localStorage.getItem(storageKey) ?? "null",
      ) as ImportAccountsRequest | null
      return saved &&
        typeof saved.id === "string" &&
        Array.isArray(saved.accounts) &&
        saved.accounts.every(
          account =>
            account &&
            typeof account.address === "string" &&
            typeof account.sourceEnvironmentId === "string" &&
            (account.name === undefined || typeof account.name === "string"),
        )
        ? saved
        : null
    } catch {
      return null
    }
  })
  const [accounts, setAccounts] = useState<readonly ImportedAccountForm[]>(
    () =>
      request?.accounts.map((account, id) => ({...account, id, name: account.name ?? ""})) ?? [],
  )
  const [operation, setOperation] = useState<AdminOperation | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const [uncertain, setUncertain] = useState(Boolean(request))
  const acknowledged = useRef<string | null>(null)
  const progressToastId = useRef<string | null>(null)
  const mounted = useRef(true)
  const sources = availableImportSources(environments).filter(
    source => source.id !== environment.id,
  )
  const active = operation !== null && operation.finishedAt === null
  const frozen = Boolean(request) || submitting
  const submitLock = useRef(false)

  useEffect(() => {
    if (!request || progressToastId.current) return

    progressToastId.current = showToast({
      title: "Importing accounts",
      description: operation
        ? formatAdminOperationProgress(operation.phase)
        : "Restoring import progress",
      variant: "loading",
      durationMs: 0,
    })
  }, [operation, request, showToast])

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  // Only public selections are stored here. The server persists the pinned cells
  // and finishes registration even when the browser is closed or storage is disabled.
  useEffect(() => {
    try {
      if (request) localStorage.setItem(storageKey, JSON.stringify(request))
      else localStorage.removeItem(storageKey)
    } catch {
      /* Browser storage is optional; the service still owns the operation */
    }
  }, [request, storageKey])

  useEffect(() => {
    const controller = new AbortController()
    const operationId = request?.id
    let polling = false
    let lastError: string | undefined

    async function poll() {
      if (!operationId || polling) return

      polling = true
      try {
        const current = await fetchStudioAdminOperation(
          environment.id,
          controller.signal,
          operationId,
        )
        if (controller.signal.aborted) return
        if (current && current.id === operationId) {
          acknowledged.current = current.id
          setOperation(current)
          setUncertain(false)
          const description = formatAdminOperationProgress(current.phase)
          if (current.finishedAt) {
            setRequest(null)
            const feedback = {
              title: current.phase === "completed" ? "Accounts imported" : "Accounts not imported",
              description:
                current.error ??
                (current.phase === "completed" ? (
                  <Link to={`${basePath}/contracts`}>View contracts</Link>
                ) : undefined),
              variant: current.phase === "completed" ? "success" : "error",
              durationMs: current.phase === "completed" ? 6000 : 8000,
            } as const
            if (progressToastId.current) {
              updateToast(progressToastId.current, feedback)
              progressToastId.current = null
            } else {
              showToast(feedback)
            }
            if (current.phase === "completed") await onCompleted()
            return
          }
          if (progressToastId.current) {
            updateToast(progressToastId.current, {
              title: "Importing accounts",
              description,
              variant: "loading",
              durationMs: 0,
            })
          }
        }
        lastError = undefined
      } catch (cause) {
        if (controller.signal.aborted) return
        const message = cause instanceof Error ? cause.message : String(cause)
        if (message !== lastError)
          showToast({title: "Import status unavailable", description: message, variant: "error"})
        lastError = message
      } finally {
        polling = false
      }
    }

    void poll()
    const timer = setInterval(() => void poll(), 1500)

    return () => {
      controller.abort()
      clearInterval(timer)
    }
  }, [basePath, environment.id, onCompleted, request, showToast, updateToast])

  async function submit() {
    if (submitLock.current || active) return
    let submitted: ImportAccountsRequest
    try {
      if (accounts.length === 0) throw new Error("Add at least one account to import")
      if (accounts.some(account => !account.address.trim()))
        throw new Error("Enter an account address or remove the empty import row")
      submitted = request ?? {
        id: crypto.randomUUID(),
        accounts: accounts.map(account => ({
          sourceEnvironmentId: account.sourceEnvironmentId,
          address: account.address.trim(),
          name: account.name.trim() || undefined,
        })),
      }
    } catch (cause) {
      showToast({
        title: "Check the selected accounts",
        description: cause instanceof Error ? cause.message : String(cause),
        variant: "error",
      })
      return
    }

    submitLock.current = true
    setSubmitting(true)
    setRequest(submitted)
    setOperation(null)
    onOpenChange(false)
    if (!progressToastId.current) {
      progressToastId.current = showToast({
        title: "Importing accounts",
        description: "Loading source accounts",
        variant: "loading",
        durationMs: 0,
      })
    }
    try {
      const result = await importStudioAccounts(environment.id, submitted)
      if (!mounted.current || acknowledged.current === submitted.id) return
      setOperation(result)
      setUncertain(false)
    } catch (cause) {
      if (!mounted.current || acknowledged.current === submitted.id) return
      const rejected = cause instanceof StudioRequestError && cause.status < 500
      if (rejected) setRequest(null)
      setUncertain(!rejected)
      const feedback = {
        title: "Import not submitted",
        description: `${cause instanceof Error ? cause.message : String(cause)}${rejected ? "" : "\nRetry sends the same import safely"}`,
        variant: "error",
        durationMs: 8000,
      } as const
      if (progressToastId.current) {
        updateToast(progressToastId.current, feedback)
        progressToastId.current = null
      } else {
        showToast(feedback)
      }
    } finally {
      submitLock.current = false
      if (mounted.current) setSubmitting(false)
    }
  }

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={onOpenChange}
        title="Import accounts"
        maxWidth="60rem"
        footer={
          <DialogActions className={styles.actions}>
            <Button variant="secondary" onClick={() => onOpenChange(false)}>
              {frozen ? "Close" : "Cancel"}
            </Button>
            <Button
              variant="primary"
              loading={submitting || active}
              disabled={
                (!uncertain && frozen) ||
                accounts.length === 0 ||
                (!uncertain && environment.status !== "running")
              }
              onClick={() => void submit()}
            >
              {uncertain ? "Retry same import" : "Import accounts"}
            </Button>
          </DialogActions>
        }
      >
        <div className={styles.content}>
          <p className={styles.notice}>
            The network pauses for a hardfork, imports the selected accounts, then resumes. Existing
            accounts at those addresses are replaced
          </p>
          <fieldset className={styles.fields} disabled={frozen}>
            <AccountImportEditor
              accounts={accounts}
              sources={sources}
              description="Copy account balance, code, and data from another environment"
              onAdd={() => {
                const source = preferredImportSource(sources)
                if (source) {
                  setOperation(null)
                  setAccounts(current => [
                    ...current,
                    {
                      id: Math.max(-1, ...current.map(account => account.id)) + 1,
                      sourceEnvironmentId: source.id,
                      name: "",
                      address: "",
                    },
                  ])
                }
              }}
              onChange={(id, update) => {
                setOperation(null)
                setAccounts(current =>
                  current.map(account => (account.id === id ? {...account, ...update} : account)),
                )
              }}
              onRemove={id => {
                setOperation(null)
                setAccounts(current => current.filter(account => account.id !== id))
              }}
            />
          </fieldset>
        </div>
      </Dialog>
    </>
  )
}
