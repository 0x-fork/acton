import path from "node:path";

import { buildSourceMap, normalizeSourcePath } from "./common.mjs";
import { importTolk } from "./registry.mjs";

export async function compileTolk(input) {
  const { runTolkCompiler } = await importTolk(input.compiler_version);
  const sources = buildSourceMap(input.sources);
  const importMappings = buildImportMappings(input.import_mappings);
  const entrypointFileName = normalizeSourcePath(input.entrypoint);

  const result = await runTolkCompiler({
    entrypointFileName,
    pathMappings: Object.fromEntries(
      importMappings.map((mapping) => [mapping.prefix, mapping.target]),
    ),
    fsReadCallback: (requestedPath) => {
      const resolvedPath = resolveSourcePath(requestedPath, sources, importMappings);
      const content = sources.get(resolvedPath);
      if (content === undefined) {
        throw new Error(`source was not provided: ${requestedPath}`);
      }
      return content;
    },
  });

  if (result.status === "error") {
    return { status: "compile_error", error: result.message };
  }

  return {
    status: "ok",
    code_hash: String(result.codeHashHex).toLowerCase(),
    generated_sources: generatedSources(entrypointFileName, result),
  };
}

function generatedSources(entrypointFileName, result) {
  if (result.abiJson === undefined || result.abiJson === null) {
    return [];
  }

  return [
    {
      path: generatedAbiPath(entrypointFileName),
      content: `${JSON.stringify(result.abiJson, null, 2)}\n`,
    },
  ];
}

function generatedAbiPath(entrypointFileName) {
  const parsed = path.posix.parse(normalizeSourcePath(entrypointFileName));
  const name = parsed.name || "contract";
  return path.posix.join("output", `${name}.abi.json`);
}

function buildImportMappings(inputMappings) {
  if (inputMappings === undefined) {
    return [];
  }

  return Object.entries(inputMappings)
    .map(([prefix, target]) => ({
      prefix: normalizeSourcePath(prefix),
      target: normalizeSourcePath(target),
    }))
    .sort((left, right) => right.prefix.length - left.prefix.length);
}

function resolveSourcePath(requestedPath, sources, importMappings) {
  const sourcePath = normalizeSourcePath(requestedPath);
  const candidates = [sourcePath];

  for (const mapping of importMappings) {
    const suffix = mappedSuffix(sourcePath, mapping.prefix);
    if (suffix === undefined) {
      continue;
    }

    candidates.push(joinMappingTarget(mapping.target, suffix));
  }

  for (const candidate of candidates) {
    const normalizedCandidate = normalizeSourcePath(candidate);
    if (sources.has(normalizedCandidate)) {
      return normalizedCandidate;
    }

    if (!path.posix.extname(normalizedCandidate)) {
      const tolkCandidate = `${normalizedCandidate}.tolk`;
      if (sources.has(tolkCandidate)) {
        return tolkCandidate;
      }
    }
  }

  return sourcePath;
}

function mappedSuffix(sourcePath, prefix) {
  if (sourcePath === prefix) {
    return "";
  }

  const prefixWithSlash = `${prefix}/`;
  if (sourcePath.startsWith(prefixWithSlash)) {
    return sourcePath.slice(prefixWithSlash.length);
  }

  return undefined;
}

function joinMappingTarget(target, suffix) {
  if (suffix.length === 0) {
    return target;
  }

  return path.posix.join(target, suffix);
}
