import {Button, CodeViewer, CopyInlineAction, HighlightedCode, PillTab, PillTabs} from "@acton/ui"
import {useEffect, useMemo, useState} from "react"
import {Download} from "lucide-react"

import {StatusPill} from "../components/StatusPill"
import compilerIcon from "../assets/ton-verifier-icons/compiler.svg"
import contractIcon from "../assets/ton-verifier-icons/contract.svg"
import verificationIcon from "../assets/ton-verifier-icons/verification.svg"
import verificationAlertIcon from "../assets/ton-verifier-icons/verification-alert.svg"
import verificationBinaryIcon from "../assets/ton-verifier-icons/verification-binary.svg"
import verificationBombIcon from "../assets/ton-verifier-icons/verification-bomb.svg"
import verificationPaperIcon from "../assets/ton-verifier-icons/verification-paper.svg"
import verifiedSourceIcon from "../assets/ton-verifier-icons/verified-light.svg"
import type {SourceBundle, VerificationSourceResponse, VerifierApi} from "../lib/api"
import {downloadSourceArchive} from "../lib/source-archive"
import {parseLookupTarget, shortenMiddle, type LookupTarget} from "../lib/target"
import detailsStyles from "./ContractDetails.module.css"
import summaryStyles from "./ContractSummary.module.css"
import styles from "./VerifiedContractPage.module.css"

function DetailRow({
  label,
  value,
  monospace = false,
}: {
  readonly label: string
  readonly value: string
  readonly monospace?: boolean
}) {
  return (
    <div
      className={`${detailsStyles.detailRow} ${monospace ? detailsStyles.detailRowMonospace : ""}`}
    >
      <dt>{label}</dt>
      <dd>
        <span title={value}>{value}</span>
        <CopyInlineAction value={value} label={`Copy ${label}`} copiedLabel={`${label} copied`} />
      </dd>
    </div>
  )
}

function formatVerifiedAt(timestamp: number): string | undefined {
  if (!Number.isFinite(timestamp) || timestamp <= 0) {
    return undefined
  }

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  }).format(new Date(timestamp * 1000))
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
    <div className={summaryStyles.panelHeading}>
      <img className={summaryStyles.panelHeadingIcon} src={icon} alt="" aria-hidden="true" />
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
    text: "You can review the stored source bundle and perform your own client-side verification.",
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
    <section className={summaryStyles.summaryProof} aria-label="How this contract is verified">
      <PanelHeading
        icon={verificationIcon}
        label="Verification"
        title="How is this contract verified?"
      />
      <div className={summaryStyles.verificationPointGrid}>
        {verificationPoints.map(point => (
          <div className={summaryStyles.verificationPoint} key={point.text}>
            <img
              className={summaryStyles.verificationPointIcon}
              src={point.icon}
              alt=""
              aria-hidden="true"
            />
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
    <PillTabs role="tablist" ariaLabel="Source bundles">
      {bundles.map(bundle => (
        <PillTab
          key={bundle.source_bundle_hash}
          role="tab"
          aria-selected={bundle.source_bundle_hash === activeBundle.source_bundle_hash}
          selected={bundle.source_bundle_hash === activeBundle.source_bundle_hash}
          onClick={() => onSelect(bundle)}
        >
          {shortenMiddle(bundle.source_bundle_hash, 8, 6)}
        </PillTab>
      ))}
    </PillTabs>
  )
}

function lookupAddress(lookupTarget: LookupTarget | undefined): string | undefined {
  return lookupTarget?.kind === "address" ? lookupTarget.value : undefined
}

function VerifiedContract({
  data,
  lookupTarget,
  selectedSourcePath,
  onSelectedSourcePathChange,
}: {
  readonly data: VerificationSourceResponse
  readonly lookupTarget: LookupTarget | undefined
  readonly selectedSourcePath?: string
  readonly onSelectedSourcePathChange?: (path: string) => void
}) {
  const address = lookupAddress(lookupTarget)
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
      <section className={styles["empty-state"]}>
        <StatusPill verified={false} />
        <h2>Contract is indexed, but no verified source bundle is available.</h2>
      </section>
    )
  }

  const verifiedAt = formatVerifiedAt(bundle.verified_at)
  const compactCompilerParams = JSON.stringify(bundle.compiler.params)
  const readableCompilerParams =
    compactCompilerParams.length <= 96
      ? compactCompilerParams
          .replaceAll(":", ": ")
          .replaceAll(",", ", ")
          .replace(/^\{/, "{ ")
          .replace(/\}$/, " }")
      : compactCompilerParams
  const compilerParams =
    readableCompilerParams.length <= 112
      ? readableCompilerParams
      : JSON.stringify(bundle.compiler.params, null, 2)
  return (
    <>
      <section className={summaryStyles.contractSummary}>
        <div className={summaryStyles.summaryMain}>
          <PanelHeading
            icon={contractIcon}
            label="Contract"
            title={address ? shortenMiddle(address, 18, 12) : "Verified code hash"}
            titleLevel="h1"
          />
          <div className={summaryStyles.summaryStatusRow}>
            <StatusPill verified={data.verified} />
          </div>
          <div className={summaryStyles.summaryFacts}>
            <div className={summaryStyles.summaryFact}>
              <span>Language</span>
              <strong>{bundle.compiler.language}</strong>
            </div>
            <div className={summaryStyles.summaryFact}>
              <span>Compiler</span>
              <strong>{bundle.compiler.version}</strong>
            </div>
            <div className={summaryStyles.summaryFact}>
              <span>Files</span>
              <strong>{bundle.files.length}</strong>
            </div>
          </div>
          <div className={summaryStyles.hashCard}>
            <span>Verified code hash</span>
            <p title={data.code_hash}>{data.code_hash}</p>
          </div>
        </div>
        <VerificationExplainer />
      </section>

      <div className={detailsStyles.layout}>
        <section className={detailsStyles.panel} aria-labelledby="verification-metadata-title">
          <div className={detailsStyles.panelHeading}>
            <img
              className={`${summaryStyles.panelHeadingIcon} ${summaryStyles.compact}`}
              src={compilerIcon}
              alt=""
              aria-hidden="true"
            />
            <h2 id="verification-metadata-title">Verification metadata</h2>
          </div>
          <dl>
            {address && <DetailRow label="Address" value={address} monospace />}
            {verifiedAt && <DetailRow label="Verified at" value={verifiedAt} />}
            <DetailRow label="Code hash" value={data.code_hash} monospace />
            <DetailRow label="Bundles" value={String(data.bundles.length)} />
            <DetailRow label="Bundle hash" value={bundle.source_bundle_hash} monospace />
            {bundle.storage_revision && (
              <DetailRow label="Storage revision" value={bundle.storage_revision} monospace />
            )}
            <DetailRow label="Language" value={bundle.compiler.language} />
            <DetailRow label="Compiler" value={bundle.compiler.version} />
            <DetailRow label="Entrypoint" value={bundle.entrypoint} />
          </dl>
          <div className={detailsStyles.metadataJson}>
            <div className={detailsStyles.metadataJsonTitle}>Compile params</div>
            <HighlightedCode
              className={detailsStyles.highlightedJson}
              value={compilerParams}
              language="json"
            />
          </div>
        </section>

        <section className={detailsStyles.sourceSection}>
          <div className={detailsStyles.sectionHeader}>
            <div className={detailsStyles.sectionTitle}>
              <img
                className={`${summaryStyles.panelHeadingIcon} ${summaryStyles.compact}`}
                src={verifiedSourceIcon}
                alt=""
                aria-hidden="true"
              />
              <div>
                <h2>Source bundle</h2>
                <span className={detailsStyles.fileCount}>{bundle.files.length} files</span>
              </div>
            </div>
            <div className={detailsStyles.sectionActions}>
              <BundleSelector
                bundles={data.bundles}
                activeBundle={bundle}
                onSelect={next => setSelectedBundleHash(next.source_bundle_hash)}
              />
              <Button
                size="sm"
                variant="outline"
                leadingIcon={<Download size={15} />}
                onClick={() => downloadSourceArchive(bundle)}
              >
                Download sources
              </Button>
            </div>
          </div>
          <CodeViewer
            key={bundle.source_bundle_hash}
            className={detailsStyles.sourceCodeViewer}
            defaultSelectedPath={selectedSourcePath}
            emptyMessage="No source files stored for this bundle"
            files={bundle.files}
            entrypoint={bundle.entrypoint}
            onSelectedPathChange={onSelectedSourcePathChange}
          />
        </section>
      </div>
    </>
  )
}

