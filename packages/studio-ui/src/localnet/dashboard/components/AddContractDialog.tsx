import {Button, Dialog, DialogActions, Input, useToast} from "@acton/ui"
import {useEffect, useState} from "react"
import type {FormEvent} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"
import {parseAddress, formatAddress} from "@acton/explorer-core/components/utils"
import {useAddressFormat} from "@acton/explorer-core/hooks/useNetworkInfo"

import styles from "./AddContractDialog.module.css"

interface AddContractDialogProps {
  readonly client: TonClient
  readonly open: boolean
  readonly onAdded: () => Promise<void>
  readonly onOpenChange: (open: boolean) => void
}

export function AddContractDialog({client, open, onAdded, onOpenChange}: AddContractDialogProps) {
  const {showToast, updateToast} = useToast()
  const addressFormat = useAddressFormat()
  const [address, setAddress] = useState("")
  const [name, setName] = useState("")
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (!open) {
      setAddress("")
      setName("")
    }
  }, [open])

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault()
    if (submitting) return

    const parsedAddress = parseAddress(address.trim())
    if (!parsedAddress) {
      showToast({
        title: "Contract not added",
        description: "Enter a valid TON address",
        variant: "error",
      })
      return
    }

    const contractAddress = formatAddress(parsedAddress.toRawString(), false, addressFormat)
    const contractName = name.trim()
    const toastId = showToast({
      title: "Adding contract",
      description: contractName || contractAddress,
      variant: "loading",
      durationMs: 0,
    })
    setSubmitting(true)
    try {
      await client.registerContract(contractAddress, contractName || undefined)
      await onAdded()
      onOpenChange(false)
      updateToast(toastId, {
        title: contractName ? `${contractName} added` : "Contract added",
        variant: "success",
        durationMs: 4000,
      })
    } catch (error) {
      updateToast(toastId, {
        title: "Contract not added",
        description: error instanceof Error ? error.message : "Failed to add contract",
        variant: "error",
        durationMs: 8000,
      })
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      open={open}
      title="Add contract"
      description="Add a deployed contract from this environment"
      maxWidth={520}
      busy={submitting}
      onOpenChange={onOpenChange}
    >
      <form className={styles.form} onSubmit={handleSubmit}>
        <Input
          autoFocus
          label="Address"
          mono
          required
          value={address}
          placeholder="EQ… or 0:…"
          onChange={event => setAddress(event.target.value)}
        />
        <Input
          label="Name"
          description="Optional name shown throughout Studio"
          value={name}
          placeholder="Counter"
          onChange={event => setName(event.target.value)}
        />
        <DialogActions stackOnMobile className={styles.actions}>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {submitting ? "Close" : "Cancel"}
          </Button>
          <Button type="submit" variant="primary" loading={submitting} disabled={!address.trim()}>
            Add contract
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  )
}
