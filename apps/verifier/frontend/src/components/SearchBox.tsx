import {ArrowRight, Search} from "lucide-react"
import {useState} from "react"

import {lookupPath, parseLookupTarget} from "../lib/target"

interface SearchBoxProps {
  readonly initialValue?: string
  readonly variant?: "default" | "header"
}

export function SearchBox({initialValue = "", variant = "default"}: SearchBoxProps) {
  const [value, setValue] = useState(initialValue)
  const [error, setError] = useState<string | undefined>()
  const isHeader = variant === "header"

  return (
    <form
      className={`lookup-form ${isHeader ? "lookup-form-header" : ""}`}
      onSubmit={event => {
        event.preventDefault()
        try {
          parseLookupTarget(value)
          window.location.assign(lookupPath(value))
        } catch (error) {
          setError(error instanceof Error ? error.message : String(error))
        }
      }}
    >
      <div className={`lookup-input-shell ${error ? "lookup-input-error" : ""}`}>
        <Search size={isHeader ? 16 : 20} aria-hidden="true" />
        <input
          className="lookup-input"
          value={value}
          autoComplete="off"
          spellCheck={false}
          placeholder={isHeader ? "Search contract" : "Contract address or code hash"}
          aria-label="Contract address or code hash"
          onChange={event => {
            setValue(event.target.value)
            setError(undefined)
          }}
        />
        <button className="lookup-submit" type="submit" aria-label="Open contract">
          <ArrowRight size={isHeader ? 16 : 18} />
        </button>
      </div>
      {error && <div className="lookup-error">{error}</div>}
    </form>
  )
}
