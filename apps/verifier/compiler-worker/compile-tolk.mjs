import fs from "node:fs";
import path from "node:path";
import process from "node:process";

import { runTolkCompiler } from "@ton/tolk-js";

const SUPPORTED_LANGUAGE = "tolk";
const SUPPORTED_TOLK_VERSION = "1.4.1";

try {
  const input = JSON.parse(await readStdin());
  validateInput(input);

  const rootDir = fs.realpathSync(input.root_dir);
  const entrypointFileName = resolveInsideRoot(rootDir, input.entrypoint);

  const result = await runTolkCompiler({
    entrypointFileName,
    fsReadCallback: (requestedPath) => {
      const sourcePath = resolveInsideRoot(rootDir, requestedPath);
      return fs.readFileSync(sourcePath, "utf8");
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
  if (typeof input.root_dir !== "string" || input.root_dir.length === 0) {
    throw new Error("root_dir is required");
  }
  if (typeof input.entrypoint !== "string" || input.entrypoint.length === 0) {
    throw new Error("entrypoint is required");
  }
}

function resolveInsideRoot(rootDir, requestedPath) {
  const candidate = path.isAbsolute(requestedPath)
    ? path.resolve(requestedPath)
    : path.resolve(rootDir, requestedPath);
  const relative = path.relative(rootDir, candidate);

  if (relative === "" || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`invalid source path: ${requestedPath}`);
  }

  return candidate;
}
