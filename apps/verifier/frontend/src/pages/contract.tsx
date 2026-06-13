import {useEffect, useMemo, useState} from "react"
import {createRoot} from "react-dom/client"
import {Download} from "lucide-react"

import {AppShell} from "../components/AppShell"
import {CodeViewer} from "../components/CodeViewer"
import {CopyButton} from "../components/CopyButton"
import {HighlightedJson} from "../components/HighlightedJson"
import {SearchBox} from "../components/SearchBox"
import {StatusPill} from "../components/StatusPill"
import compilerIcon from "../assets/ton-verifier-icons/compiler.svg"
import contractIcon from "../assets/ton-verifier-icons/contract.svg"
import verificationIcon from "../assets/ton-verifier-icons/verification.svg"
import verificationAlertIcon from "../assets/ton-verifier-icons/verification-alert.svg"
import verificationBinaryIcon from "../assets/ton-verifier-icons/verification-binary.svg"
import verificationBombIcon from "../assets/ton-verifier-icons/verification-bomb.svg"
import verificationPaperIcon from "../assets/ton-verifier-icons/verification-paper.svg"
import verifiedSourceIcon from "../assets/ton-verifier-icons/verified-light.svg"
import {fetchVerificationSource, type SourceBundle, type VerificationSourceResponse} from "../lib/api"
import {downloadSourceArchive} from "../lib/source-archive"
import {getPathLookupValue, parseLookupTarget, shortenMiddle} from "../lib/target"
import "../styles.css"

function DetailRow({label, value}: {readonly label: string; readonly value: string}) {
  return (
    <div className="detail-row">
      <dt>{label}</dt>
      <dd>
        <span title={value}>{value}</span>
        <CopyButton value={value} label={label} />
      </dd>
    </div>
  )
}

function PanelHeading({
  icon,
  label,
  title,
  titleLevel = "h2",
}: {
  readonly icon: string
  readonly label: string
  readonly title: string
  readonly titleLevel?: "h1" | "h2"
}) {
  const Title = titleLevel

  return (
    <div className="panel-heading">
      <img className="panel-heading-icon" src={icon} alt="" aria-hidden="true" />
      <div>
        <span>{label}</span>
        <Title>{title}</Title>
      </div>
    </div>
  )
}

const verificationPoints = [
  {
    icon: verificationBinaryIcon,
    text: "This source code compiles to the same exact bytecode that is found on-chain.",
  },
  {
    icon: verificationPaperIcon,
    text: "You can review verification proofs and perform your own client-side verification.",
  },
  {
    icon: verificationAlertIcon,
    text: "Variable/function names may not reflect actual usage. compiler may remove unused code.",
  },
  {
    icon: verificationBombIcon,
    text: "Comments may not be honest and should generally be ignored.",
  },
] as const

function VerificationExplainer() {
  return (
    <section className="summary-proof" aria-label="How this contract is verified">
      <PanelHeading
        icon={verificationIcon}
        label="Verification"
        title="How is this contract verified?"
      />
      <div className="verification-point-grid">
        {verificationPoints.map(point => (
          <div className="verification-point" key={point.text}>
            <img className="verification-point-icon" src={point.icon} alt="" aria-hidden="true" />
            <p>{point.text}</p>
          </div>
        ))}
      </div>
    </section>
  )
}

function BundleSelector({
  bundles,
  activeBundle,
  onSelect,
}: {
  readonly bundles: readonly SourceBundle[]
  readonly activeBundle: SourceBundle
  readonly onSelect: (bundle: SourceBundle) => void
}) {
  if (bundles.length <= 1) {
    return null
  }

  return (
    <div className="bundle-tabs" role="tablist" aria-label="Source bundles">
      {bundles.map(bundle => (
        <button
          key={bundle.source_bundle_hash}
          type="button"
          role="tab"
          aria-selected={bundle.source_bundle_hash === activeBundle.source_bundle_hash}
          className={`bundle-tab ${
            bundle.source_bundle_hash === activeBundle.source_bundle_hash ? "bundle-tab-active" : ""
          }`}
          onClick={() => onSelect(bundle)}
        >
          {shortenMiddle(bundle.source_bundle_hash, 8, 6)}
        </button>
      ))}
    </div>
  )
}

