import { parse } from "luaparse";
import type { Token } from "luaparse";
import type { CodeEdit } from "./live-code-sync";

export interface LuaFormatOptions {
  indentUnit?: string;
  from?: number;
  to?: number;
}

export interface LuaFormatResult {
  code: string;
  changed: boolean;
  edits: CodeEdit[];
  error?: string;
}

interface SourceRange {
  range?: [number, number];
}

interface SourceLine {
  from: number;
  to: number;
}

const parserOptions = {
  luaVersion: "LuaJIT" as const,
  extendedIdentifiers: true,
  encodingMode: "none" as const,
};

function sourceLines(source: string): SourceLine[] {
  const lines: SourceLine[] = [];
  const lineBreak = /\r\n|\n\r|[\r\n]/g;
  let from = 0;
  for (let match = lineBreak.exec(source); match; match = lineBreak.exec(source)) {
    lines.push({ from, to: match.index });
    from = match.index + match[0].length;
  }
  lines.push({ from, to: source.length });
  return lines;
}

function closes(value: string): boolean {
  return value === "end" || value === "until" || value === "else" || value === "elseif"
    || value === ")" || value === "]" || value === "}";
}

function opens(value: string): boolean {
  return value === "function" || value === "then" || value === "do" || value === "repeat"
    || value === "else" || value === "(" || value === "[" || value === "{";
}


export function formatLuaCode(source: string, options: LuaFormatOptions = {}): LuaFormatResult {
  const unchanged = (error?: string): LuaFormatResult => ({ code: source, changed: false, edits: [], error });
  const indentUnit = options.indentUnit ?? "    ";
  if (!/^[ \t]+$/.test(indentUnit) || indentUnit.length > 16) {
    return unchanged("Formatting needs an indentation unit of 1 to 16 spaces or tabs.");
  }

  const protectedRanges: [number, number][] = [];
  const tokens: Token[] = [];
  try {
    const chunk = parse(source, {
      ...parserOptions,
      ranges: true,
      comments: true,
      onCreateNode(node) {
        const range = (node as typeof node & SourceRange).range;
        if (node.type === "StringLiteral" && range) protectedRanges.push(range);
      },
    });
    for (const comment of chunk.comments ?? []) {
      const range = (comment as typeof comment & SourceRange).range;
      if (range) protectedRanges.push(range);
    }

    const lexicalSource = source.startsWith("#!")
      ? source.replace(/^[^\r\n]*/, (line) => " ".repeat(line.length))
      : source;
    const lexer = parse(lexicalSource, { ...parserOptions, wait: true, comments: false });
    for (let token = lexer.lex(); token.range[0] < source.length; token = lexer.lex()) tokens.push(token);
  } catch (error) {
    const problem = error as { index?: number; message?: string } | null;
    if (typeof problem?.index !== "number" || !Number.isFinite(problem.index)) {
      return unchanged("Unable to format Lua. The document may be too deeply nested.");
    }
    const message = typeof problem.message === "string" ? problem.message : "Invalid Lua syntax";
    return unchanged(`Fix the Lua syntax before formatting: ${message.replace(/^\[\d+:\d+\]\s*/, "")}`);
  }

  const selectionFrom = Math.min(options.from ?? 0, options.to ?? source.length);
  const selectionTo = Math.max(options.from ?? 0, options.to ?? source.length);
  const hasSelection = options.from !== undefined && options.to !== undefined && selectionTo > selectionFrom;
  const edits: CodeEdit[] = [];
  let depth = 0;
  let tokenIndex = 0;
  protectedRanges.sort((a, b) => a[0] - b[0]);
  let protectedIndex = 0;

  for (const line of sourceLines(source)) {
    while (protectedIndex < protectedRanges.length && protectedRanges[protectedIndex][1] <= line.from) protectedIndex++;
    const protectedRange = protectedRanges[protectedIndex];
    const protectedPrefix = protectedRange && protectedRange[0] < line.from && line.from < protectedRange[1];
    const lineTokens: Token[] = [];
    while (tokenIndex < tokens.length && tokens[tokenIndex].range[0] < line.to) {
      const token = tokens[tokenIndex++];
      if (token.range[0] >= line.from) lineTokens.push(token);
    }

    let leadingClosers = 0;
    let prefixEnd = line.from;
    for (const token of lineTokens) {
      if (!closes(token.value) || !/^[ \t]*$/.test(source.slice(prefixEnd, token.range[0]))) break;
      leadingClosers++;
      prefixEnd = token.range[1];
      if (token.value === "else" || token.value === "elseif" || token.value === "until") break;
    }

    const selected = !hasSelection || (line.to >= selectionFrom && line.from < selectionTo);
    if (selected && !protectedPrefix) {
      const text = source.slice(line.from, line.to);
      const originalIndent = /^[ \t]*/.exec(text)![0];
      const desiredIndent = text.length === originalIndent.length
        ? ""
        : indentUnit.repeat(Math.max(0, depth - leadingClosers));
      if (desiredIndent !== originalIndent) {
        edits.push({ from: line.from, to: line.from + originalIndent.length, insert: desiredIndent });
      }
    }

    for (const token of lineTokens) {
      if (closes(token.value)) depth = Math.max(0, depth - 1);
      if (opens(token.value)) depth++;
    }
  }

  if (!edits.length) return unchanged();
  let code = source;
  for (let index = edits.length - 1; index >= 0; index--) {
    const edit = edits[index];
    code = code.slice(0, edit.from) + edit.insert + code.slice(edit.to);
  }
  return { code, changed: true, edits };
}

const indentationCanonicalCache = new Map<string, string | null>();

function canonicalIndentation(source: string): string | null {
  if (indentationCanonicalCache.has(source)) {
    const cached = indentationCanonicalCache.get(source)!;
    indentationCanonicalCache.delete(source);
    indentationCanonicalCache.set(source, cached);
    return cached;
  }
  const result = formatLuaCode(source);
  const canonical = result.error ? null : result.code;
  indentationCanonicalCache.set(source, canonical);
  if (indentationCanonicalCache.size > 4) {
    const oldest = indentationCanonicalCache.keys().next().value;
    if (oldest !== undefined) indentationCanonicalCache.delete(oldest);
  }
  return canonical;
}

function matchesApartFromLinePrefixes(left: string, right: string): boolean {
  let leftIndex = 0;
  let rightIndex = 0;
  let lineStart = true;
  for (;;) {
    if (lineStart) {
      while (left.charCodeAt(leftIndex) === 32 || left.charCodeAt(leftIndex) === 9) leftIndex += 1;
      while (right.charCodeAt(rightIndex) === 32 || right.charCodeAt(rightIndex) === 9) rightIndex += 1;
    }
    if (leftIndex === left.length || rightIndex === right.length) {
      return leftIndex === left.length && rightIndex === right.length;
    }
    const character = left.charCodeAt(leftIndex);
    if (character !== right.charCodeAt(rightIndex)) return false;
    lineStart = character === 10 || character === 13;
    leftIndex += 1;
    rightIndex += 1;
  }
}

export function isLuaIndentationEquivalent(code: string, generated: string): boolean {
  if (code === generated) return true;
  if (!matchesApartFromLinePrefixes(code, generated)) return false;
  const canonicalCode = canonicalIndentation(code);
  if (canonicalCode === null) return false;
  const canonicalGenerated = canonicalIndentation(generated);
  return canonicalGenerated !== null && canonicalCode === canonicalGenerated;
}
