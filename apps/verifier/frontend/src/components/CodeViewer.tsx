import {FileCode2, Folder} from "lucide-react"
import {useEffect, useMemo, useState, type CSSProperties} from "react"

import type {SourceFile} from "../lib/api"
import {highlightCodeToHtml, type HighlightLanguage} from "../lib/syntax-highlighter"

interface CodeViewerProps {
  readonly files: readonly SourceFile[]
  readonly entrypoint?: string
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
}

function fileContent(file: SourceFile): string {
  const content = file.content_text !== null ? file.content_text : atob(file.content_base64)
  return content.endsWith("\n") ? content.slice(0, -1) : content
}

function languageForPath(path: string): HighlightLanguage | undefined {
  const normalizedPath = path.toLowerCase()
  if (normalizedPath.endsWith(".tolk")) {
    return "tolk"
  }
  if (normalizedPath.endsWith(".fc") || normalizedPath.endsWith(".func")) {
    return "func"
  }
  if (normalizedPath.endsWith(".tact")) {
    return "tact"
  }
  if (
    normalizedPath.endsWith(".json") ||
    normalizedPath.endsWith(".abi") ||
    normalizedPath.endsWith(".pkg")
  ) {
    return "json"
  }
  return undefined
}

function lineCount(code: string): number {
  if (code.length === 0) {
    return 1
  }
  return code.split("\n").length
}

function plainCodeToHtml(code: string): string {
  const lines = code.length === 0 ? [""] : code.split("\n")
  return `<pre class="shiki plain-code"><code>${lines
    .map(line => `<span class="line">${escapeHtml(line)}</span>`)
    .join("\n")}</code></pre>`
}

function normalizeFilePath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\.?\//, "")
}

function findEntrypointFile(
  files: readonly SourceFile[],
  entrypoint: string | undefined,
): SourceFile | undefined {
  if (!entrypoint) {
    return undefined
  }

  const normalizedEntrypoint = normalizeFilePath(entrypoint)
  const exactMatch =
    files.find(file => file.path === entrypoint) ??
    files.find(file => normalizeFilePath(file.path) === normalizedEntrypoint)

  if (exactMatch) {
    return exactMatch
  }

  const suffix = `/${normalizedEntrypoint}`
  const suffixMatches = files.filter(file => normalizeFilePath(file.path).endsWith(suffix))
  return suffixMatches.length === 1 ? suffixMatches[0] : undefined
}

interface FileTreeNode {
  readonly kind: "folder" | "file"
  readonly name: string
  readonly path: string
  readonly children: readonly FileTreeNode[]
  readonly file?: SourceFile
}

interface MutableFileTreeNode {
  kind: "folder" | "file"
  name: string
  path: string
  children: Map<string, MutableFileTreeNode>
  file?: SourceFile
}

function sortTree(nodes: readonly FileTreeNode[]): FileTreeNode[] {
  return [...nodes].sort((left, right) => {
    if (left.kind !== right.kind) {
      return left.kind === "folder" ? -1 : 1
    }
    return left.name.localeCompare(right.name)
  })
}

function freezeTree(node: MutableFileTreeNode): FileTreeNode {
  return {
    kind: node.kind,
    name: node.name,
    path: node.path,
    children: sortTree([...node.children.values()].map(freezeTree)),
    file: node.file,
  }
}

function buildFileTree(files: readonly SourceFile[]): readonly FileTreeNode[] {
  const root = new Map<string, MutableFileTreeNode>()

  for (const file of files) {
    const parts = file.path.split("/").filter(Boolean)
    let currentLevel = root
    let currentPath = ""

    for (const [index, part] of parts.entries()) {
      currentPath = currentPath ? `${currentPath}/${part}` : part
      const isFile = index === parts.length - 1
      let node = currentLevel.get(part)
      if (!node) {
        node = {
          kind: isFile ? "file" : "folder",
          name: part,
          path: currentPath,
          children: new Map(),
        }
        currentLevel.set(part, node)
      }

      if (isFile) {
        node.kind = "file"
        node.file = file
      }

      currentLevel = node.children
    }
  }

  return sortTree([...root.values()].map(freezeTree))
}

