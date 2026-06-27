import {createRoot} from "react-dom/client"
import {useEffect, useState} from "react"

import {AppShell} from "../components/AppShell"
import {SearchBox} from "../components/SearchBox"
import {fetchLastVerified, type LastVerifiedItem} from "../lib/api"
import {lookupPath, shortenMiddle} from "../lib/target"
import "../styles.css"

function formatVerifiedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return "Unknown time"
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestamp * 1000))
}

function LastVerifiedCard({item}: {readonly item: LastVerifiedItem}) {
  return (
    <a className="recent-contract-card" href={lookupPath(item.code_hash)}>
      <div className="recent-contract-card-header">
        <span>{item.compiler.language}</span>
        {item.has_tolk_abi && <strong>ABI</strong>}
      </div>
      <div className="recent-contract-hash" title={item.code_hash}>
        {shortenMiddle(item.code_hash, 12, 10)}
      </div>
      <div className="recent-contract-meta">
        <span>{item.compiler.version}</span>
        <span>{item.file_count} files</span>
        <span>{formatVerifiedAt(item.verified_at)}</span>
      </div>
    </a>
  )
}

function HomePage() {
  const [items, setItems] = useState<readonly LastVerifiedItem[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  useEffect(() => {
    let cancelled = false

    fetchLastVerified(12)
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

  return (
    <AppShell>
      <main className="home-page">
        <section className="home-panel" aria-labelledby="home-title">
          <div className="home-kicker">TON source registry</div>
          <h1 id="home-title">Find verified contract sources</h1>
          <p className="home-copy">
            Search by contract address or code hash. The verifier checks the source registry and
            returns the stored source bundle when the code hash is verified.
          </p>
          <SearchBox />
        </section>
        <section className="recent-contracts-section" aria-labelledby="recent-contracts-title">
          <div className="recent-contracts-heading">
            <div>
              <span>Registry activity</span>
              <h2 id="recent-contracts-title">Last verified contracts</h2>
            </div>
          </div>
          {isLoading ? (
            <div className="recent-contracts-state">Loading latest verifications...</div>
          ) : error ? (
            <div className="recent-contracts-state error-state">{error}</div>
          ) : items.length === 0 ? (
            <div className="recent-contracts-state">No verified contracts indexed yet.</div>
          ) : (
            <div className="recent-contracts-grid">
              {items.map(item => (
                <LastVerifiedCard
                  key={`${item.code_hash}:${item.source_bundle_hash}`}
                  item={item}
                />
              ))}
            </div>
          )}
        </section>
      </main>
    </AppShell>
  )
}

createRoot(document.getElementById("root")!).render(<HomePage />)
