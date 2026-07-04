import {Popover as PopoverBase} from "@base-ui/react/popover"
import {type ComponentPropsWithRef, type ReactNode, type Ref, useCallback, useState} from "react"

import {cx} from "../../lib/cx"
import styles from "./Popover.module.css"

export type PopoverPlacement = "top" | "right" | "bottom" | "left"
export type PopoverInteraction = "hover" | "click"

export type PopoverProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children" | "content"> & {
    readonly children: ReactNode
    readonly content: ReactNode
    readonly interaction?: PopoverInteraction
    readonly placement?: PopoverPlacement
    readonly open?: boolean
    readonly defaultOpen?: boolean
    readonly onOpenChange?: (open: boolean) => void
    readonly openDelay?: number
    readonly closeDelay?: number
    readonly offset?: number
    readonly contentClassName?: string
    readonly triggerClassName?: string
    readonly panelId?: string
    readonly ariaLabel?: string
  }
>

const defaultOffset = 8

export function Popover({
  children,
  className,
  closeDelay = 120,
  content,
  contentClassName,
  defaultOpen = false,
  interaction = "hover",
  offset = defaultOffset,
  onOpenChange,
  open,
  openDelay = 0,
  panelId,
  placement = "bottom",
  ref,
  tabIndex = 0,
  triggerClassName,
  ariaLabel,
  ...props
}: PopoverProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const [triggerElement, setTriggerElement] = useState<HTMLSpanElement | null>(null)
  const isControlled = open !== undefined
  const isOpen = open ?? uncontrolledOpen
  const portalTheme = isOpen ? getPortalTheme(triggerElement) : undefined

  const setTriggerRef = useCallback(
    (node: HTMLSpanElement | null) => {
      setTriggerElement(node)
      assignRef(ref, node)
    },
    [ref],
  )

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!isControlled) setUncontrolledOpen(nextOpen)
      onOpenChange?.(nextOpen)
    },
    [isControlled, onOpenChange],
  )

  return (
    <PopoverBase.Root open={isOpen} onOpenChange={handleOpenChange}>
      <PopoverBase.Trigger
        closeDelay={closeDelay}
        delay={openDelay}
        openOnHover={interaction === "hover"}
        render={
          <span
            {...props}
            ref={setTriggerRef}
            tabIndex={tabIndex}
            className={cx(styles.popover, className)}
            data-interaction={interaction}
          />
        }
      >
        <span className={cx(styles.trigger, triggerClassName)}>{children}</span>
      </PopoverBase.Trigger>

      <PopoverBase.Portal>
        <PopoverBase.Positioner className={styles.positioner} side={placement} sideOffset={offset}>
          <PopoverBase.Popup
            id={panelId}
            aria-label={ariaLabel}
            className={cx(styles.panel, contentClassName)}
            data-theme={portalTheme}
          >
            <PopoverBase.Arrow className={styles.arrow}>
              <ArrowSvg />
            </PopoverBase.Arrow>
            {content}
          </PopoverBase.Popup>
        </PopoverBase.Positioner>
      </PopoverBase.Portal>
    </PopoverBase.Root>
  )
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null) {
  if (!ref) return

  if (typeof ref === "function") {
    ref(value)
    return
  }

  ref.current = value
}

function getPortalTheme(trigger: HTMLElement | null) {
  const themedElement = trigger?.closest<HTMLElement>("[data-theme]")
  if (themedElement?.dataset.theme) return themedElement.dataset.theme

  if (
    typeof document !== "undefined" &&
    document.documentElement.classList.contains("dark-theme")
  ) {
    return "dark"
  }

  return undefined
}

function ArrowSvg(props: ComponentPropsWithRef<"svg">) {
  return (
    <svg width="20" height="10" viewBox="0 0 20 10" fill="none" {...props}>
      <path
        d="M9.66437 2.60207L4.80758 6.97318C4.07308 7.63423 3.11989 8 2.13172 8H0V10H20V8H18.5349C17.5468 8 16.5936 7.63423 15.8591 6.97318L11.0023 2.60207C10.622 2.2598 10.0447 2.25979 9.66437 2.60207Z"
        className={styles.arrowBody}
      />
      <path
        d="M8.99542 1.85876C9.75604 1.17425 10.9106 1.17422 11.6713 1.85878L16.5281 6.22989C17.0789 6.72568 17.7938 7.00001 18.5349 7.00001L15.89 7L11.0023 2.60207C10.622 2.2598 10.0447 2.2598 9.66436 2.60207L4.77734 7L2.13171 7.00001C2.87284 7.00001 3.58774 6.72568 4.13861 6.22989L8.99542 1.85876Z"
        className={styles.arrowOuterStroke}
      />
      <path
        d="M10.3333 3.34539L5.47654 7.71648C4.55842 8.54279 3.36693 9 2.13172 9H0V8H2.13172C3.11989 8 4.07308 7.63423 4.80758 6.97318L9.66437 2.60207C10.0447 2.25979 10.622 2.2598 11.0023 2.60207L15.8591 6.97318C16.5936 7.63423 17.5468 8 18.5349 8H20V9H18.5349C17.2998 9 16.1083 8.54278 15.1901 7.71648L10.3333 3.34539Z"
        className={styles.arrowInnerStroke}
      />
    </svg>
  )
}
