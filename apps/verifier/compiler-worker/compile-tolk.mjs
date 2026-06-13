import path from "node:path";
import process from "node:process";

import { runTolkCompiler } from "@ton/tolk-js";

const SUPPORTED_LANGUAGE = "tolk";
const SUPPORTED_TOLK_VERSION = "1.4.1";

try {
  const input = JSON.parse(await readStdin());
  validateInput(input);

  const sources = buildSourceMap(input.sources);
  const entrypointFileName = normalizeSourcePath(input.entrypoint);

  const result = await runTolkCompiler({
    entrypointFileName,
    fsReadCallback: (requestedPath) => {
      const sourcePath = normalizeSourcePath(requestedPath);
      const content = sources.get(sourcePath);
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
