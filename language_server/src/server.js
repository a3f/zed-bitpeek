import {
  ProposedFeatures,
  TextDocuments,
  createConnection,
} from "vscode-languageserver/node.js";
import { TextDocument } from "vscode-languageserver-textdocument";

const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);

function fromCharCode(code) {
  if (0 <= code && code <= 31) {
    return String.fromCharCode(code + 0x2400);
  } else if ((32 <= code && code <= 126) || (128 <= code && code <= 255)) {
    return String.fromCharCode(code);
  } else if (code == 127) {
    return "\u2421";
  }
}

function isPowerOfTwo(value) {
  return value > 0n && (value & (value - 1n)) === 0n;
}

function highestSetBit(value) {
  return value.toString(2).length - 1;
}

function formatSizeMacro(value) {
  if (value <= 512n) {
    return `SZ_${value}`;
  }

  const units = [
    [60n, "E"],
    [50n, "P"],
    [40n, "T"],
    [30n, "G"],
    [20n, "M"],
    [10n, "K"],
  ];

  for (const [shift, suffix] of units) {
    const unit = 1n << shift;
    if (value >= unit && value % unit === 0n) {
      const size = value / unit;
      if (size <= 512n) {
        return `SZ_${size}${suffix}`;
      }
    }
  }
}

function consecutiveSetBitRange(value) {
  const bits = value.toString(2);
  if (!/^1+0*$/.test(bits)) {
    return null;
  }

  return {
    highest: bits.length - 1,
    lowest: bits.length - bits.lastIndexOf("1") - 1,
  };
}

function formatMacro(value) {
  if (value <= 0n) {
    return null;
  }

  if (isPowerOfTwo(value)) {
    const bit = highestSetBit(value);
    const sizeMacro = formatSizeMacro(value);
    return sizeMacro ? `BIT(${bit})  /* ${sizeMacro} */` : `BIT(${bit})`;
  }

  const range = consecutiveSetBitRange(value);
  if (range) {
    return `GENMASK(${range.highest}, ${range.lowest})`;
  }

  return null;
}

