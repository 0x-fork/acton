import {createHighlighterCore} from "shiki/core"
import {createOnigurumaEngine} from "shiki/engine/oniguruma"
import jsonGrammar from "shiki/langs/json.mjs"

import {jetbrainsDarculaTheme, jetbrainsLightTheme} from "./jetbrains-themes"
import {tolkGrammar} from "./tolk-grammar"

let highlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

function getHighlighter() {
  highlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [tolkGrammar, ...jsonGrammar],
    engine: createOnigurumaEngine(() => import("shiki/wasm")),
  })

  return highlighterPromise
}

function themeName(isDark: boolean) {
  return isDark ? "jetbrains-darcula" : "jetbrains-light"
}

export async function highlightTolkToHtml(code: string, isDark: boolean) {
  const highlighter = await getHighlighter()
  return highlighter.codeToHtml(code, {
    lang: "tolk",
    theme: themeName(isDark),
  })
}

export async function highlightJsonToHtml(code: string, isDark: boolean) {
  const highlighter = await getHighlighter()
  return highlighter.codeToHtml(code, {
    lang: "json",
    theme: themeName(isDark),
  })
}
