import {useEffect, useRef, useState, type ReactNode} from "react"
import {
  BocInput,
  Button,
  Checkbox,
  Disclosure,
  InlineAction,
  InlineButton,
  Input,
  MultiValueInput,
  SEND_MODE_CONSTANTS,
  useToast,
} from "@acton/ui"
import {TonAddressInput, type ContractABI, type TonAddressSuggestion} from "@acton/transaction-ui"
import {ArrowDown, ArrowUp, Plus, Trash2} from "lucide-react"

import {WalletMessageBodyEditor} from "./WalletMessageBodyEditor"
import {
  decodeWalletV5Messages,
  EMPTY_MESSAGE_BODY,
  encodeWalletV5Messages,
  MAX_WALLET_V5_MESSAGES,
  type WalletSendDraft,
} from "./walletV5"
import styles from "./WalletV5MessageEditor.module.css"

const sendModeFlags = [1, 2, 4, 8, 16, 32, 64, 128].map(value => ({
  value,
  name: `${SEND_MODE_CONSTANTS[value as keyof typeof SEND_MODE_CONSTANTS]?.name ?? "Unknown flag"} (+${value})`,
}))
const sendModeOptions = sendModeFlags
  .filter(flag => flag.value in SEND_MODE_CONSTANTS)
  .map(flag => flag.name)

interface WalletMessageListEditorProps {
  readonly value: unknown
  readonly onChange: (value: unknown) => void
  readonly disabled: boolean
  readonly rawEditor: ReactNode
  readonly addressSuggestions: readonly TonAddressSuggestion[]
  readonly loadRecipientAbi: (address: string) => Promise<ContractABI | undefined>
}

