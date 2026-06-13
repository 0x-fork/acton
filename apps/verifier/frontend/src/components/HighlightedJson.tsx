import {useEffect, useState} from "react"

import {highlightJsonToHtml} from "../lib/syntax-highlighter"

interface HighlightedJsonProps {
  readonly value: unknown
}

export function HighlightedJson({value}: HighlightedJsonProps) {
  const prettyCode = JSON.stringify(value, null, 2)
  const compactCode = JSON.stringify(value)
  const readableCompactCode =
    compactCode.length <= 96
      ? compactCode.replaceAll(":", ": ").replaceAll(",", ", ").replace(/^\{/, "{ ").replace(/\}$/, " }")
      : compactCode
  const code = readableCompactCode.length <= 112 ? readableCompactCode : prettyCode
  const [html, setHtml] = useState("")
  const [themeRevision, setThemeRevision] = useState(0)
  const isDark = document.documentElement.classList.contains("dark-theme")

  useEffect(() => {
    let cancelled = false

    const render = async () => {
      const highlighted = await highlightJsonToHtml(code, isDark)
      if (!cancelled) {
        setHtml(highlighted)
      }
    }

    void render()
    return () => {
      cancelled = true
    }
  }, [code, isDark, themeRevision])

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeRevision(current => current + 1)
    })
    observer.observe(document.documentElement, {attributes: true, attributeFilter: ["class"]})
    return () => observer.disconnect()
  }, [])

  return <div className="highlighted-json" dangerouslySetInnerHTML={{__html: html}} />
}
