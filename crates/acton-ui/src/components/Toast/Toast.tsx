import {Toast as ToastBase, type ToastManagerUpdateOptions} from "@base-ui/react/toast"
import {CheckCircle2, CircleAlert, Info, LoaderCircle, X} from "lucide-react"
import {
  type ComponentPropsWithoutRef,
  createContext,
  type PropsWithChildren,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
} from "react"

import {cx} from "../../lib/cx"
import styles from "./Toast.module.css"

export type ToastVariant = "info" | "success" | "error" | "loading"
export type ToastPriority = "low" | "high"

export type ToastOptions = Readonly<{
  readonly id?: string
  readonly title?: ReactNode
  readonly description?: ReactNode
  readonly variant?: ToastVariant
  readonly durationMs?: number
  readonly priority?: ToastPriority
}>

export type ToastUpdateOptions = Readonly<
  Partial<Omit<ToastOptions, "id">> & {
    readonly id?: never
  }
>

export type ToastPromiseOptions<Value> = Readonly<{
  readonly loading: ToastPromiseState<Value>
  readonly success: ToastPromiseState<Value>
  readonly error: ToastPromiseState<Value>
}>

export type ToastPromiseState<Value> =
  | string
  | ToastUpdateOptions
  | ((value: Value) => string | ToastUpdateOptions)

export type ToastContextValue = Readonly<{
  readonly showToast: (options: ToastOptions) => string
  readonly updateToast: (id: string, options: ToastUpdateOptions) => void
  readonly dismissToast: (id?: string) => void
  readonly promiseToast: <Value>(
    promise: Promise<Value>,
    options: ToastPromiseOptions<Value>,
  ) => Promise<Value>
}>

export type ToastProviderProps = PropsWithChildren<
  Readonly<{
    readonly timeoutMs?: number
    readonly limit?: number
    readonly theme?: string
    readonly viewportClassName?: string
  }>
>

type ToastData = Readonly<{
  variant?: ToastVariant
}>

const ToastContext = createContext<ToastContextValue | undefined>(undefined)
const defaultTimeoutMs = 4000
const defaultLimit = 4

export function ToastProvider({
  children,
  limit = defaultLimit,
  theme,
  timeoutMs = defaultTimeoutMs,
  viewportClassName,
}: ToastProviderProps) {
  return (
    <ToastBase.Provider limit={limit} timeout={timeoutMs}>
      <ToastProviderBridge theme={theme} viewportClassName={viewportClassName}>
        {children}
      </ToastProviderBridge>
    </ToastBase.Provider>
  )
}

function ToastProviderBridge({
  children,
  theme,
  viewportClassName,
}: PropsWithChildren<{
  readonly theme?: string
  readonly viewportClassName?: string
}>) {
  const toastManager = ToastBase.useToastManager<ToastData>()

  const showToast = useCallback(
    (options: ToastOptions) => {
      return toastManager.add(toBaseToastOptions(options, options.variant ?? "info"))
    },
    [toastManager],
  )

  const updateToast = useCallback(
    (id: string, options: ToastUpdateOptions) => {
      toastManager.update(id, toBaseToastOptions(options, options.variant))
    },
    [toastManager],
  )

  const dismissToast = useCallback(
    (id?: string) => {
      toastManager.close(id)
    },
    [toastManager],
  )

  const promiseToast = useCallback(
    <Value,>(promise: Promise<Value>, options: ToastPromiseOptions<Value>) => {
      return toastManager.promise(promise, {
        loading: resolvePromiseState(options.loading, "loading"),
        success: result => resolvePromiseState(options.success, "success", result),
        error: error => resolvePromiseState(options.error, "error", error),
      })
    },
    [toastManager],
  )

  const contextValue = useMemo<ToastContextValue>(
    () => ({
      dismissToast,
      promiseToast,
      showToast,
      updateToast,
    }),
    [dismissToast, promiseToast, showToast, updateToast],
  )

  return (
    <ToastContext.Provider value={contextValue}>
      {children}
      <ToastBase.Portal>
        <ToastBase.Viewport
          className={cx(styles.viewport, viewportClassName)}
          data-theme={theme}
        >
          <ToastList />
        </ToastBase.Viewport>
      </ToastBase.Portal>
    </ToastContext.Provider>
  )
}

function ToastList() {
  const {toasts} = ToastBase.useToastManager<ToastData>()

  return toasts.map(toast => {
    const variant = toast.data?.variant ?? normalizeToastVariant(toast.type)

    return (
      <ToastBase.Root
        key={toast.id}
        toast={toast}
        className={styles.toast}
        data-variant={variant}
        swipeDirection={["right", "down"]}
      >
        <ToastBase.Content className={styles.content}>
          <span className={styles.icon} aria-hidden="true">
            <ToastIcon variant={variant} />
          </span>
          <span className={styles.body}>
            {toast.title ? <ToastBase.Title className={styles.title} /> : undefined}
            {toast.description ? (
              <ToastBase.Description className={styles.description} />
            ) : undefined}
          </span>
          <ToastBase.Close className={styles.closeButton} aria-label="Dismiss notification">
            <X size={16} strokeWidth={2.25} aria-hidden="true" />
          </ToastBase.Close>
        </ToastBase.Content>
      </ToastBase.Root>
    )
  })
}

function ToastIcon({variant}: {readonly variant: ToastVariant}) {
  if (variant === "success") return <CheckCircle2 size={17} strokeWidth={2.25} />
  if (variant === "error") return <CircleAlert size={17} strokeWidth={2.25} />
  if (variant === "loading") return <LoaderCircle size={17} strokeWidth={2.25} />
  return <Info size={17} strokeWidth={2.25} />
}

export function useToast() {
  const context = useContext(ToastContext)

  if (!context) {
    throw new Error("useToast must be used within ToastProvider")
  }

  return context
}

function toBaseToastOptions(
  options: ToastOptions | ToastUpdateOptions,
  fallbackVariant: ToastVariant | undefined,
): ToastManagerUpdateOptions<ToastData> & {readonly id?: string} {
  const variant = options.variant ?? fallbackVariant
  const timeout = options.durationMs

  return {
    id: "id" in options ? options.id : undefined,
    title: options.title,
    description: options.description,
    priority: options.priority ?? (variant === "error" ? "high" : "low"),
    timeout,
    type: variant,
    data: {
      variant,
    },
  }
}

function resolvePromiseState<Value>(
  state: ToastPromiseState<Value>,
  variant: ToastVariant,
  value?: Value,
): ToastManagerUpdateOptions<ToastData> {
  const resolved = typeof state === "function" ? state(value as Value) : state

  if (typeof resolved === "string") {
    return toBaseToastOptions({description: resolved, variant}, variant)
  }

  return toBaseToastOptions({...resolved, variant: resolved.variant ?? variant}, variant)
}

function normalizeToastVariant(type: string | undefined): ToastVariant {
  if (type === "success" || type === "error" || type === "loading") return type
  return "info"
}

export type ToastCloseProps = ComponentPropsWithoutRef<typeof ToastBase.Close>
