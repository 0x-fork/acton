import {mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync} from "node:fs"
import path from "node:path"
import {createRequire} from "node:module"
import {constants, gzipSync} from "node:zlib"

import react from "@vitejs/plugin-react"
import {defineConfig, type Plugin} from "vite"
import {nodePolyfills} from "vite-plugin-node-polyfills"

import {themeBootstrap} from "../ui/vite/themeBootstrap.ts"

const require = createRequire(import.meta.url)
const nodePolyfillsRoot = path.dirname(path.dirname(require.resolve("vite-plugin-node-polyfills")))
const outputDirectory = path.resolve(import.meta.dirname, "dist")

export default defineConfig({
  plugins: [
    themeBootstrap({storageKey: "acton-studio-theme"}),
    react(),
    nodePolyfills({
      include: ["buffer", "path"],
      globals: {
        Buffer: true,
      },
    }),
    embeddedAssets(outputDirectory),
  ],
  resolve: {
    alias: {
      "@acton/transaction-ui": path.resolve(import.meta.dirname, "../transaction-ui/src"),
      "vite-plugin-node-polyfills/shims/buffer": path.resolve(
        nodePolyfillsRoot,
        "shims/buffer/index.ts",
      ),
    },
  },
  build: {
    outDir: outputDirectory,
    emptyOutDir: true,
  },
  server: {
    port: 3015,
    proxy: {
      "/api": "http://127.0.0.1:3016",
    },
  },
  preview: {
    port: 3015,
  },
})

function embeddedAssets(buildDirectory: string): Plugin {
  const embeddedDirectory = path.join(buildDirectory, ".embedded")

  return {
    name: "acton-studio-embedded-assets",
    apply: "build",
    closeBundle() {
      rmSync(embeddedDirectory, {recursive: true, force: true})
      const sourceFiles = [...filesIn(buildDirectory)]

      for (const sourceFile of sourceFiles) {
        const outputFile = path.join(embeddedDirectory, path.relative(buildDirectory, sourceFile))
        mkdirSync(path.dirname(outputFile), {recursive: true})
        writeFileSync(
          outputFile,
          gzipSync(readFileSync(sourceFile), {level: constants.Z_BEST_COMPRESSION}),
        )
      }
    },
  }
}

function* filesIn(directory: string): Generator<string> {
  for (const entry of readdirSync(directory, {withFileTypes: true})) {
    const entryPath = path.join(directory, entry.name)

    if (entry.isDirectory()) {
      yield* filesIn(entryPath)
    } else if (entry.isFile()) {
      yield entryPath
    }
  }
}
