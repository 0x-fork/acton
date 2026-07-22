import path from "node:path";
import { getMethodId } from "@ton/core";

import { normalizeSourcePath } from "./common.mjs";
import { importTolk, SUPPORTED_TOLK_VERSIONS } from "./registry.mjs";

/** @typedef {import("@ton/core").ABIReceiver} ABIReceiver */
/** @typedef {import("@ton/core").ABIGetter} ABIGetter */
/** @typedef {import("@ton/core").ABIType} ABIType */
/** @typedef {import("@ton/core").ABITypeRef} ABITypeRef */
/** @typedef {import("@ton/core").ContractABI} ContractABI */
/** @typedef {{ path: string, content: string }} GeneratedSource */
/** @typedef {{ name?: string, abi?: string | ContractABI }} TactPackage */
/** @typedef {{ contractName: string, getters: Map<string, ABIGetter>, source: string }} GeneratedTolkSource */
/** @typedef {(name: string, selected: Set<string>, serializable?: boolean) => void} CollectStruct */

const PRIMITIVE_TYPES = new Set([
  "address",
  "bool",
  "builder",
  "cell",
  "fixed-bytes",
  "int",
  "slice",
  "string",
  "uint",
]);

const TOLK_KEYWORDS = new Set([
  "asm",
  "assert",
  "break",
  "catch",
  "const",
  "continue",
  "contract",
  "do",
  "else",
  "enum",
  "export",
  "extern",
  "false",
  "for",
  "fun",
  "get",
  "global",
  "if",
  "import",
  "in",
  "inline",
  "lazy",
  "match",
  "mutate",
  "null",
  "operator",
  "redef",
  "repeat",
  "return",
  "self",
  "struct",
  "throw",
  "true",
  "try",
  "type",
  "val",
  "var",
  "while",
]);

const TOLK_GETTER_NAME_CONFLICTS = new Set(["address", "random"]);

/**
 * @param {TactPackage} tactPackage
 * @param {GeneratedSource[]} generatedSources
 * @returns {Promise<GeneratedSource[] | undefined>}
 */
export async function generatedTolkAbiSources(tactPackage, generatedSources) {
  try {
    const tactAbi = findMainTactAbi(tactPackage, generatedSources);
    if (tactAbi === undefined) {
      return undefined;
    }

    const compilerVersion = SUPPORTED_TOLK_VERSIONS[0];
    if (compilerVersion === undefined) {
      return undefined;
    }

    const generated = tactAbiToTolk(tactAbi);
    const typesPath = path.posix.join(
      "output",
      `${generated.contractName}.types.tolk`,
    );
    const { runTolkCompiler } = await importTolk(compilerVersion);
    const result = await runTolkCompiler({
      entrypointFileName: typesPath,
      allowNoEntrypoint: true,
      pathMappings: {},
      fsReadCallback: (requestedPath) => {
        if (normalizeSourcePath(requestedPath) === typesPath) {
          return generated.source;
        }
        throw new Error(
          `generated Tact ABI source was not provided: ${requestedPath}`,
        );
      },
    });

    if (
      result.status === "error" ||
      result.abiJson === undefined ||
      result.abiJson === null
    ) {
      return undefined;
    }

    // Tolk emits ABI entries only for `get fun`, whose method ID is derived
    // from the function name. Some valid Tact names collide with Tolk symbols
    // and need a temporary suffix. Restore their names and locally calculated
    // method IDs after Tolk has produced the parameter and return type indices.
    for (const getter of result.abiJson.get_methods ?? []) {
      const tactGetter = generated.getters.get(getter.name);
      if (tactGetter === undefined) {
        continue;
      }
      getter.name = tactGetter.name;
      getter.tvm_method_id = Number.isInteger(tactGetter.methodId)
        ? tactGetter.methodId
        : getMethodId(tactGetter.name);
    }

    return [
      { path: typesPath, content: generated.source },
      {
        path: path.posix.join("output", `${generated.contractName}.abi.json`),
        content: `${JSON.stringify(result.abiJson, null, 2)}\n`,
      },
    ];
  } catch {
    return undefined;
  }
}

