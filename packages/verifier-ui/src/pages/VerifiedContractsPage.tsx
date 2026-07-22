import {
  CopyInlineAction,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableFooter,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  Button,
} from "@acton/ui"
import {useEffect, useMemo, useState} from "react"
import type {KeyboardEvent as ReactKeyboardEvent} from "react"

import type {LastVerifiedItem, VerifierApi} from "../lib/api"
import {shortenMiddle} from "../lib/target"
import styles from "./VerifiedPage.module.css"

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

function handleRowKeyDown(
  event: ReactKeyboardEvent<HTMLTableRowElement>,
  item: LastVerifiedItem,
  onOpenContract: (item: LastVerifiedItem) => void,
): void {
  if (event.currentTarget !== event.target) {
    return
  }

  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault()
    onOpenContract(item)
  }
}

export interface VerifiedContractsPageProps {
  readonly api: VerifierApi
  readonly onOpenContract: (item: LastVerifiedItem) => void
  readonly limit?: number
  readonly className?: string
}

export function VerifiedContractsPage({
  api,
  onOpenContract,
  limit = 25,
  className,
}: VerifiedContractsPageProps) {
  const pageSize = Number.isFinite(limit) ? Math.min(Math.max(Math.trunc(limit), 1), 99) : 25
  const [page, setPage] = useState(0)
  const [items, setItems] = useState<readonly LastVerifiedItem[]>([])
  const [hasNextPage, setHasNextPage] = useState(false)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  useEffect(() => {
    let cancelled = false

    setIsLoading(true)
    setError(undefined)

    api
      .fetchLastVerified(pageSize + 1, page * pageSize)
      .then(response => {
        if (!cancelled) {
          setItems(response.items.slice(0, pageSize))
          setHasNextPage(response.items.length > pageSize)
          setError(undefined)
        }
      })
      .catch(error => {
        if (!cancelled) {
          setItems([])
          setHasNextPage(false)
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
  }, [api, page, pageSize])

  const sortedItems = useMemo(
    () => [...items].sort((left, right) => right.verified_at - left.verified_at),
    [items],
  )

  return (
    <section className={`${styles.container} ${className ?? ""}`}>
      <section className={styles.hero}>
        <h1 className={styles.title}>Verified contracts</h1>
      </section>

      <DataTable title="Contracts" minWidth="53.75rem">
        <DataTableTable aria-label="Verified contracts">
          <DataTableHead>
            <DataTableRow>
              <DataTableHeaderCell columnWidth="32%">Code hash</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="20%">Name</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="18%">Compiler</DataTableHeaderCell>
              <DataTableHeaderCell columnWidth="10%">Files</DataTableHeaderCell>
              <DataTableHeaderCell>Verified at</DataTableHeaderCell>
            </DataTableRow>
          </DataTableHead>
          <DataTableBody>
            {isLoading ? (
              <DataTableSkeletonRows
                columns={5}
                rows={8}
                widths={["72%", "54%", "48%", "2.5rem", "68%"]}
              />
            ) : error ? (
              <DataTableEmpty colSpan={5}>{error}</DataTableEmpty>
            ) : sortedItems.length === 0 ? (
              <DataTableEmpty colSpan={5}>No verified contracts indexed yet</DataTableEmpty>
            ) : (
              sortedItems.map(item => (
                <DataTableRow
                  key={`${item.code_hash}:${item.source_bundle_hash}`}
                  interactive
                  role="link"
                  tabIndex={0}
                  aria-label={`Open code hash ${item.code_hash}`}
                  onClick={() => onOpenContract(item)}
                  onKeyDown={event => handleRowKeyDown(event, item, onOpenContract)}
                >
                  <DataTableCell>
                    <div className={styles.codeHashCell}>
                      <span className={styles.codeHash} title={item.code_hash}>
                        {shortenMiddle(item.code_hash, 18, 12)}
                      </span>
                      <CopyInlineAction
                        className={styles.hashCopyButton}
                        value={item.code_hash}
                        label="Copy code hash"
                        copiedLabel="Code hash copied"
                      />
                    </div>
                  </DataTableCell>
                  <DataTableCell>
                    <span className={styles.sourceName} title={sourceName(item)}>
                      {sourceName(item)}
                    </span>
                  </DataTableCell>
                  <DataTableCell truncate title={compilerLabel(item)}>
                    {compilerLabel(item)}
                  </DataTableCell>
                  <DataTableCell>{item.file_count}</DataTableCell>
                  <DataTableCell truncate>{formatVerifiedAt(item.verified_at)}</DataTableCell>
                </DataTableRow>
              ))
            )}
          </DataTableBody>
          {(page > 0 || hasNextPage) && (
            <DataTableFooter>
              <DataTableRow>
                <DataTableCell className={styles.paginationCell} colSpan={5}>
                  <div className={styles.pagination}>
                    <span className={styles.paginationStatus} aria-live="polite">
                      Page {page + 1}
                    </span>
                    <div className={styles.paginationActions}>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={isLoading || page === 0}
                        onClick={() => setPage(current => Math.max(0, current - 1))}
                      >
                        Previous
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={isLoading || Boolean(error) || !hasNextPage}
                        onClick={() => setPage(current => current + 1)}
                      >
                        Next
                      </Button>
                    </div>
                  </div>
                </DataTableCell>
              </DataTableRow>
            </DataTableFooter>
          )}
        </DataTableTable>
      </DataTable>
    </section>
  )
}
