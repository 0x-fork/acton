import {createRoot} from "react-dom/client"
import {useEffect, useMemo, useState} from "react"
import type {KeyboardEvent as ReactKeyboardEvent} from "react"

import {AppShell} from "../components/AppShell"
import {CopyValueButton} from "../components/CopyValueButton"
import {fetchLastVerified, type LastVerifiedItem} from "../lib/api"
import {lookupPath, shortenMiddle} from "../lib/target"
import styles from "./VerifiedPage.module.css"
import "../styles.css"

function formatVerifiedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return "Unknown"
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000))
}

function compilerLabel(item: LastVerifiedItem): string {
  const language = item.compiler.language || "unknown"
  const version = item.compiler.version || "unknown"
  return `${language} ${version}`
}

function sourceName(item: LastVerifiedItem): string {
  const abiName = item.abi_name?.trim()
  if (abiName) {
    return abiName
  }

  return item.entrypoint || "Unknown"
}

function openContract(path: string): void {
  window.location.assign(path)
}

function handleRowKeyDown(event: ReactKeyboardEvent<HTMLTableRowElement>, path: string): void {
  if (event.currentTarget !== event.target) {
    return
  }

  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault()
    openContract(path)
  }
}

function VerifiedPage() {
  const [items, setItems] = useState<readonly LastVerifiedItem[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  useEffect(() => {
    let cancelled = false

    fetchLastVerified(100)
      .then(response => {
        if (!cancelled) {
          setItems(response.items)
          setError(undefined)
        }
      })
      .catch(error => {
        if (!cancelled) {
          setError(error instanceof Error ? error.message : String(error))
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [])

  const sortedItems = useMemo(
    () => [...items].sort((left, right) => right.verified_at - left.verified_at),
    [items],
  )

  return (
    <AppShell>
      <section className={styles.container}>
        <section className={styles.hero}>
          <h1 className={styles.title}>Verified contracts</h1>
        </section>

        <section className={styles.tableFrame}>
          <header className={styles.tableTitle}>Contracts</header>
          {isLoading ? (
            <VerifiedTableSkeleton />
          ) : error ? (
            <div className={styles.empty}>{error}</div>
          ) : (
            <div className={styles.tableScroller}>
              <table className={styles.table}>
                <thead>
                  <tr>
                    <th>Code hash</th>
                    <th>Name</th>
                    <th>Compiler</th>
                    <th>Files</th>
                    <th>Verified at</th>
                  </tr>
                </thead>
                <tbody>
                  {sortedItems.length === 0 ? (
                    <tr>
                      <td colSpan={5}>
                        <div className="verified-empty">No verified contracts indexed yet</div>
                      </td>
                    </tr>
                  ) : (
                    sortedItems.map(item => {
                      const path = lookupPath(item.code_hash)

                      return (
                        <tr
                          key={`${item.code_hash}:${item.source_bundle_hash}`}
                          className={styles.tableRow}
                          role="link"
                          tabIndex={0}
                          aria-label={`Open code hash ${item.code_hash}`}
                          onClick={() => openContract(path)}
                          onKeyDown={event => handleRowKeyDown(event, path)}
                        >
                          <td>
                            <div className={styles.codeHashCell}>
                              <span className={styles.codeHash} title={item.code_hash}>
                                {shortenMiddle(item.code_hash, 18, 12)}
                              </span>
                              <CopyValueButton
                                className={styles.hashCopyButton}
                                value={item.code_hash}
                                label="code hash"
                              />
                            </div>
                          </td>
                          <td>
                            <span className={styles.sourceName} title={sourceName(item)}>
                              {sourceName(item)}
                            </span>
                          </td>
                          <td>{compilerLabel(item)}</td>
                          <td>{item.file_count}</td>
                          <td>{formatVerifiedAt(item.verified_at)}</td>
                        </tr>
                      )
                    })
                  )}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </section>
    </AppShell>
  )
}

function VerifiedTableSkeleton() {
  return (
    <div className={styles.skeletonList} aria-label="Loading verified contracts">
      {Array.from({length: 8}, (_, index) => (
        <div className={styles.skeletonRow} key={index}>
          <span />
          <span />
          <span />
        </div>
      ))}
    </div>
  )
}

createRoot(document.getElementById("root")!).render(<VerifiedPage />)
