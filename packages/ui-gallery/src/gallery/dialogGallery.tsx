import {Button, Dialog, RawDataBlock} from "@acton/ui"
import {useState} from "react"

import styles from "./dialogGallery.module.css"
import type {ComponentGallery} from "./types"

const metadataJson = JSON.stringify(
  {
    address: "0:b113a994b5024a16719f69139328eb759596c38a25f59028b146fecdc3621dfe",
    decimals: "6",
    name: "Tether USD",
    symbol: "USD₮",
  },
  undefined,
  2,
)
const diagnosticEntries = Array.from({length: 18}, (_, index) => `Diagnostic entry ${index + 1}`)

function StandardDialogSample() {
  const [open, setOpen] = useState(false)

  return (
    <article className={styles.sample}>
      <div className={styles.sampleText}>
        <h4>Inspection dialog</h4>
        <p>A compact modal combines structured details with existing technical-data components.</p>
      </div>
      <Button variant="secondary" onClick={() => setOpen(true)}>
        Open dialog
      </Button>
      <Dialog open={open} onOpenChange={setOpen} title="Metadata" maxWidth="38rem">
        <div className={styles.dialogContent}>
          <div className={styles.identity}>
            <span className={styles.avatar} aria-hidden="true">
              T
            </span>
            <div>
              <h3>Tether USD</h3>
              <p>Tether Token for Tether USD</p>
            </div>
          </div>
          <dl className={styles.details}>
            <div>
              <dt>Symbol</dt>
              <dd>USD₮</dd>
            </div>
            <div>
              <dt>Mintable</dt>
              <dd>true</dd>
            </div>
          </dl>
          <RawDataBlock title="Raw metadata" value={metadataJson} copyLabel="metadata JSON" />
        </div>
      </Dialog>
    </article>
  )
}

function ScrollingDialogSample() {
  const [open, setOpen] = useState(false)

  return (
    <article className={styles.sample}>
      <div className={styles.sampleText}>
        <h4>Long content</h4>
        <p>The shared frame stays inside the viewport while only the dialog content scrolls.</p>
      </div>
      <Button variant="outline" onClick={() => setOpen(true)}>
        Open long dialog
      </Button>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        title="Trace diagnostics"
        description="A deliberately long example for viewport and focus checks."
        maxWidth="34rem"
      >
        <ol className={styles.longList}>
          {diagnosticEntries.map(entry => (
            <li key={entry}>{entry}</li>
          ))}
        </ol>
      </Dialog>
    </article>
  )
}

export const dialogGallery = {
  id: "dialog",
  title: "Dialog",
  status: "ready",
  summary:
    "Dialog provides the shared modal frame, backdrop, focus management, dismissal behavior, and viewport-safe scrolling.",
  importStatement: 'import {Dialog} from "@acton/ui"',
  agentSummary:
    "Use Dialog for modal inspection and focused workflows. Keep domain content caller-owned and compose existing UI components inside it.",
  usage: [
    "Use for content that must trap focus and temporarily block interaction with the page.",
    "Use title as the required accessible dialog name and description for optional supporting context.",
    "Compose RawDataBlock, DataTable, AddressChip, and other domain components inside the shared frame.",
    "Let onOpenChange handle close button, Escape, and outside-press state changes.",
  ],
  avoid: [
    "Do not rebuild fixed overlays, backdrops, Escape listeners, or close buttons locally.",
    "Do not use Dialog for compact context that belongs in a Popover.",
    "Do not add a second scroll container around the shared dialog content.",
  ],
  sections: [
    {
      id: "dialog-states",
      title: "Display States",
      description:
        "Standard and long-content dialogs exercise composition, dismissal, focus management, and viewport scrolling.",
      content: (
        <div className={styles.sampleGrid}>
          <StandardDialogSample />
          <ScrollingDialogSample />
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
