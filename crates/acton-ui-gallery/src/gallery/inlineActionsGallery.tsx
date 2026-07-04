import {CopyInlineAction, InlineAction, InlineActions} from "@acton/ui"
import {Check, Copy, ExternalLink, Trash2} from "lucide-react"

import styles from "./inlineActionsGallery.module.css"
import type {ComponentGallery} from "./types"

const iconProps = {
  size: 13,
  strokeWidth: 2.25,
} as const

function CopyAction({label = "Copy", value}: Readonly<{label?: string; value: string}>) {
  return (
    <CopyInlineAction
      value={value}
      label={label}
      copiedLabel="Copied"
      icon={<Copy {...iconProps} aria-hidden="true" />}
      copiedIcon={<Check {...iconProps} aria-hidden="true" />}
    />
  )
}

function HoverSample() {
  return (
    <div className={styles.grid}>
      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Hover Reveal</h4>
          <p>Best for low-risk helper actions like copying a hash or address.</p>
        </div>
        <InlineActions
          visibility="hover"
          actions={<CopyAction label="Copy hash" value="0:9f7d8a02b618e67cf2c941f551eaf0c9" />}
        >
          <code className={styles.inlineCode}>0:9f7d8a02b618e67cf2c941f551eaf0c9</code>
        </InlineActions>
      </article>
      <article className={styles.sample}>
        <div className={styles.sampleText}>
          <h4>Always Visible</h4>
          <p>Use when the action must be discoverable or includes destructive behavior.</p>
        </div>
        <InlineActions
          visibility="always"
          actions={
            <>
              <CopyAction label="Copy account" value="Pinned wallet account" />
              <InlineAction label="Remove" icon={<Trash2 {...iconProps} aria-hidden="true" />} />
            </>
          }
        >
          <span className={styles.valueText}>Pinned wallet account</span>
        </InlineActions>
      </article>
    </div>
  )
}

function DenseRows() {
  return (
    <div className={styles.rowPanel}>
      <div className={styles.row}>
        <span className={styles.rowLabel}>Account</span>
        <InlineActions
          visibility="hover"
          actions={<CopyAction label="Copy account" value="EQD3r9LqD6N4kAFz3Va7s3J8a0w5xR3k" />}
        >
          <code className={styles.inlineCode}>EQD3r9LqD6N4kAFz3Va7s3J8a0w5xR3k</code>
        </InlineActions>
      </div>
      <div className={styles.row}>
        <span className={styles.rowLabel}>Route</span>
        <InlineActions
          visibility="always"
          actions={
            <>
              <InlineAction
                label="Open"
                icon={<ExternalLink {...iconProps} aria-hidden="true" />}
              />
              <InlineAction label="Remove" icon={<Trash2 {...iconProps} aria-hidden="true" />} />
            </>
          }
        >
          <span className={styles.valueText}>wallet-v5 / nft-sale / payout</span>
        </InlineActions>
      </div>
    </div>
  )
}

export const inlineActionsGallery = {
  id: "inline-actions",
  title: "InlineActions",
  status: "ready",
  summary:
    "InlineActions wraps inline content and attaches compact icon-only actions such as copy, open, or remove.",
  importStatement: 'import { CopyInlineAction, InlineAction, InlineActions } from "@acton/ui"',
  agentSummary:
    "Use InlineActions when an inline value needs one or more icon actions. Use CopyInlineAction for copy-to-check behavior.",
  usage: [
    "Use hover visibility for helper actions such as copy hash, copy address, or copy id.",
    "Use always visibility for destructive actions, important row actions, and touch-heavy surfaces.",
    "Use CopyInlineAction when copy should switch to a check mark after click.",
  ],
  avoid: [
    "Do not place the inline content outside InlineActions when hover or focus reveal is needed.",
    "Do not use hover visibility for delete or other high-consequence actions.",
    "Do not use InlineAction for text buttons; use InlineButton when the action needs a visible label.",
  ],
  sections: [
    {
      id: "inline-actions-visibility",
      title: "Visibility",
      description: "Hover reveal and always-visible action clusters.",
      content: <HoverSample />,
    },
    {
      id: "inline-actions-context",
      title: "Dense Rows",
      description:
        "InlineActions inside compact metadata rows with copy, open, and delete actions.",
      content: <DenseRows />,
    },
  ],
} satisfies ComponentGallery