/**
 * @param {ContractABI} tactAbi
 * @returns {GeneratedTolkSource}
 */
function tactAbiToTolk(tactAbi) {
  if (
    tactAbi === null ||
    typeof tactAbi !== "object" ||
    Array.isArray(tactAbi)
  ) {
    throw new Error("Tact ABI must be a JSON object");
  }
  if (typeof tactAbi.name !== "string" || tactAbi.name.length === 0) {
    throw new Error("Tact ABI contract name is required");
  }

  const usedTypeNames = new Set();
  const contractName = uniqueIdentifier(
    tactAbi.name,
    "Contract",
    usedTypeNames,
  );
  const types = Array.isArray(tactAbi.types) ? tactAbi.types : [];
  const typeNames = new Map();
  for (const type of types) {
    if (typeof type?.name !== "string" || type.name.length === 0) {
      throw new Error("Tact ABI type name is required");
    }
    const originalName = identifier(type.name, "Type");
    const prefixedName = originalName.startsWith(contractName)
      ? originalName
      : `${contractName}${originalName}`;
    typeNames.set(
      type.name,
      uniqueIdentifier(prefixedName, "Type", usedTypeNames),
    );
  }

  const typeName = (name) => typeNames.get(name) ?? identifier(name, "Type");
  const typesByName = new Map(types.map((type) => [type.name, type]));
  const selectedTypes = new Set();

  /**
   * @param {ABITypeRef} type
   * @param {Set<string>} selected
   * @param {boolean} [serializable]
   * @param {Set<string>} [visiting]
   */
  function collectTypeReference(
    type,
    selected,
    serializable = false,
    visiting = new Set(),
  ) {
    renderType(type, typeName);
    if (
      serializable &&
      type.kind === "simple" &&
      (type.type === "slice" || type.type === "builder") &&
      type.format !== "remainder"
    ) {
      throw new Error(
        `Tact ABI type cannot be serialized by Tolk: ${type.type}`,
      );
    }
    if (type.kind === "dict") {
      if (!PRIMITIVE_TYPES.has(type.key)) {
        collectStruct(type.key, selected, true, visiting);
      }
      if (!PRIMITIVE_TYPES.has(type.value)) {
        collectStruct(type.value, selected, true, visiting);
      }
      return;
    }
    if (!PRIMITIVE_TYPES.has(type.type)) {
      collectStruct(
        type.type,
        selected,
        serializable || type.format === "ref",
        visiting,
      );
    }
  }

  /**
   * @param {string} name
   * @param {Set<string>} selected
   * @param {boolean} [serializable]
   * @param {Set<string>} [visiting]
   */
  function collectStruct(
    name,
    selected,
    serializable = false,
    visiting = new Set(),
  ) {
    const visitKey = `${serializable ? "serialized" : "stack"}:${name}`;
    if (visiting.has(visitKey)) {
      return;
    }
    const type = typesByName.get(name);
    if (type === undefined) {
      throw new Error(`Tact ABI type is not defined: ${name}`);
    }
    renderStruct(type, typeName);
    visiting.add(visitKey);
    selected.add(name);
    for (const field of Array.isArray(type.fields) ? type.fields : []) {
      collectTypeReference(field?.type, selected, serializable, visiting);
    }
    visiting.delete(visitKey);
  }

  const contractProperties = [];
  const storageType = `${tactAbi.name}$Data`;
  const storageName = typeNames.get(storageType);
  if (storageName !== undefined) {
    collectStruct(storageType, selectedTypes, true);
    contractProperties.push(`    storage: ${storageName}`);
  }

  const receivers = Array.isArray(tactAbi.receivers) ? tactAbi.receivers : [];
  const internalMessages = typedReceiverNames(
    receivers,
    "internal",
    typeName,
    selectedTypes,
    collectStruct,
  );
  const externalMessages = typedReceiverNames(
    receivers,
    "external",
    typeName,
    selectedTypes,
    collectStruct,
  );
  const aliases = [];
  addMessageProperty({
    property: "incomingMessages",
    suffix: "IncomingMessage",
    messages: internalMessages,
    contractName,
    contractProperties,
    aliases,
    usedTypeNames,
  });
  addMessageProperty({
    property: "incomingExternal",
    suffix: "IncomingExternalMessage",
    messages: externalMessages,
    contractName,
    contractProperties,
    aliases,
    usedTypeNames,
  });

  const errors = renderErrors(tactAbi.errors, contractName, usedTypeNames);
  if (errors !== undefined) {
    contractProperties.push(`    thrownErrors: ${errors.name}`);
  }

  const lines = [
    "// Generated from the verified Tact contract ABI.",
    `contract ${contractName} {`,
    ...contractProperties,
    "}",
  ];

  const rawGetters = Array.isArray(tactAbi.getters) ? tactAbi.getters : [];
  for (const getter of rawGetters) {
    for (const argument of Array.isArray(getter?.arguments)
      ? getter.arguments
      : []) {
      collectTypeReference(argument?.type, selectedTypes);
    }
    if (getter?.returnType) {
      collectTypeReference(getter.returnType, selectedTypes);
    }
  }

  for (const type of types.filter((type) => selectedTypes.has(type.name))) {
    lines.push("", renderStruct(type, typeName));
  }
  for (const alias of aliases) {
    lines.push("", alias);
  }
  if (errors !== undefined) {
    lines.push("", errors.source);
  }

  const getters = new Map();
  const usedGetterNames = new Set();
  for (const getter of rawGetters) {
    if (typeof getter?.name !== "string" || getter.name.length === 0) {
      throw new Error("Tact ABI getter name is required");
    }
    const sourceGetterName = TOLK_GETTER_NAME_CONFLICTS.has(getter.name)
      ? `${getter.name}_`
      : getter.name;
    const getterName = uniqueIdentifier(
      sourceGetterName,
      "getter",
      usedGetterNames,
    );
    const usedArgumentNames = new Set();
    const argumentsSource = (
      Array.isArray(getter.arguments) ? getter.arguments : []
    )
      .map((argument, index) => {
        const name = uniqueIdentifier(
          argument?.name,
          `argument${index + 1}`,
          usedArgumentNames,
        );
        return `${name}: ${renderType(argument?.type, typeName)}`;
      })
      .join(", ");
    const returnType = getter.returnType
      ? renderType(getter.returnType, typeName)
      : "void";
    const tactMethodId = Number.isInteger(getter.methodId)
      ? getter.methodId
      : getMethodId(getter.name);
    lines.push("");
    if (getterName !== getter.name) {
      lines.push(`// Tact getter name: ${getter.name}`);
    }
    if (tactMethodId !== getMethodId(getterName)) {
      lines.push(`// Tact method ID: ${tactMethodId}`);
    }
    lines.push(
      `get fun ${getterName}(${argumentsSource}): ${returnType} {`,
      "    throw 0;",
      "}",
    );
    getters.set(getterName, getter);
  }

  lines.push("");
  return { contractName, getters, source: lines.join("\n") };
}

