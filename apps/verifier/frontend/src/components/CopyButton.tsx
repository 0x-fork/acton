import {Check, Copy} from "lucide-react"
import {useState} from "react"

interface CopyButtonProps {
  readonly value: string
  readonly label: string
}

export function CopyButton({value, label}: CopyButtonProps) {
  const [copied, setCopied] = useState(false)

  return (
    <button
      type="button"
      className="copy-button"
      aria-label={`Copy ${label}`}
      title={`Copy ${label}`}
      onClick={async () => {
        await navigator.clipboard.writeText(value)
        setCopied(true)
        window.setTimeout(() => setCopied(false), 1200)
      }}
    >
      {copied ? <Check size={14} /> : <Copy size={14} />}
    </button>
  )
}
