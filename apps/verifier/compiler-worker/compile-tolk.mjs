import path from "node:path";
import process from "node:process";

import { runTolkCompiler } from "@ton/tolk-js";

const SUPPORTED_LANGUAGE = "tolk";
const SUPPORTED_TOLK_VERSION = "1.4.1";

try {
  const input = JSON.parse(await readStdin());
  validateInput(input);

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
    writeOutput({ status: "compile_error", error: result.message });
  } else {
    writeOutput({
      status: "ok",
      code_hash: String(result.codeHashHex).toLowerCase(),
    });
  }
} catch (error) {
  writeOutput({
    status: "compile_error",
    error: error instanceof Error ? error.message : String(error),
  });
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }
  return Buffer.concat(chunks).toString("utf8");
}

function writeOutput(output) {
  process.stdout.write(`${JSON.stringify(output)}\n`);
}

function validateInput(input) {
  if (input.language !== SUPPORTED_LANGUAGE) {
    throw new Error(`unsupported language: ${input.language}`);
  }
  if (input.compiler_version !== SUPPORTED_TOLK_VERSION) {
    throw new Error(`unsupported Tolk compiler_version: ${input.compiler_version}`);
  }
  if (typeof input.entrypoint !== "string" || input.entrypoint.length === 0) {
    throw new Error("entrypoint is required");
  }
  if (!Array.isArray(input.sources) || input.sources.length === 0) {
    throw new Error("sources are required");
  }
  if (input.import_mappings !== undefined && !isPlainObject(input.import_mappings)) {
    throw new Error("import_mappings must be an object");
  }
}

function buildSourceMap(inputSources) {
  const sources = new Map();
  for (const source of inputSources) {
    if (typeof source?.path !== "string" || source.path.length === 0) {
      throw new Error("source path is required");
    }
    if (typeof source.content !== "string") {
      throw new Error(`source content is required: ${source.path}`);
    }

    const sourcePath = normalizeSourcePath(source.path);
    if (sources.has(sourcePath)) {
      throw new Error(`duplicate source path: ${source.path}`);
    }
    sources.set(sourcePath, source.content);
  }

  return sources;
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

function normalizeSourcePath(sourcePath) {
  if (typeof sourcePath !== "string" || sourcePath.length === 0) {
    throw new Error("source path is required");
  }
  if (sourcePath.includes("\\")) {
    throw new Error(`source path must use '/' separators: ${sourcePath}`);
  }
  if (path.posix.isAbsolute(sourcePath) || sourcePath.split("/").includes("..")) {
    throw new Error(`invalid source path: ${sourcePath}`);
  }

  const normalized = path.posix.normalize(sourcePath);
  if (
    normalized === "." ||
    normalized === ".." ||
    normalized.startsWith("../") ||
    path.posix.isAbsolute(normalized)
  ) {
    throw new Error(`invalid source path: ${sourcePath}`);
  }

  return normalized;
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