function UnverifiedContract({
  data,
  lookupTarget,
}: {
  readonly data: VerificationSourceResponse
  readonly lookupTarget: LookupTarget | undefined
}) {
  const address = lookupAddress(lookupTarget)

  return (
    <section className={styles["empty-state"]}>
      <StatusPill verified={false} />
      <h1>Contract is not verified</h1>
      <p>
        This target resolves to code hash{" "}
        <span className={styles["mono-inline"]}>{data.code_hash}</span>, but there is no stored
        source bundle for it.
      </p>
      {address && (
        <dl className={detailsStyles.summaryGrid}>
          <DetailRow label="Address" value={address} monospace />
        </dl>
      )}
    </section>
  )
}

export interface VerifiedContractPageProps {
  readonly api: VerifierApi
  readonly target: string
  readonly selectedSourcePath?: string
  readonly onSelectedSourcePathChange?: (path: string) => void
  readonly className?: string
}

export function VerifiedContractPage({
  api,
  target,
  selectedSourcePath,
  onSelectedSourcePathChange,
  className,
}: VerifiedContractPageProps) {
  const rawLookup = target.trim()
  const lookupTarget = useMemo(() => {
    try {
      return parseLookupTarget(rawLookup)
    } catch {
      return undefined
    }
  }, [rawLookup])
  const [data, setData] = useState<VerificationSourceResponse | undefined>()
  const [error, setError] = useState<string | undefined>()
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false

    const load = async () => {
      setLoading(true)
      setError(undefined)
      setData(undefined)
      try {
        const target = parseLookupTarget(rawLookup)
        const result = await api.fetchVerificationSource(target)
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
  }, [api, rawLookup])

  return (
    <div className={`${styles.page} ${className ?? ""}`}>
      {loading ? (
        <section className={styles["loading-state"]}>Loading verification state...</section>
      ) : error ? (
        <section className={`${styles["empty-state"]} ${styles["error-state"]}`}>
          <h1>Could not load contract</h1>
          <p>{error}</p>
        </section>
      ) : data?.verified ? (
        <VerifiedContract
          data={data}
          lookupTarget={lookupTarget}
          selectedSourcePath={selectedSourcePath}
          onSelectedSourcePathChange={onSelectedSourcePathChange}
        />
      ) : data ? (
        <UnverifiedContract data={data} lookupTarget={lookupTarget} />
      ) : null}
    </div>
  )
}