/**
 * @param {TactPackage} tactPackage
 * @param {GeneratedSource[]} generatedSources
 * @returns {ContractABI | undefined}
 */
function findMainTactAbi(tactPackage, generatedSources) {
  const packageName = tactPackage?.name;
  for (const source of generatedSources) {
    if (!source.path.endsWith(".abi")) {
      continue;
    }
    const abi = JSON.parse(source.content);
    if (abi?.name === packageName) {
      return abi;
    }
  }

  if (typeof tactPackage?.abi === "string") {
    const abi = JSON.parse(tactPackage.abi);
    if (abi?.name === packageName) {
      return abi;
    }
  } else if (tactPackage?.abi?.name === packageName) {
    return tactPackage.abi;
  }

  return undefined;
}

/**
 * @param {ABIReceiver[]} receivers
 * @param {"internal" | "external"} receiverKind
 * @param {(name: string) => string} typeName
 * @param {Set<string>} selectedTypes
 * @param {CollectStruct} collectStruct
 * @returns {string[]}
 */
function typedReceiverNames(
  receivers,
  receiverKind,
  typeName,
  selectedTypes,
  collectStruct,
) {
  const names = [];
  const seen = new Set();
  for (const receiver of receivers) {
    const message = receiver?.message;
    if (
      receiver?.receiver !== receiverKind ||
      message?.kind !== "typed" ||
      typeof message.type !== "string"
    ) {
      continue;
    }
    const messageTypes = new Set(selectedTypes);
    try {
      collectStruct(message.type, messageTypes, true);
    } catch {
      continue;
    }
    selectedTypes.clear();
    for (const selected of messageTypes) {
      selectedTypes.add(selected);
    }

    const name = typeName(message.type);
    if (!seen.has(name)) {
      names.push(name);
      seen.add(name);
    }
  }
  return names;
}

