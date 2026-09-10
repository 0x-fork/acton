import {existsSync, readFileSync, readdirSync, rmSync} from "node:fs"
import path from "node:path"
import {fileURLToPath} from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const nodeModulesDir = path.resolve(scriptDir, "..", "node_modules")

const PRUNE_RULES = [
  // @tact-lang/opcode ships PDF specifications in reference/ and, in older
  // versions, TypeScript sources and tests in src/. Its runtime lives entirely
  // in dist/ and never reads either directory.
  {
    packageName: "@tact-lang/opcode",
    mainPrefixes: ["dist/"],
    paths: ["reference", "src"],
  },
  // Older @tact-lang/compiler packages ship their original TypeScript sources,
  // tests, and build artifacts, while Node executes only their dist/ entrypoint.
  // funcCompile.js supplies wasmBinary from funcfiftlib.wasm.js, so the separate
  // funcfiftlib.wasm shipped by older versions is an unused copy of that binary.
  {
    packageName: "@tact-lang/compiler",
    mainPrefixes: ["./dist/"],
    paths: ["src", "dist/func/funcfiftlib.wasm"],
  },
  // @ton/core publishes TypeScript sources, tests, and test data, but its package
  // entrypoint and all runtime imports resolve to files under dist/.
  {
    packageName: "@ton/core",
    mainPrefixes: ["dist/"],
    paths: ["src"],
  },
  // The legacy ton-core package has the same compiled dist/ layout as @ton/core;
  // its src/ directory is only the published TypeScript source and test material.
  {
    packageName: "ton-core",
    mainPrefixes: ["dist/"],
    paths: ["src"],
  },
  // ohm-js 16 uses index.js -> src/main, and 17 uses dist/ohm.cjs in Node.
  // These browser bundles are unused by either version's Node entrypoint.
  {
    packageName: "ohm-js",
    mainPrefixes: ["index.js", "./dist/ohm.cjs"],
    paths: [
      "dist/ohm.js",
      "dist/ohm.min.js",
      "dist/ohm-extras.js",
      "dist/ohm-extras.min.js",
    ],
  },
  // @tact-lang/opcode installs @ton/sandbox, but the worker's compiler runtime
  // never imports it. Keep package metadata and licenses so repeated pruning
  // can still discover and validate the package.
  {
    packageName: "@ton/sandbox",
    mainPrefixes: ["dist/"],
    paths: ["dist", "jest-environment.js", "jest-reporter.js"],
  },
]

for (const rule of PRUNE_RULES) {
  prunePackagePaths(rule)
}

function prunePackagePaths(rule) {
  const packageDirs = findInstalledPackages(nodeModulesDir, rule.packageName)
  if (packageDirs.length === 0) {
    throw new Error(`Could not find installed package ${rule.packageName}`)
  }

  for (const packageDir of packageDirs) {
    const packageJsonPath = path.join(packageDir, "package.json")
    const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"))
    if (
      packageJson.name !== rule.packageName ||
      typeof packageJson.main !== "string" ||
      !rule.mainPrefixes.some((prefix) => packageJson.main.startsWith(prefix))
    ) {
      throw new Error(`Refusing to prune unexpected package at ${packageJsonPath}`)
    }

    for (const relativePath of rule.paths) {
      rmSync(path.join(packageDir, relativePath), {
        recursive: true,
        force: true,
      })
    }
  }
}

function findInstalledPackages(rootNodeModulesDir, packageName) {
  const packageDirs = []
  visitNodeModules(rootNodeModulesDir)
  return packageDirs

  function visitNodeModules(currentNodeModulesDir) {
    if (!existsSync(currentNodeModulesDir)) {
      return
    }

    for (const entry of readdirSync(currentNodeModulesDir, {
      withFileTypes: true,
    })) {
      if (!entry.isDirectory() || entry.name === ".bin") {
        continue
      }

      const entryPath = path.join(currentNodeModulesDir, entry.name)
      if (entry.name.startsWith("@")) {
        for (const scopedEntry of readdirSync(entryPath, {
          withFileTypes: true,
        })) {
          if (scopedEntry.isDirectory()) {
            visitPackage(path.join(entryPath, scopedEntry.name))
          }
        }
      } else {
        visitPackage(entryPath)
      }
    }
  }

  function visitPackage(packageDir) {
    const packageJsonPath = path.join(packageDir, "package.json")
    if (!existsSync(packageJsonPath)) {
      return
    }

    const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"))
    if (packageJson.name === packageName) {
      packageDirs.push(packageDir)
    }

    visitNodeModules(path.join(packageDir, "node_modules"))
  }
}