function parseNumberAtPosition(doc, position) {
  const line = doc.getText({
    start: { line: position.line, character: 0 },
    end: { line: position.line, character: Number.MAX_SAFE_INTEGER },
  });
  let wordStart = position.character;
  while (wordStart >= 0 && line.charAt(wordStart).match(/['\w]/)) {
    wordStart--;
  }
  wordStart++;
  let wordEnd = position.character;
  while (wordEnd < line.length && line.charAt(wordEnd).match(/['\w]/)) {
    wordEnd++;
  }
  connection.console.log(`wordStart: ${wordStart}, wordEnd: ${wordEnd}`);
  if (wordStart >= wordEnd) {
    return null;
  }

  let word = line.substring(wordStart, wordEnd);
  let hasNegativePrefix = wordStart > 0 && line.charAt(wordStart - 1) == "-";
  connection.console.log(`word: ${word}`);
  if (word.charAt(0) == "-") {
    word = word.substring(1);
    wordStart++;
    hasNegativePrefix = true;
  }

  let num = 0;
  let bigNum = 0n;
  let hexs = "";
  let literalKind = "";
  if (word.match(/^(0[bB]['01]*[01]|0[bB][_01]*[01])$/)) {
    literalKind = "binary";
    connection.console.log("bin");
    for (let i = 0; i < word.length; i++) {
      let ch = word.charAt(i);
      if (ch == "b" || ch == "B" || ch == "'" || ch == "_") {
        continue;
      }
      const digit = ch - "0";
      num = num * 2 + digit;
      bigNum = bigNum * 2n + BigInt(digit);
    }
  } else if (word.match(/^(0[oO]?['0-7]*[0-7]|0[oO]?[_0-7]*[0-7])$/)) {
    literalKind = "octal";
    connection.console.log("oct");
    for (let i = 0; i < word.length; i++) {
      let ch = word.charAt(i);
      if (ch == "o" || ch == "O" || ch == "'" || ch == "_") {
        continue;
      }
      const digit = ch - "0";
      num = num * 8 + digit;
      bigNum = bigNum * 8n + BigInt(digit);
    }
  } else if (
    word.match(
      /^(0[xX]['0-9a-fA-F]*[0-9a-fA-F]|0[xX][_0-9a-fA-F]*[0-9a-fA-F])$/,
    )
  ) {
    literalKind = "hex";
    connection.console.log("hex");
    for (let i = 0; i < word.length; i++) {
      let ch = word.charAt(i);
      if (ch == "x" || ch == "X" || ch == "'" || ch == "_") {
        continue;
      }
      const digit =
        ch >= "0" && ch <= "9"
          ? ch - "0"
          : ch >= "a" && ch <= "f"
            ? ch.charCodeAt(0) - 87
            : ch.charCodeAt(0) - 55;
      num = num * 16 + digit;
      bigNum = bigNum * 16n + BigInt(digit);
    }
    hexs = word.substring(2);
  } else if (word.match(/^([0-9]|[1-9]['0-9]*[0-9]|[1-9][_0-9]*[0-9])$/)) {
    literalKind = "decimal";
    connection.console.log("dec");
    for (let i = 0; i < word.length; i++) {
      let ch = word.charAt(i);
      if (ch == "'" || ch == "_") {
        continue;
      }
      const digit = ch - "0";
      num = num * 10 + digit;
      bigNum = bigNum * 10n + BigInt(digit);
    }
  } else {
    return null;
  }

  hexs = hexs || num.toString(16);

  return {
    word,
    num,
    bigNum,
    hexs,
    literalKind,
    hasNegativePrefix,
    macro: formatMacro(bigNum),
    range: {
      start: {
        line: position.line,
        character: wordStart,
      },
      end: {
        line: position.line,
        character: wordEnd,
      },
    },
  };
}

function replacementAction(uri, range, replacement) {
  return {
    title: `Replace with ${replacement}`,
    kind: "refactor.rewrite",
    edit: {
      changes: {
        [uri]: [{ range, newText: replacement }],
      },
    },
  };
}

function macroReplacementActions(uri, parsed) {
  if (
    !parsed ||
    parsed.literalKind != "hex" ||
    parsed.hasNegativePrefix ||
    parsed.bigNum <= 0n
  ) {
    return [];
  }

  if (isPowerOfTwo(parsed.bigNum)) {
    const bit = highestSetBit(parsed.bigNum);
    const actions = [replacementAction(uri, parsed.range, `BIT(${bit})`)];
    const sizeMacro = formatSizeMacro(parsed.bigNum);
    if (sizeMacro) {
      actions.push(replacementAction(uri, parsed.range, sizeMacro));
    }
    return actions;
  }

  const range = consecutiveSetBitRange(parsed.bigNum);
  if (!range) {
    return [];
  }

  return [
    replacementAction(
      uri,
      parsed.range,
      `GENMASK(${range.highest}, ${range.lowest})`,
    ),
  ];
}

connection.onInitialize((params) => {
  return {
    capabilities: {
      hoverProvider: true,
      codeActionProvider: true,
    },
  };
});

connection.onHover((params) => {
  const doc = documents.get(params.textDocument.uri);
  if (!doc) {
    connection.console.error(`Document ${params.textDocument.uri} not found`);
    return null;
  }

  const parsed = parseNumberAtPosition(doc, params.position);
  if (!parsed) {
    return null;
  }

  const { word, num, hexs, macro, range } = parsed;
  const macroLine = macro ? `Macro:      ${macro}\n` : "";
  connection.console.log(`hex: ${hexs}`);
  let numInLE = 0;
  let ascii = "";
  for (let i = hexs.length - 2; i >= 0; i -= 2) {
    numInLE = numInLE * 256 + parseInt(hexs.substring(i, i + 2), 16);
  }
  if (hexs.length % 2 == 1) {
    const v = parseInt(hexs.substring(0, 1), 16);
    numInLE = numInLE * 256 + v;
    ascii = fromCharCode(v);
  }
  for (let i = hexs.length % 2; i < hexs.length; i += 2) {
    const v = parseInt(hexs.substring(i, i + 2), 16);
    connection.console.info(`v: ${v}`);
    ascii += fromCharCode(v);
  }
  connection.console.log(`numInLE: ${num}`);
  connection.console.log(`ascii: ${ascii}`);
  return {
    contents: {
      kind: "markdown",
      value: `[**HexPeek**](https://github.com/A-23187/zed-hexpeek) \`${word}\`
\`\`\`
Binary:      0b${num.toString(2)}
Octal:       0o${num.toString(8)}
Decimal:
  in BE:     ${num}
  in LE:     ${numInLE}
Hexadecimal: 0x${hexs}
${macroLine}Ascii:       ${ascii}
Time:
  in  S:     ${new Date(num * 1000).toISOString()}
  in MS:     ${new Date(num).toISOString()}
\`\`\`
`,
    },
    range,
  };
});

connection.onCodeAction((params) => {
  const doc = documents.get(params.textDocument.uri);
  if (!doc) {
    connection.console.error(`Document ${params.textDocument.uri} not found`);
    return [];
  }

  const parsed = parseNumberAtPosition(doc, params.range.start);
  return macroReplacementActions(params.textDocument.uri, parsed);
});

documents.listen(connection);
connection.listen();
