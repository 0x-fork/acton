import {Eye} from "lucide-react"
import {useState} from "react"
import {Popover} from "@acton/ui"

import type {NftImageRevealScope} from "../hooks/useNftImageVisibility"
import styles from "./NftImageReveal.module.css"

interface NftImageRevealProps {
  readonly name: string
  readonly hasCollection: boolean
  readonly collectionName?: string
  readonly onReveal: (scope: NftImageRevealScope) => void
}

/** Image consent stays beside its card and never activates the card's navigation. */
export function NftImageReveal({
  name,
  hasCollection,
  collectionName,
  onReveal,
}: NftImageRevealProps) {
  const [open, setOpen] = useState(false)
  const reveal = (scope: NftImageRevealScope) => {
    setOpen(false)
    onReveal(scope)
  }

  return (
    <Popover
      interaction="click"
      placement="bottom"
      triggerAsChild
      open={open}
      onOpenChange={setOpen}
      ariaLabel="Show NFT image"
      content={
        <div className={styles.options}>
          <button type="button" onClick={() => reveal("once")}>
            Show once
          </button>
          <button type="button" onClick={() => reveal("item")}>
            Always show this NFT
          </button>
          {hasCollection && (
            <button type="button" onClick={() => reveal("collection")}>
              {collectionName ? `Always show “${collectionName}”` : "Always show this collection"}
            </button>
          )}
        </div>
      }
    >
      <button type="button" className={styles.trigger} aria-label={`Show image for ${name}`}>
        <Eye size={18} aria-hidden="true" />
      </button>
    </Popover>
  )
}
