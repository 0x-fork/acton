import {Moon, Sun} from "lucide-react"
import {useEffect, useState} from "react"

import {applyTheme, getInitialTheme, type Theme} from "../lib/theme"

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(() => getInitialTheme())

  useEffect(() => {
    applyTheme(theme)
  }, [theme])

  const nextTheme = theme === "dark" ? "light" : "dark"

  return (
    <button
      type="button"
      className="theme-switch"
      title={`Switch to ${nextTheme} theme`}
      aria-label={`Switch to ${nextTheme} theme`}
      data-theme-toggle=""
      onClick={() => setTheme(nextTheme)}
    >
      <Sun
        fill="currentColor"
        className={`theme-switch-item ${theme === "light" ? "theme-switch-item-active" : ""}`}
      />
      <Moon
        fill="currentColor"
        className={`theme-switch-item ${theme === "dark" ? "theme-switch-item-active" : ""}`}
      />
    </button>
  )
}
