/** Exact links between generated Lua literals and visual fields. */
export interface FieldBinding {
  sourcePath: (string | number)[];
  valueType: "string" | "number" | "boolean";
  originalValue: string | number | boolean;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
}

export interface BoundFieldRange extends FieldBinding {
  from: number;
  to: number;
  sourceIds?: Record<number, string>;
}

export interface CodeEdit {
  from: number;
  to: number;
  insert: string;
}

type Scalar = string | number | boolean;
type LuaToken = { from: number; to: number; key: string };

const pathKey = (path: FieldBinding["sourcePath"]) => JSON.stringify(path);

/** Parse data, never execute the user's Lua. Incomplete edits stay in the editor. */
export function parseLuaScalar(source: string): Scalar | undefined {
  const text = source.trim();
  if (text === "true") return true;
  if (text === "false") return false;
  if (/^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(text)) {
    const value = Number(text);
    return Number.isFinite(value) ? value : undefined;
  }
  if (/^[+-]?0[xX][\da-fA-F]+$/.test(text)) {
    const sign = text.startsWith("-") ? -1 : 1;
    const value = sign * Number(text.replace(/^[+-]/, ""));
    return Number.isFinite(value) ? value : undefined;
  }
  const longString = text.match(/^\[(=*)\[([\s\S]*)\]\1\]$/);
  if (longString) {
    const closing = `]${longString[1]}]`;
    if (longString[2].includes(closing)) return undefined;
    return longString[2].replace(/^(?:\r\n|\n|\r)/, "");
  }
  const quote = text[0];
  if (text.length < 2 || (quote !== '"' && quote !== "'") || text[text.length - 1] !== quote) {
    return undefined;
  }
  let result = "";
  for (let i = 1; i < text.length - 1; i += 1) {
    const char = text[i];
    if (char === quote || char === "\n" || char === "\r") return undefined;
    if (char !== "\\") {
      result += char;
      continue;
    }
    i += 1;
    if (i >= text.length - 1) return undefined;
    const escaped = text[i];
    const escapes: Record<string, string> = {
      a: "\x07", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t", v: "\x0b",
      "\\": "\\", '"': '"', "'": "'",
    };
    if (Object.prototype.hasOwnProperty.call(escapes, escaped)) {
      result += escapes[escaped];
    } else if (escaped === "\n" || escaped === "\r") {
      if (escaped === "\r" && text[i + 1] === "\n") i += 1;
      result += "\n";
    } else if (escaped === "z") {
      while (/\s/.test(text[i + 1] ?? "") && i < text.length - 2) i += 1;
    } else if (escaped === "x") {
      const hex = text.slice(i + 1, i + 3);
      if (!/^[\da-fA-F]{2}$/.test(hex)) return undefined;
      result += String.fromCharCode(parseInt(hex, 16));
      i += 2;
    } else if (/\d/.test(escaped)) {
      const digits = text.slice(i).match(/^\d{1,3}/)?.[0];
      if (!digits || Number(digits) > 255) return undefined;
      result += String.fromCharCode(Number(digits));
      i += digits.length - 1;
    } else {
      return undefined;
    }
  }
  return result;
}

const lineStarts = (code: string): number[] => {
  const starts = [0];
  for (let i = 0; i < code.length; i += 1) {
    if (code[i] === "\n") starts.push(i + 1);
  }
  return starts;
};

export function toRanges(code: string, bindings: FieldBinding[]): BoundFieldRange[] {
  const starts = lineStarts(code);
  const position = (line: number, column: number) => {
    const start = starts[line - 1];
    if (start === undefined || column < 1) return -1;
    // Preview metadata and CodeMirror both use UTF-16 columns.
    const index = start + column - 1;
    const end = starts[line] === undefined ? code.length : starts[line] - 1;
    return index <= end ? index : -1;
  };
  return bindings.flatMap((binding) => {
    const from = position(binding.startLine, binding.startColumn);
    const to = position(binding.endLine, binding.endColumn);
    if (from < 0 || to <= from || to > code.length) return [];
    const value = parseLuaScalar(code.slice(from, to));
    return typeof value === binding.valueType && value === binding.originalValue
      ? [{ ...binding, from, to }]
      : [];
  });
}

