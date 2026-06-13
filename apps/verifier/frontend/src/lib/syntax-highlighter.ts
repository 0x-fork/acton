import {createHighlighterCore} from "shiki/core"
import {createOnigurumaEngine} from "shiki/engine/oniguruma"
import jsonGrammar from "shiki/langs/json.mjs"

import {funcGrammar} from "./func-grammar"
import {jetbrainsDarculaTheme, jetbrainsLightTheme} from "./jetbrains-themes"
import {tactGrammar} from "./tact-grammar"
import {tolkGrammar} from "./tolk-grammar"

export type HighlightLanguage = "func" | "json" | "tact" | "tolk"

let highlighterPromise: ReturnType<typeof createHighlighterCore> | undefined

function getHighlighter() {
  highlighterPromise ??= createHighlighterCore({
    themes: [jetbrainsLightTheme, jetbrainsDarculaTheme],
    langs: [funcGrammar, tactGrammar, tolkGrammar, ...jsonGrammar],
    engine: createOnigurumaEngine(() => import("shiki/wasm")),
  })

  return highlighterPromise
}

function themeName(isDark: boolean) {
  return isDark ? "jetbrains-darcula" : "jetbrains-light"
}

export async function highlightCodeToHtml(
  code: string,
  language: HighlightLanguage,
  isDark: boolean,
) {
  const highlighter = await getHighlighter()
  return highlighter.codeToHtml(code, {
    lang: language,
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