function FileTreeRows({
  nodes,
  activePath,
  entrypoint,
  depth = 0,
  onSelect,
}: {
  readonly nodes: readonly FileTreeNode[]
  readonly activePath: string
  readonly entrypoint?: string
  readonly depth?: number
  readonly onSelect: (path: string) => void
}) {
  return (
    <>
      {nodes.map(node => {
        const depthStyle = {"--depth": String(depth)} as CSSProperties
        if (node.kind === "folder") {
          return (
            <div key={node.path}>
              <div className="file-tree-row file-tree-folder" style={depthStyle}>
                <Folder size={14} aria-hidden="true" />
                <span>{node.name}</span>
              </div>
              <FileTreeRows
                nodes={node.children}
                activePath={activePath}
                entrypoint={entrypoint}
                depth={depth + 1}
                onSelect={onSelect}
              />
            </div>
          )
        }

        return (
          <button
            key={node.path}
            type="button"
            className={`file-tree-row file-tree-file ${
              node.path === activePath ? "file-tree-row-active" : ""
            }`}
            style={depthStyle}
            title={node.path}
            aria-current={node.path === activePath ? "true" : undefined}
            onClick={() => onSelect(node.path)}
          >
            <FileCode2 size={14} aria-hidden="true" />
            <span>{node.name}</span>
            {node.path === entrypoint && <span className="file-tree-entrypoint">main</span>}
          </button>
        )
      })}
    </>
  )
}

export function CodeViewer({files, entrypoint}: CodeViewerProps) {
  const entrypointPath = useMemo(
    () => findEntrypointFile(files, entrypoint)?.path,
    [entrypoint, files],
  )
  const initialActivePath = entrypointPath ?? files[0]?.path ?? ""
  const [activePath, setActivePath] = useState(initialActivePath)
  const [html, setHtml] = useState("")
  const [themeRevision, setThemeRevision] = useState(0)

  const activeFile = useMemo(
    () =>
      files.find(file => file.path === activePath) ??
      files.find(file => file.path === entrypointPath) ??
      files[0],
    [activePath, entrypointPath, files],
  )
  const code = activeFile ? fileContent(activeFile) : ""
  const tree = useMemo(() => buildFileTree(files), [files])
  const isDark = document.documentElement.classList.contains("dark-theme")

  useEffect(() => {
    setActivePath(initialActivePath)
  }, [files, initialActivePath])

  useEffect(() => {
    if (!activeFile) {
      setHtml("")
      return
    }

    let cancelled = false
    const render = async () => {
      const language = languageForPath(activeFile.path)
      const highlighted = await (language
        ? highlightCodeToHtml(code, language, isDark).catch(() => plainCodeToHtml(code))
        : Promise.resolve(plainCodeToHtml(code)))
      if (!cancelled) {
        setHtml(highlighted)
      }
    }

    void render()
    return () => {
      cancelled = true
    }
  }, [activeFile, code, isDark, themeRevision])

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setThemeRevision(current => current + 1)
    })
    observer.observe(document.documentElement, {attributes: true, attributeFilter: ["class"]})
    return () => observer.disconnect()
  }, [])

  if (!activeFile) {
    return <div className="empty-state compact">No source files stored for this bundle.</div>
  }

  return (
    <section className="code-workspace" aria-label="Source code">
      <aside className="file-tree" aria-label="Source files">
        <div className="file-tree-list">
          <FileTreeRows
            nodes={tree}
            activePath={activeFile.path}
            entrypoint={entrypointPath}
            onSelect={setActivePath}
          />
        </div>
      </aside>
      <div className="code-pane">
        <div className="code-pane-header">
          <span title={activeFile.path}>{activeFile.path}</span>
        </div>
        <div className="code-frame">
          <div className="line-numbers" aria-hidden="true">
            {Array.from({length: lineCount(code)}, (_, index) => (
              <span key={index + 1}>{index + 1}</span>
            ))}
          </div>
          <div className="highlighted-code" dangerouslySetInnerHTML={{__html: html}} />
        </div>
      </div>
    </section>
  )
}