function VerifiedContract({data}: {readonly data: VerificationSourceResponse}) {
  const [selectedBundleHash, setSelectedBundleHash] = useState(
    data.bundles[0]?.source_bundle_hash ?? "",
  )
  const bundle = useMemo(
    () =>
      data.bundles.find(item => item.source_bundle_hash === selectedBundleHash) ?? data.bundles[0],
    [data.bundles, selectedBundleHash],
  )

  if (!bundle) {
    return (
      <section className="empty-state">
        <StatusPill verified={false} />
        <h2>Contract is registered, but no verified source bundle is available.</h2>
      </section>
    )
  }

  return (
    <>
      <section className="contract-summary">
        <div className="summary-main">
          <PanelHeading
            icon={contractIcon}
            label="Contract"
            title={data.address ? shortenMiddle(data.address, 18, 12) : "Verified code hash"}
            titleLevel="h1"
          />
          <div className="summary-status-row">
            <StatusPill verified={data.verified} />
          </div>
          <div className="summary-facts" aria-label="Source bundle summary">
            <div className="summary-fact">
              <span>Language</span>
              <strong>{bundle.language}</strong>
            </div>
            <div className="summary-fact">
              <span>Compiler</span>
              <strong>{bundle.compiler_version}</strong>
            </div>
            <div className="summary-fact">
              <span>Files</span>
              <strong>{bundle.files.length}</strong>
            </div>
          </div>
          <div className="hash-card">
            <span>Verified code hash</span>
            <p title={data.code_hash}>{data.code_hash}</p>
          </div>
        </div>
        <VerificationExplainer />
      </section>

      <div className="contract-layout">
        <details className="metadata-panel" open>
          <summary>
            <img className="panel-heading-icon compact" src={compilerIcon} alt="" aria-hidden="true" />
            <span>Verification metadata</span>
          </summary>
          <dl>
            {data.address && <DetailRow label="Address" value={data.address} />}
            <DetailRow label="Code hash" value={data.code_hash} />
            <DetailRow label="Record" value={data.onchain.verification_record_address} />
            <DetailRow label="Master" value={data.onchain.master_address} />
            <DetailRow label="Bundle hash" value={bundle.source_bundle_hash} />
            {bundle.commit && <DetailRow label="Git commit" value={bundle.commit} />}
            <DetailRow label="Bundle path" value={bundle.bundle_path} />
            <DetailRow label="Language" value={bundle.language} />
            <DetailRow label="Compiler" value={bundle.compiler_version} />
            <DetailRow label="Entrypoint" value={bundle.entrypoint} />
          </dl>
          <div className="metadata-json">
            <div className="metadata-json-title">Compile params</div>
            <HighlightedJson value={bundle.compile_params} />
          </div>
        </details>

        <section className="source-section">
          <div className="section-header">
            <div className="section-title">
              <img className="panel-heading-icon compact" src={verifiedSourceIcon} alt="" aria-hidden="true" />
              <div>
                <h2>Source bundle</h2>
                <span>{bundle.files.length} files</span>
              </div>
            </div>
            <div className="section-actions">
              <BundleSelector
                bundles={data.bundles}
                activeBundle={bundle}
                onSelect={next => setSelectedBundleHash(next.source_bundle_hash)}
              />
              <button
                type="button"
                className="download-sources-button"
                onClick={() => downloadSourceArchive(bundle)}
              >
                <Download size={15} aria-hidden="true" />
                <span>Download sources</span>
              </button>
            </div>
          </div>
          <CodeViewer files={bundle.files} entrypoint={bundle.entrypoint} />
        </section>
      </div>
    </>
  )
}

function UnverifiedContract({data}: {readonly data: VerificationSourceResponse}) {
  return (
    <section className="empty-state">
      <StatusPill verified={false} />
      <h1>Contract is not verified</h1>
      <p>
        This target resolves to code hash <span className="mono-inline">{data.code_hash}</span>, but
        there is no confirmed source registration for it.
      </p>
      {data.address && (
        <dl className="summary-grid compact-grid">
          <DetailRow label="Address" value={data.address} />
        </dl>
      )}
    </section>
  )
}

function ContractPage() {
  const rawLookup = getPathLookupValue()
  const [data, setData] = useState<VerificationSourceResponse | undefined>()
  const [error, setError] = useState<string | undefined>()
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false

    const load = async () => {
      setLoading(true)
      setError(undefined)
      try {
        const target = parseLookupTarget(rawLookup)
        const result = await fetchVerificationSource(target)
        if (!cancelled) {
          setData(result)
        }
      } catch (error) {
        if (!cancelled) {
          setError(error instanceof Error ? error.message : String(error))
        }
      } finally {
        if (!cancelled) {
          setLoading(false)
        }
      }
    }

    void load()
    return () => {
      cancelled = true
    }
  }, [rawLookup])

  return (
    <AppShell headerAccessory={<SearchBox initialValue={rawLookup} variant="header" />}>
      <main className="contract-page">
        {loading ? (
          <section className="loading-state">Loading verification state...</section>
        ) : error ? (
          <section className="empty-state error-state">
            <h1>Could not load contract</h1>
            <p>{error}</p>
          </section>
        ) : data?.verified ? (
          <VerifiedContract data={data} />
        ) : data ? (
          <UnverifiedContract data={data} />
        ) : null}
      </main>
    </AppShell>
  )
}

createRoot(document.getElementById("root")!).render(<ContractPage />)
