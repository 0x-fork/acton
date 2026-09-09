import {existsSync, readFileSync, readdirSync, rmSync} from "node:fs"
import path from "node:path"
import {fileURLToPath} from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const nodeModulesDir = path.resolve(scriptDir, "..", "node_modules")

const PRUNE_RULES = [
  // @tact-lang/opcode ships PDF specifications in reference/, but its runtime
  // entrypoint and implementation live entirely in dist/ and never read them.
  {
    packageName: "@tact-lang/opcode",
    mainPrefix: "dist/",
    path: "reference",
  },
  // Older @tact-lang/opcode packages include their TypeScript sources and tests
  // in src/, while consumers load the compiled implementation from dist/index.js.
  {
    packageName: "@tact-lang/opcode",
    mainPrefix: "dist/",
    path: "src",
  },
  // Older @tact-lang/compiler packages ship their original TypeScript sources,
  // tests, and build artifacts, while Node executes only their dist/ entrypoint.
  {
    packageName: "@tact-lang/compiler",
    mainPrefix: "./dist/",
    path: "src",
  },
  // @ton/core publishes TypeScript sources, tests, and test data, but its package
  // entrypoint and all runtime imports resolve to files under dist/.
  {
    packageName: "@ton/core",
    mainPrefix: "dist/",
    path: "src",
  },
  // The legacy ton-core package has the same compiled dist/ layout as @ton/core;
  // its src/ directory is only the published TypeScript source and test material.
  {
    packageName: "ton-core",
    mainPrefix: "dist/",
    path: "src",
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
      !packageJson.main.startsWith(rule.mainPrefix)
    ) {
      throw new Error(`Refusing to prune unexpected package at ${packageJsonPath}`)
    }

    rmSync(path.join(packageDir, rule.path), {
      recursive: true,
      force: true,
    })
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
