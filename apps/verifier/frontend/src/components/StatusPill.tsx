import {CheckCircle2, CircleSlash} from "lucide-react"

interface StatusPillProps {
  readonly verified: boolean
}

export function StatusPill({verified}: StatusPillProps) {
  return (
    <span className={`status-pill ${verified ? "status-pill-verified" : "status-pill-unverified"}`}>
      {verified ? <CheckCircle2 size={15} /> : <CircleSlash size={15} />}
      {verified ? "Verified" : "Not verified"}
    </span>
  )
}