/** Keeps unfinished message fields locally while invalidating the parent BoC until all rows serialize. */
export function WalletMessageListEditor({
  value,
  onChange,
  disabled,
  rawEditor,
  addressSuggestions,
  loadRecipientAbi,
}: WalletMessageListEditorProps) {
  const {showToast} = useToast()
  const [initial] = useState(() => {
    try {
      return {drafts: decodeWalletV5Messages(value), raw: false}
    } catch {
      return {drafts: [] as readonly WalletSendDraft[], raw: true}
    }
  })
  const [drafts, setDrafts] = useState(initial.drafts)
  const [raw, setRaw] = useState(initial.raw)
  const nextId = useRef(0)
  const emittedValue = useRef(value)

  useEffect(() => {
    if (value === emittedValue.current) return
    emittedValue.current = value
    try {
      setDrafts(decodeWalletV5Messages(value))
    } catch {
      setRaw(true)
    }
  }, [value])

  function update(next: readonly WalletSendDraft[]) {
    setDrafts(next)
    try {
      emittedValue.current = encodeWalletV5Messages(next)
    } catch {
      // Empty text is deliberately invalid, rather than the last valid message silently remaining active.
      emittedValue.current = ""
    }
    onChange(emittedValue.current)
  }

  function updateMessage(id: string, changes: Partial<WalletSendDraft>) {
    update(drafts.map(draft => (draft.id === id ? {...draft, ...changes} : draft)))
  }

  function move(index: number, offset: number) {
    const next = [...drafts]
    const [entry] = next.splice(index, 1)
    next.splice(index + offset, 0, entry)
    update(next)
  }

  function switchMode() {
    if (raw) {
      try {
        setDrafts(decodeWalletV5Messages(value))
        setRaw(false)
      } catch (error) {
        reportError(error)
      }
    } else if (value === "") {
      reportError(new Error("Complete the message fields before switching to the raw cell"))
    } else {
      setRaw(true)
    }
  }

  function reportError(error: unknown) {
    showToast({
      title: "Cannot build wallet messages",
      description: error instanceof Error ? error.message : String(error),
      variant: "error",
    })
  }

  return (
    <div className={styles.messages}>
      <div className={styles.header}>
        <h3 className={styles.title}>
          Outgoing messages{!raw && drafts.length > 0 ? ` (${drafts.length})` : ""}
        </h3>
        <InlineButton variant="utility" onClick={switchMode} disabled={disabled}>
          {raw ? "Use message builder" : "Edit raw cell"}
        </InlineButton>
      </div>

      {raw ? (
        rawEditor
      ) : (
        <>
          {drafts.length === 0 && (
            <p className={styles.hint}>Add a message for the wallet to send</p>
          )}
          {drafts.map((draft, index) => (
            <div
              key={draft.id}
              className={styles.message}
              role="group"
              aria-label={`Outgoing message ${index + 1}`}
            >
              <div className={styles.header}>
                <h4 className={styles.title}>Message {index + 1}</h4>
                <div className={styles.actions}>
                  <InlineAction
                    icon={<ArrowUp />}
                    label="Move message up"
                    disabled={disabled || index === 0}
                    onClick={() => move(index, -1)}
                  />
                  <InlineAction
                    icon={<ArrowDown />}
                    label="Move message down"
                    disabled={disabled || index === drafts.length - 1}
                    onClick={() => move(index, 1)}
                  />
                  <InlineAction
                    icon={<Trash2 />}
                    label="Remove message"
                    disabled={disabled}
                    onClick={() => update(drafts.filter(entry => entry.id !== draft.id))}
                  />
                </div>
              </div>
              <TonAddressInput
                label="To"
                ariaLabel={`Message ${index + 1} recipient`}
                value={draft.address}
                onValueChange={address => updateMessage(draft.id, {address})}
                suggestions={addressSuggestions}
                disabled={disabled}
              />
              <Input
                label="Amount"
                suffix="GRAM"
                inputMode="decimal"
                value={draft.amount}
                onChange={event => updateMessage(draft.id, {amount: event.target.value})}
                disabled={disabled}
              />
              <WalletMessageBodyEditor
                address={draft.address}
                value={draft.bodyBoc}
                onChange={bodyBoc => updateMessage(draft.id, {bodyBoc})}
                disabled={disabled}
                loadAbi={loadRecipientAbi}
                addressSuggestions={addressSuggestions}
              />
              <Disclosure
                label="Message options"
                className={styles.disclosure}
                contentClassName={`${styles.fields} ${styles.disclosureContent}`}
              >
                <Checkbox
                  label="Bounce"
                  checked={draft.bounce}
                  onChange={event => updateMessage(draft.id, {bounce: event.target.checked})}
                  disabled={disabled}
                />
                <MultiValueInput
                  className={styles.sendModes}
                  label="Send mode"
                  values={sendModeFlags
                    .filter(flag => Number(draft.sendMode) & flag.value)
                    .map(flag => flag.name)}
                  options={sendModeOptions}
                  placeholder="Select send modes"
                  description="External V5 messages require SendModeIgnoreErrors (+2)"
                  onValuesChange={values =>
                    updateMessage(draft.id, {
                      sendMode: String(
                        sendModeFlags
                          .filter(flag => values.includes(flag.name))
                          .reduce((mode, flag) => mode | flag.value, 0),
                      ),
                    })
                  }
                  disabled={disabled}
                />
                <BocInput
                  label="StateInit"
                  value={draft.stateInitBoc}
                  rows={3}
                  description="Optional code and data for deploying the recipient · Hex or base64 BoC"
                  onValueChange={stateInitBoc => updateMessage(draft.id, {stateInitBoc})}
                  onError={reportError}
                  disabled={disabled}
                />
              </Disclosure>
            </div>
          ))}
          <div>
            <Button
              variant="outline"
              size="sm"
              leadingIcon={<Plus size={16} />}
              disabled={disabled || drafts.length >= MAX_WALLET_V5_MESSAGES}
              onClick={() =>
                update([
                  ...drafts,
                  {
                    id: `new-${nextId.current++}`,
                    address: "",
                    amount: "0.1",
                    bounce: true,
                    sendMode: "3",
                    bodyBoc: EMPTY_MESSAGE_BODY,
                    stateInitBoc: "",
                  },
                ])
              }
            >
              Add message
            </Button>
          </div>
        </>
      )}
    </div>
  )
}
