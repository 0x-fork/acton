const identifierPattern = "`[^`]+`|[a-zA-Z$_][a-zA-Z0-9$_]*"
const genericCallPattern = String.raw`\s*(?:<[^>]+>)?\s*\(`
const propertyNamePattern = `(${identifierPattern})(?=\\s*:)`

export const tactGrammar = {
  $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  name: "tact",
  scopeName: "source.tact",
  foldingStartMarker: String.raw`\{\s*$`,
  foldingStopMarker: String.raw`^\s*\}`,
  fileTypes: ["tact"],
  patterns: [
    {include: "#declarationBody"},
    {include: "#objectLiteral"},
    {
      name: "comment.line.double-slash",
      match: "//(.*)",
    },
    {
      name: "comment.block",
      begin: String.raw`/\*`,
      end: String.raw`\*/`,
    },
    {
      name: "string.quoted.double.tact",
      begin: '"',
      end: '"',
      patterns: [
        {
          name: "constant.character.escape.tact",
          match: String.raw`\\([nrt0\\'"u]|u[0-9a-fA-F]{4})`,
        },
      ],
    },
    {
      name: "constant.numeric",
      match: String.raw`\b(-?([\d]+|0x[\da-fA-F]+|0b[01]+))\b`,
    },
    {
      name: "keyword.control",
      match:
        String.raw`\b(do|if|else|while|repeat|foreach|try|catch|return|throw|require|asm)\b`,
    },
    {
      name: "keyword.operator",
      match:
        String.raw`\+|-|\*|/|%|\?|:|,|;|\.|\(|\)|\[|\]|\{|\}|=|<|>|!|&|\||\^|==|!=|<=|>=|<<|>>|&&|\|\||~/|\^/|\+=|-=|\*=|/=|%=|&=|\|=|\^=|->|=>|\?\?`,
    },
    {
      name: "keyword.other",
      match:
        String.raw`\b(import|extends|with|as|map|let|mutates|native|override|abstract|virtual|true|false|null|self|initOf|codeOf|sender|context|myBalance)\b`,
    },
    {
      name: "storage.modifier",
      match:
        String.raw`\b(contract|trait|struct|message|primitive|fun|get|receive|external|bounced|init|const)\b`,
    },
    {
      name: "storage.type",
      match:
        String.raw`\b(Int|Bool|Cell|Slice|Builder|Address|String|StringBuilder|StateInit|SendParameters|Context|Map|void|coins|int\d+|uint\d+)\b`,
    },
    {
      name: "entity.name.type",
      match: String.raw`@\w+`,
    },
    {
      name: "entity.name.function.method",
      match: `(?<=\\.)(${identifierPattern})(?=${genericCallPattern})`,
    },
    {
      name: "property",
      match: `(?<=\\.)(${identifierPattern})(?!${genericCallPattern})`,
    },
    {
      name: "entity.name.function",
      match: `(${identifierPattern})(?=${genericCallPattern})`,
    },
    {
      name: "entity.name.type",
      match: String.raw`\b[A-Z][a-zA-Z0-9_]*\b`,
    },
    {
      name: "variable.name",
      match: identifierPattern,
    },
  ],
  repository: {
    declarationBody: {
      begin: String.raw`\b(contract|trait|struct|message)\b[^{]*\{`,
      beginCaptures: {
        "1": {name: "storage.modifier"},
      },
      end: String.raw`\}`,
      patterns: [
        {
          match: `^\\s*${propertyNamePattern}`,
          captures: {
            "1": {name: "meta.property-name"},
          },
        },
        {include: "#objectLiteral"},
        {include: "$self"},
      ],
    },
    objectLiteral: {
      begin: String.raw`(?:(?:=|:|,)\s*|return\s+|\(\s*)(?:[A-Z][a-zA-Z0-9_]*\s*)?\{`,
      end: String.raw`\}`,
      patterns: [
        {
          match: `(?:^\\s*|(?<=[{,])\\s*)${propertyNamePattern}`,
          captures: {
            "1": {name: "meta.property-name"},
          },
        },
        {include: "$self"},
      ],
    },
  },
}