/**
 * @param {{
 *   property: string,
 *   suffix: string,
 *   messages: string[],
 *   contractName: string,
 *   contractProperties: string[],
 *   aliases: string[],
 *   usedTypeNames: Set<string>,
 * }} options
 */
function addMessageProperty({
  property,
  suffix,
  messages,
  contractName,
  contractProperties,
  aliases,
  usedTypeNames,
}) {
  if (messages.length === 0) {
    return;
  }
  if (messages.length === 1) {
    contractProperties.push(`    ${property}: ${messages[0]}`);
    return;
  }

  const aliasName = uniqueIdentifier(
    `${contractName}${suffix}`,
    suffix,
    usedTypeNames,
  );
  contractProperties.push(`    ${property}: ${aliasName}`);
  aliases.push(
    `type ${aliasName} =\n${messages.map((name) => `    | ${name}`).join("\n")}`,
  );
}

/**
 * @param {ABIType} type
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderStruct(type, typeName) {
  const name = typeName(type.name);
  let header = "";
  if (type.header !== null && type.header !== undefined) {
    if (
      !Number.isInteger(type.header) ||
      type.header < 0 ||
      type.header > 0xffffffff
    ) {
      throw new Error(
        `invalid Tact ABI header for ${type.name}: ${type.header}`,
      );
    }
    header = ` (0x${type.header.toString(16).padStart(8, "0")})`;
  }

  const usedFieldNames = new Set();
  const fields = (Array.isArray(type.fields) ? type.fields : []).map(
    (field, index) => {
      const fieldName = uniqueIdentifier(
        field?.name,
        `field${index + 1}`,
        usedFieldNames,
      );
      return `    ${fieldName}: ${renderType(field?.type, typeName)}`;
    },
  );
  return [`struct${header} ${name} {`, ...fields, "}"].join("\n");
}

/**
 * @param {ABITypeRef} type
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderType(type, typeName) {
  if (type === null || typeof type !== "object" || Array.isArray(type)) {
    throw new Error("invalid Tact ABI type reference");
  }
  if (type.kind === "dict") {
    const key = renderDictionaryPart(type.key, type.keyFormat, typeName);
    const value = renderDictionaryPart(type.value, type.valueFormat, typeName);
    return `map<${key}, ${value}>`;
  }
  if (type.kind !== "simple" || typeof type.type !== "string") {
    throw new Error(`unsupported Tact ABI type kind: ${String(type.kind)}`);
  }

  let rendered;
  if (type.format === "remainder") {
    rendered = "RemainingBitsAndRefs";
  } else if (type.format === "ref") {
    rendered = `Cell<${typeName(type.type)}>`;
  } else if (type.type === "fixed-bytes") {
    if (!Number.isInteger(type.format) || type.format <= 0) {
      throw new Error(`invalid fixed-bytes format: ${String(type.format)}`);
    }
    rendered = `bits${type.format * 8}`;
  } else if (type.type === "int" || type.type === "uint") {
    rendered = renderInteger(type.type, type.format);
  } else if (PRIMITIVE_TYPES.has(type.type)) {
    rendered = type.type;
  } else {
    rendered = typeName(type.type);
  }

  return type.optional === true ? `${rendered}?` : rendered;
}

/**
 * @param {string} type
 * @param {string | number | boolean | null | undefined} format
 * @param {(name: string) => string} typeName
 * @returns {string}
 */
