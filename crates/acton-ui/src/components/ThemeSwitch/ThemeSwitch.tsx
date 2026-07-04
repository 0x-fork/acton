import {Moon, Sun} from "lucide-react"
import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import styles from "./ThemeSwitch.module.css"

export type ThemeMode = "light" | "dark"

export type ThemeSwitchProps = Readonly<
  Omit<ComponentPropsWithRef<"button">, "children" | "onClick" | "type"> & {
    readonly onToggleTheme: () => void
    readonly theme: ThemeMode
  }
>

export function ThemeSwitch({
  "aria-label": ariaLabel = "Toggle Theme",
  className,
  onToggleTheme,
  ref,
  theme,
  ...props
}: ThemeSwitchProps) {
  return (
    <button
      {...props}
      ref={ref}
      type="button"
      className={cx(styles.themeSwitch, className)}
      aria-label={ariaLabel}
      data-theme-toggle=""
      onClick={onToggleTheme}
    >
      <Sun
        aria-hidden="true"
        fill="currentColor"
        className={cx(styles.themeSwitchItem, theme === "light" && styles.themeSwitchItemActive)}
      />
      <Moon
        aria-hidden="true"
        fill="currentColor"
        className={cx(styles.themeSwitchItem, theme === "dark" && styles.themeSwitchItemActive)}
      />
    </button>
  )
}
