import { readFileSync, writeFileSync } from "node:fs";

const nodeModulesUrl = new URL("../node_modules/", import.meta.url);
const FUNC_VERSIONS = ["0.4.5", "0.4.6", "0.4.6-wasmfix.0"];
const TACT_VERSIONS = [
  "1.0.0",
  "1.1.0",
  "1.1.1",
  "1.1.2",
  "1.1.3",
  "1.1.4",
  "1.1.5",
  "1.2.0",
  "1.3.0",
  "1.3.1",
  "1.4.0",
  "1.4.1",
  "1.4.2",
  "1.4.3",
  "1.4.4",
  "1.5.0",
  "1.5.1",
  "1.5.2",
  "1.5.3",
  "1.5.4",
  "1.6.2",
  "1.6.3",
  "1.6.4",
  "1.6.5",
  "1.6.6",
  "1.6.7",
  "1.6.10",
  "1.6.11",
  "1.6.12",
  "1.6.13",
];
const TOLK_VERSIONS = [
  "0.6.0",
  "0.7.0",
  "0.8.0",
  "0.9.0",
  "0.10.0",
  "0.11.0",
  "0.12.0",
  "0.13.0",
  "0.99.0",
  "1.0.0",
  "1.1.0",
  "1.2.0",
];
const compilerRuntimePaths = [
  ...FUNC_VERSIONS.map((version) => `func-${version}/dist/funcfiftlib.js`),
  ...TACT_VERSIONS.map((version) => `tact-${version}/dist/func/funcfiftlib.js`),
  ...TOLK_VERSIONS.map((version) => `tolk-${version}/dist/tolkfiftlib.js`),
];

for (const runtimePath of compilerRuntimePaths) {
  const runtimeUrl = new URL(runtimePath, nodeModulesUrl);
  const current = readFileSync(runtimeUrl, "utf8");
  const patched = current
    .replaceAll('process["binding"]("constants")', 'require("node:fs").constants')
    .replaceAll('process.binding("constants")', 'require("node:fs").constants');

  if (patched !== current) {
    writeFileSync(runtimeUrl, patched);
  }
}
