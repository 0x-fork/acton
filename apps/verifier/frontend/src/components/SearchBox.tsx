import {History, Search, X} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {FC, KeyboardEvent as ReactKeyboardEvent, MouseEvent as ReactMouseEvent} from "react"

import {lookupPath, parseLookupTarget, shortenMiddle} from "../lib/target"
import styles from "./SearchBox.module.css"

interface SearchBoxProps {
  readonly autoFocus?: boolean
  readonly className?: string
  readonly initialValue?: string
  readonly variant?: "hero" | "header"
}

interface SearchTarget {
  readonly displayValue: string
  readonly path: string
}

const MAX_HISTORY_ITEMS = 5
const VERIFIER_HISTORY_STORAGE_KEY = "verifier-search-history"

export const SearchBox: FC<SearchBoxProps> = ({
  autoFocus = false,
  className,
  initialValue = "",
  variant = "hero",
}) => {
  const [value, setValue] = useState(initialValue)
  const [history, setHistory] = useState<readonly string[]>([])
  const [isFocused, setIsFocused] = useState(false)
  const [isInvalid, setIsInvalid] = useState(false)
  const [showHistoryDropdown, setShowHistoryDropdown] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const hasQuery = value.trim().length > 0
  const visibleHistory = hasQuery ? [] : history
  const showDropdown = showHistoryDropdown && visibleHistory.length > 0
  const rootClassName = [
    styles.search,
    variant === "header" ? styles.searchHeader : styles.searchHero,
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ")

  useEffect(() => {
    setHistory(readSearchHistory())
  }, [])

  useEffect(() => {
    if (autoFocus) {
      inputRef.current?.focus()
    }
  }, [autoFocus])

  const persistHistory = useCallback((nextHistory: readonly string[]) => {
    setHistory(nextHistory)
    localStorage.setItem(VERIFIER_HISTORY_STORAGE_KEY, JSON.stringify(nextHistory))
  }, [])

  const addToHistory = useCallback(
    (nextValue: string) => {
      const nextHistory = [nextValue, ...history.filter(item => item !== nextValue)].slice(
        0,
        MAX_HISTORY_ITEMS,
      )
      persistHistory(nextHistory)
    },
    [history, persistHistory],
  )

  const removeFromHistory = useCallback(
    (event: ReactMouseEvent, nextValue: string) => {
      event.stopPropagation()
      const nextHistory = history.filter(item => item !== nextValue)
      persistHistory(nextHistory)
      setShowHistoryDropdown(nextHistory.length > 0)
    },
    [history, persistHistory],
  )

  const handleSearch = useCallback(
    (nextValue: string) => {
      const target = resolveSearchTarget(nextValue)
      if (!target) {
        if (!nextValue.trim()) return

        setIsInvalid(true)
        return
      }

      setValue("")
      setIsInvalid(false)
      addToHistory(target.displayValue)
      setShowHistoryDropdown(false)
      window.location.assign(target.path)
    },
    [addToHistory],
  )

  const handleInputKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        handleSearch(value)
      }
    },
    [handleSearch, value],
  )

  return (
    <section className={rootClassName} aria-label="Verifier search">
      <div
        className={`${styles.inputWrapper} ${isFocused ? styles.focused : ""} ${
          isInvalid ? styles.inputInvalid : ""
        }`}
      >
        <div className={styles.searchIcon} aria-hidden="true">
          <Search size={variant === "header" ? 16 : 20} />
        </div>
        <input
          ref={inputRef}
          type="text"
          spellCheck="false"
          autoComplete="off"
          autoCorrect="off"
          className={styles.input}
          placeholder="Search by address or hash"
          value={value}
          aria-invalid={isInvalid}
          onChange={event => {
            const nextInput = event.target.value
            setValue(nextInput)
            if (isFocused) {
              setShowHistoryDropdown(true)
            }
            if (isInvalid) {
              setIsInvalid(false)
            }
          }}
          onKeyDown={handleInputKeyDown}
          onFocus={() => {
            setIsFocused(true)
            if (visibleHistory.length > 0) {
              setShowHistoryDropdown(true)
            }
          }}
          onBlur={() => {
            setIsFocused(false)
            globalThis.setTimeout(() => setShowHistoryDropdown(false), 100)
          }}
          onClick={() => {
            if (isFocused && visibleHistory.length > 0) {
              setShowHistoryDropdown(true)
            }
          }}
        />
      </div>

      {showDropdown && (
        <div className={styles.historyDropdown} onMouseDown={event => event.preventDefault()}>
          {visibleHistory.map(item => (
            <div key={`history:${item}`} className={styles.historyItem}>
              <button
                type="button"
                className={styles.historyItemButton}
                onClick={() => handleSearch(item)}
              >
                <History size={16} className={styles.historyItemIcon} aria-hidden="true" />
                <span className={styles.historyValue}>{formatHistoryItem(item)}</span>
              </button>
              <button
                type="button"
                className={styles.historyItemDeleteButton}
                onMouseDown={event => event.preventDefault()}
                onClick={event => removeFromHistory(event, item)}
                title="Remove from history"
                aria-label="Remove from history"
              >
                <X size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

function resolveSearchTarget(rawValue: string): SearchTarget | undefined {
  const trimmed = rawValue.trim()
  if (!trimmed) {
    return undefined
  }

  try {
    const target = parseLookupTarget(trimmed)
    return {
      displayValue: target.value,
      path: lookupPath(target.value),
    }
  } catch {
    return undefined
  }
}

function formatHistoryItem(value: string): string {
  try {
    const target = parseLookupTarget(value)
    return target.kind === "code_hash" ? shortenMiddle(target.value, 14, 10) : target.value
  } catch {
    return value
  }
}

function readSearchHistory(): readonly string[] {
  const savedHistory = localStorage.getItem(VERIFIER_HISTORY_STORAGE_KEY)
  if (!savedHistory) {
    return []
  }

  try {
    const parsed = JSON.parse(savedHistory)
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : []
  } catch (error) {
    console.error("Failed to parse verifier search history", error)
    return []
  }
}
