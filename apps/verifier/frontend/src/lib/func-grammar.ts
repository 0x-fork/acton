const identifierPattern = String.raw`~?[a-zA-Z_][a-zA-Z0-9_']*`
const typePattern = String.raw`\(\)|int|cell|slice|builder|tuple|cont|var|void`

export const funcGrammar = {
  $schema: "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
  name: "func",
  scopeName: "source.func",
  aliases: ["fun-c"],
  foldingStartMarker: String.raw`\{\s*$`,
  foldingStopMarker: String.raw`^\s*\}`,
  fileTypes: ["fc", "func"],
  patterns: [
    {include: "#preprocessor"},
    {include: "#comments"},
    {include: "#strings"},
    {
      name: "constant.numeric",
      match: String.raw`\b-?(?:0x[\da-fA-F]+|0b[01]+|\d+)\b`,
    },
    {
      name: "keyword.control",
      match:
        String.raw`\b(if|ifnot|else|elseif|while|do|repeat|return|throw|try|catch)\b`,
    },
    {
      name: "keyword.operator",
      match:
        String.raw`~|\+|-|\*|/|%|\?|:|,|;|\(|\)|\[|\]|\{|\}|=|<|>|!|&|\||\^|==|!=|<=|>=|<<|>>|&&|\|\||~/|\^/|\+=|-=|\*=|/=|%=|&=|\|=|\^=|->|=>`,
    },
    {
      name: "keyword.other",
      match:
        String.raw`\b(asm|const|forall|global|impure|inline|inline_ref|method_id|operator|infix|infixl|infixr|prefix|builtin|auto_apply|true|false|null)\b`,
    },
    {
      captures: {
        "1": {name: "storage.type"},
        "2": {name: "entity.name.function"},
      },
      match: String.raw`\b(${typePattern})\s+(${identifierPattern})(?=\s*\()`,
    },
    {
      name: "storage.type",
      match: String.raw`\b(?:${typePattern})\b|\(\)`,
    },
    {
      name: "constant.other",
      match: String.raw`\b[A-Z][A-Z0-9_]{2,}\b`,
    },
    {
      name: "entity.name.function",
      match: `(${identifierPattern})(?=\\s*\\()`,
    },
    {
      name: "variable.name",
      match: identifierPattern,
    },
  ],
  repository: {
    comments: {
      patterns: [
        {
          name: "comment.line.semicolon.func",
          match: ";;.*$",
        },
        {
          name: "comment.block.func",
          begin: String.raw`\{-`,
          end: String.raw`-\}`,
        },
      ],
    },
    preprocessor: {
      begin: String.raw`^\s*(#(?:include|pragma|define|ifdef|ifndef|else|endif))\b`,
      beginCaptures: {
        "1": {name: "keyword.control.directive"},
      },
      end: "$",
      name: "meta.preprocessor.func",
      patterns: [
        {include: "#strings"},
        {
          name: "constant.other",
          match: String.raw`\b[A-Z_][A-Z0-9_]*\b`,
        },
      ],
    },
    strings: {
      patterns: [
        {
          name: "string.quoted.double.func",
          begin: '"',
          end: '"',
          patterns: [
            {
              name: "constant.character.escape.func",
              match: String.raw`\\.`,
            },
          ],
        },
      ],
    },
  },
}
