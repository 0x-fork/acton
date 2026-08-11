// biome-ignore lint/correctness/noNodejsModules: Vite plugins run in Node.js during builds
import {mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync} from "node:fs"
// biome-ignore lint/correctness/noNodejsModules: Vite plugins run in Node.js during builds
import path from "node:path"
// biome-ignore lint/correctness/noNodejsModules: Vite plugins run in Node.js during builds
import {constants, gzipSync} from "node:zlib"

export function gzipEmbeddedAssets(buildDirectory: string) {
  const embeddedDirectory = path.join(buildDirectory, ".embedded")

  return {
    name: "acton-gzip-embedded-assets",
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