/** A small lexer lets saved custom code retain links through whitespace/comments. */
function tokenize(code: string): LuaToken[] {
  const tokens: LuaToken[] = [];
  let index = 0;
  while (index < code.length) {
    if (/\s/.test(code[index])) { index += 1; continue; }
    const from = index;
    const tail = code.slice(index);
    const comment = tail.startsWith("--");
    const long = tail.slice(comment ? 2 : 0).match(/^\[(=*)\[/);
    if (long) {
      const openingLength = long[0].length + (comment ? 2 : 0);
      const closing = `]${long[1]}]`;
      const end = code.indexOf(closing, index + openingLength);
      index = end < 0 ? code.length : end + closing.length;
    } else if (comment) {
      const end = code.indexOf("\n", index);
      index = end < 0 ? code.length : end;
    } else if (code[index] === '"' || code[index] === "'") {
      const quote = code[index++];
      while (index < code.length) {
        const char = code[index++];
        if (char === "\\") index += 1;
        else if (char === quote) break;
      }
    } else {
      const word = tail.match(/^(?:0[xX][\da-fA-F]+|(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?|[A-Za-z_][\w]*|\.\.\.|\.\.|==|~=|<=|>=)/)?.[0];
      index += word?.length ?? 1;
    }
    const token = code.slice(from, index);
    const value = comment ? undefined : parseLuaScalar(token);
    tokens.push({ from, to: index, key: value === undefined ? token : `literal:${typeof value}` });
  }
  return tokens;
}

/** Myers alignment: unchanged syntax anchors changed literals to their own fields. */
function tokenMatches(before: LuaToken[], after: LuaToken[]): Map<number, number> {
  let frontier = new Map<number, number>([[1, 0]]);
  const trace: Map<number, number>[] = [];
  let operations = 0;
  for (let distance = 0; distance <= before.length + after.length; distance += 1) {
    trace.push(frontier);
    const next = new Map<number, number>();
    for (let diagonal = -distance; diagonal <= distance; diagonal += 2) {
      const left = frontier.get(diagonal - 1) ?? -1;
      const right = frontier.get(diagonal + 1) ?? -1;
      let x = diagonal === -distance || (diagonal !== distance && left < right) ? right : left + 1;
      let y = x - diagonal;
      while (x < before.length && y < after.length && before[x]?.key === after[y]?.key) {
        x += 1;
        y += 1;
      }
      next.set(diagonal, x);
      operations += 1;
      if (operations > 250_000) return new Map();
      if (x >= before.length && y >= after.length) {
        const matches = new Map<number, number>();
        for (let step = distance; step >= 0; step -= 1) {
          const prior = trace[step];
          const k = x - y;
          const priorLeft = prior.get(k - 1) ?? -1;
          const priorRight = prior.get(k + 1) ?? -1;
          const priorK = k === -step || (k !== step && priorLeft < priorRight) ? k + 1 : k - 1;
          const priorX = Math.max(0, prior.get(priorK) ?? 0);
          const priorY = priorX - priorK;
          while (x > priorX && y > priorY) {
            matches.set(--x, --y);
          }
          x = priorX;
          y = priorY;
        }
        return matches;
      }
    }
    frontier = next;
  }
  return new Map();
}

function remapRanges(before: string, after: string, ranges: BoundFieldRange[]): BoundFieldRange[] {
  if (before === after) return ranges;
  const oldTokens = tokenize(before);
  const newTokens = tokenize(after);
  const matches = tokenMatches(oldTokens, newTokens);
  return ranges.flatMap((range) => {
    const first = oldTokens.findIndex((token) => token.from === range.from);
    const last = oldTokens.findIndex((token) => token.to === range.to);
    if (first < 0 || last < first) return [];
    const mappedFirst = matches.get(first);
    const mappedLast = matches.get(last);
    if (mappedFirst === undefined || mappedLast === undefined) return [];
    if (mappedLast - mappedFirst !== last - first) return [];
    // Require surrounding syntax to agree, not just another equal literal.
    for (const neighbor of [first - 1, last + 1]) {
      if (neighbor < 0 || neighbor >= oldTokens.length) continue;
      const expected = neighbor < first ? mappedFirst - 1 : mappedLast + 1;
      if (matches.get(neighbor) !== expected) return [];
    }
    return [{ ...range, from: newTokens[mappedFirst].from, to: newTokens[mappedLast].to }];
  });
}

/** Track actual editor transactions, including a temporarily incomplete literal. */
export function updateBoundRanges(
  oldCode: string,
  newCode: string,
  ranges: BoundFieldRange[],
  changes?: CodeEdit[],
): BoundFieldRange[] {
  if (!changes) return remapRanges(oldCode, newCode, ranges);
  const edits = [...changes].sort((a, b) => a.from - b.from);
  let newTokens: LuaToken[] | undefined;
  const position = (pos: number, association: -1 | 1) => {
    let shift = 0;
    for (const edit of edits) {
      if (edit.from > pos || (edit.from === pos && association < 0)) break;
      if (edit.to < pos || (edit.to === pos && (edit.from < pos || association > 0))) {
        shift += edit.insert.length - (edit.to - edit.from);
      } else {
        return edit.from + shift + (association > 0 ? edit.insert.length : 0);
      }
    }
    return pos + shift;
  };
  return ranges.flatMap((range) => {
    // An edit crossing a literal's boundary changes Lua structure; detach it.
    if (edits.some((edit) => edit.from < range.to && edit.to > range.from &&
      (edit.from < range.from || edit.to > range.to))) return [];
    const incomplete = parseLuaScalar(oldCode.slice(range.from, range.to)) === undefined;
    const numericBoundary = (at: number) => range.valueType === "number" && edits.some((edit) =>
      edit.from === at && edit.to === at && /^[\d.eE+\-]+$/.test(edit.insert));
    const replacesStart = edits.some((edit) => edit.from <= range.from && edit.to > range.from);
    const replacesEnd = edits.some((edit) => edit.from < range.to && edit.to >= range.to);
    let from = position(range.from, replacesStart || incomplete || numericBoundary(range.from) ? -1 : 1);
    let to = position(range.to, replacesEnd || incomplete || numericBoundary(range.to) ? 1 : -1);
    // CodeMirror can wrap a selected literal in quotes, then replace its interior.
    // Reattach the quotes when that interior becomes a complete string literal.
    if (range.valueType === "string" && parseLuaScalar(newCode.slice(from, to)) === undefined) {
      newTokens ??= tokenize(newCode);
      const enclosing = newTokens.find((token) => token.from <= from && token.to >= to &&
        typeof parseLuaScalar(newCode.slice(token.from, token.to)) === "string");
      if (enclosing) { from = enclosing.from; to = enclosing.to; }
    }
    return from >= 0 && to >= from && to <= newCode.length ? [{ ...range, from, to }] : [];
  });
}

export function readBoundFieldEdits(code: string, ranges: BoundFieldRange[]) {
  const fields = new Map<string, { sourcePath: FieldBinding["sourcePath"]; values: Set<Scalar>; changed: Set<Scalar> }>();
  for (const range of ranges) {
    const value = parseLuaScalar(code.slice(range.from, range.to));
    if (typeof value !== range.valueType) continue;
    const key = pathKey(range.sourcePath);
    const field = fields.get(key) ?? { sourcePath: range.sourcePath, values: new Set<Scalar>(), changed: new Set<Scalar>() };
    field.values.add(value as Scalar);
    if (value !== range.originalValue) field.changed.add(value as Scalar);
    fields.set(key, field);
  }
  return [...fields.values()].flatMap((field) => {
    const values = field.changed.size > 0 ? field.changed : field.values;
    return values.size === 1 ? [{ sourcePath: field.sourcePath, value: [...values][0] }] : [];
  });
}

/** GUI updates replace their linked literals while keeping custom Lua intact. */
export function rebaseLinkedCode(
  currentCode: string,
  _oldGenerated: string,
  newGenerated: string,
  oldRanges: BoundFieldRange[],
  newBindings: FieldBinding[],
): { code: string; ranges: BoundFieldRange[] } {
  const newRanges = toRanges(newGenerated, newBindings);
  const newByPath = new Map(newRanges.map((range) => [pathKey(range.sourcePath), range]));
  const replacements: CodeEdit[] = [];
  for (const range of oldRanges) {
    const next = newByPath.get(pathKey(range.sourcePath));
    if (!next || next.originalValue === range.originalValue) continue;
    const currentValue = parseLuaScalar(currentCode.slice(range.from, range.to));
    if (typeof currentValue !== range.valueType) continue;
    const insert = newGenerated.slice(next.from, next.to);
    if (currentCode.slice(range.from, range.to) !== insert) {
      replacements.push({ from: range.from, to: range.to, insert });
    }
  }
  let code = currentCode;
  for (const replacement of replacements.sort((a, b) => b.from - a.from)) {
    code = code.slice(0, replacement.from) + replacement.insert + code.slice(replacement.to);
  }
  const tracked = updateBoundRanges(currentCode, code, oldRanges, replacements);
  const ranges: BoundFieldRange[] = [];
  const trackedPaths = new Set<string>();
  // Tracked identities take precedence over matching syntax after a rule reorder.
  for (const range of tracked) {
    const next = newByPath.get(pathKey(range.sourcePath));
    if (next) {
      const currentValue = parseLuaScalar(code.slice(range.from, range.to));
      ranges.push({ ...range, ...next, originalValue: currentValue === undefined ? range.originalValue : next.originalValue,
        from: range.from, to: range.to });
      trackedPaths.add(pathKey(range.sourcePath));
    }
  }
  ranges.push(...remapRanges(newGenerated, code, newRanges).filter((range) => !trackedPaths.has(pathKey(range.sourcePath))));
  return { code, ranges };
}
