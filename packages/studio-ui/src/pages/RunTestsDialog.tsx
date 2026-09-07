import {Button, Checkbox, Dialog, DialogActions, Input, useToast} from "@acton/ui"
import {Play} from "lucide-react"
import {type FormEvent, useEffect, useState} from "react"

import {type StartTestRunRequest, type TestRunRecord, startStudioTestRun} from "../studioApi"

import styles from "./RunTestsDialog.module.css"

interface RunTestsDialogProps {
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
  readonly onStarted: (run: TestRunRecord) => void
}

interface RunTestsFormState {
  readonly paths: string
  readonly filter: string
  readonly include: string
  readonly exclude: string
  readonly failFast: boolean
  readonly saveTraces: boolean
}

const INITIAL_FORM: RunTestsFormState = {
  paths: "",
  filter: "",
  include: "",
  exclude: "",
  failFast: false,
  saveTraces: true,
}

export function RunTestsDialog({open, onOpenChange, onStarted}: RunTestsDialogProps) {
  const {showToast, updateToast} = useToast()
  const [form, setForm] = useState(INITIAL_FORM)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState<string>()

  useEffect(() => {
    if (!open) return
    setForm(INITIAL_FORM)
    setError(undefined)
  }, [open])

  const updateForm = <Key extends keyof RunTestsFormState>(
    key: Key,
    value: RunTestsFormState[Key],
  ) => {
    setForm(current => ({...current, [key]: value}))
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault()
    if (isSubmitting) return

    const toastId = showToast({
      title: "Starting test run",
      variant: "loading",
      durationMs: 0,
    })
    setIsSubmitting(true)
    setError(undefined)
    const request: StartTestRunRequest = {
      paths: splitValues(form.paths),
      filter: form.filter.trim() || undefined,
      include: splitValues(form.include),
      exclude: splitValues(form.exclude),
      failFast: form.failFast,
      saveTraces: form.saveTraces,
    }

    try {
      const run = await startStudioTestRun(request)
      onStarted(run)
      onOpenChange(false)
      updateToast(toastId, {
        title: "Test run started",
        variant: "success",
        durationMs: 4000,
      })
    } catch (error) {
      const message = getErrorMessage(error)
      setError(message)
      updateToast(toastId, {
        title: "Test run not started",
        description: message,
        variant: "error",
        durationMs: 8000,
      })
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Run tests"
      description="Start the same Acton test command used by the CLI"
      maxWidth="42rem"
      busy={isSubmitting}
      contentPadding="none"
    >
      <form className={styles.form} onSubmit={event => void handleSubmit(event)}>
        <Input
          label="Test paths"
          description="Leave empty to discover all test files in the project"
          placeholder="tests, contracts/counter.test.tolk"
          value={form.paths}
          autoFocus
          onChange={event => updateForm("paths", event.target.value)}
        />
        <Input
          label="Test name filter"
          description="Run only tests whose name matches this filter"
          placeholder="Optional"
          value={form.filter}
          onChange={event => updateForm("filter", event.target.value)}
        />
        <div className={styles.formGrid}>
          <Input
            label="Include patterns"
            description="Comma-separated glob patterns"
            placeholder="**/integration/**"
            value={form.include}
            onChange={event => updateForm("include", event.target.value)}
          />
          <Input
            label="Exclude patterns"
            description="Comma-separated glob patterns"
            placeholder="**/slow/**"
            value={form.exclude}
            onChange={event => updateForm("exclude", event.target.value)}
          />
        </div>
        <div className={styles.options}>
          <Checkbox
            label="Fail fast"
            description="Stop after the first failed test"
            checked={form.failFast}
            onChange={event => updateForm("failFast", event.target.checked)}
          />
          <Checkbox
            label="Save traces"
            description="Keep transaction traces for inspection in Studio"
            checked={form.saveTraces}
            onChange={event => updateForm("saveTraces", event.target.checked)}
          />
        </div>

        {error ? (
          <div className={styles.error} role="alert">
            {error}
          </div>
        ) : null}

        <DialogActions>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {isSubmitting ? "Close" : "Cancel"}
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={isSubmitting}
            leadingIcon={<Play size={15} aria-hidden="true" />}
          >
            Run tests
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  )
}

function splitValues(value: string) {
  return value
    .split(/[,\n]/)
    .map(item => item.trim())
    .filter(Boolean)
}

function getErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