function renderDictionaryPart(type, format, typeName) {
  if (typeof type !== "string") {
    throw new Error("invalid Tact ABI dictionary type");
  }
  if (type === "int" || type === "uint") {
    return renderInteger(type, format);
  }
  if (type === "cell") {
    if (format !== undefined && format !== null && format !== "ref") {
      throw new Error(`unsupported Tact ABI dictionary cell format: ${format}`);
    }
    return "cell";
  }
  if (PRIMITIVE_TYPES.has(type)) {
    if (format === "ref") {
      throw new Error(`unsupported Tact ABI dictionary ref type: ${type}`);
    }
    return type;
  }
  if (format === undefined || format === null || format === "ref") {
    return `Cell<${typeName(type)}>`;
  }
  throw new Error(`unsupported Tact ABI dictionary format: ${format}`);
}

/**
 * @param {string} type
 * @param {string | number | boolean | null | undefined} format
 * @returns {string}
 */
function renderInteger(type, format) {
  if (format === undefined || format === null || format === 257) {
    return "int257";
  }
  if (
    format === "coins" ||
    format === "varint16" ||
    format === "varint32" ||
    format === "varuint16" ||
    format === "varuint32"
  ) {
    return format;
  }
  if (Number.isInteger(format) && format > 0 && format <= 256) {
    return `${type}${format}`;
  }
  throw new Error(`unsupported Tact ABI integer format: ${String(format)}`);
}

/**
 * @param {ContractABI["errors"]} rawErrors
 * @param {string} contractName
 * @param {Set<string>} usedTypeNames
 * @returns {{ name: string, source: string } | undefined}
 */
function renderErrors(rawErrors, contractName, usedTypeNames) {
  if (
    rawErrors === null ||
    typeof rawErrors !== "object" ||
    Array.isArray(rawErrors)
  ) {
    return undefined;
  }
  const entries = Object.entries(rawErrors)
    .map(([code, value]) => ({ code: Number(code), message: value?.message }))
    .filter(
      ({ code, message }) =>
        Number.isInteger(code) && typeof message === "string",
    )
    .sort((left, right) => left.code - right.code);
  if (entries.length === 0) {
    return undefined;
  }

  const name = uniqueIdentifier(
    `${contractName}Errors`,
    "TactErrors",
    usedTypeNames,
  );
  const usedMembers = new Set();
  const lines = [`enum ${name} {`];
  for (const entry of entries) {
    const comment = entry.message.replace(/[\r\n]+/g, " ").trim();
    if (comment.length > 0) {
      lines.push(`    /// ${comment}`);
    }
    const suggestedName = entry.message
      .match(/[A-Za-z0-9]+/g)
      ?.map((part) => `${part[0].toUpperCase()}${part.slice(1)}`)
      .join("");
    const member = uniqueIdentifier(
      suggestedName,
      `Error${entry.code}`,
      usedMembers,
    );
    lines.push(`    ${member} = ${entry.code}`);
  }
  lines.push("}");
  return { name, source: lines.join("\n") };
}

/**
 * @param {unknown} value
 * @param {string} fallback
 * @param {Set<string>} used
 * @returns {string}
 */
function uniqueIdentifier(value, fallback, used) {
  const base = identifier(value, fallback);
  let candidate = base;
  let suffix = 2;
  while (used.has(candidate)) {
    candidate = `${base}${suffix}`;
    suffix += 1;
  }
  used.add(candidate);
  return candidate;
}

/**
 * @param {unknown} value
 * @param {string} fallback
 * @returns {string}
 */
function identifier(value, fallback) {
  const text = typeof value === "string" ? value : "";
  let result = text.replace(/\$+([A-Za-z0-9_])/g, (_, next) =>
    next.toUpperCase(),
  );
  result = result.replace(/[^A-Za-z0-9_]/g, "_");
  if (result.length === 0) {
    result = fallback;
  }
  if (!/^[A-Za-z_]/.test(result)) {
    result = `_${result}`;
  }
  if (TOLK_KEYWORDS.has(result)) {
    result = `${result}_`;
  }
  return result;
}
