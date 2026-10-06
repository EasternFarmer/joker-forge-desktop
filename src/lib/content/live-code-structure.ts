import { foldService } from "@codemirror/language";
import { StateEffect, StateField, Text, type Extension } from "@codemirror/state";
import { ViewPlugin, type EditorView, type ViewUpdate } from "@codemirror/view";
import { parse, type Options, type Token } from "luaparse";
import type { CodeSegment } from "./code-sections";
import { getCodeSegmentRanges } from "./live-code-navigation";

export type LiveCodeOutlineKind = "configuration" | "localization" | "callback" | "rule";

export interface LiveCodeOutlineEntry {
  id: string;
  label: string;
  kind: LiveCodeOutlineKind;
  from: number;
  to: number;
  depth: number;
  segmentId?: string;
}

export interface LiveCodeFoldRange {
  from: number;
  to: number;
}

export interface LiveCodeStructure {
  outline: LiveCodeOutlineEntry[];
  folds: LiveCodeFoldRange[];
  syntaxValid: boolean;
  pending?: boolean;
}

interface SyntaxNode {
  type: string;
  range?: [number, number];
  [key: string]: unknown;
}

const parserOptions: Partial<Options> = {
  luaVersion: "LuaJIT",
  extendedIdentifiers: true,
  encodingMode: "none",
  comments: false,
  ranges: true,
};

const isNode = (value: unknown): value is SyntaxNode =>
  typeof value === "object" && value !== null && "type" in value && typeof value.type === "string";

const childNodes = (value: unknown): SyntaxNode[] =>
  Array.isArray(value) ? value.filter(isNode) : isNode(value) ? [value] : [];

function expressionName(value: unknown): string | undefined {
  if (!isNode(value)) return undefined;
  if (value.type === "Identifier" && typeof value.name === "string") return value.name;
  if (value.type === "MemberExpression") {
    const base = expressionName(value.base);
    const name = expressionName(value.identifier);
    return base && name ? `${base}${value.indexer === ":" ? ":" : "."}${name}` : name;
  }
  if (value.type === "StringLiteral" && typeof value.raw === "string") {
    const raw = value.raw;
    if ((raw.startsWith("'") && raw.endsWith("'")) || (raw.startsWith('"') && raw.endsWith('"'))) {
      return raw.slice(1, -1);
    }
    return raw;
  }
  return undefined;
}

const sectionLabel = (name: string): { kind: LiveCodeOutlineKind; label: string } | undefined => {
  if (name === "config") return { kind: "configuration", label: "Configuration" };
  if (name === "loc_txt") return { kind: "localization", label: "Localization" };
  return undefined;
};

export function combineKnownLiveCodeOutline(
  structure: LiveCodeStructure,
  source: string,
  segments: CodeSegment[],
  labels?: ReadonlyMap<string, string>,
): LiveCodeOutlineEntry[] {
  const verified = !structure.pending && structure.syntaxValid;
  const outline = verified ? structure.outline.filter((entry) => entry.kind !== "rule").map((entry) => ({ ...entry })) : [];
  const ranges = getCodeSegmentRanges(source, segments);
  if (!verified) {
    for (const { segment, from, to } of ranges) {
      const section = sectionLabel(segment.id);
      if (section || segment.id === "loc_vars") outline.push({ id: `segment:${segment.id}:${from}`,
        label: labels?.get(segment.id) ?? section?.label ?? "loc_vars", kind: section?.kind ?? "callback",
        from, to, depth: 0, segmentId: segment.id });
    }
  }
  const ruleRanges = ranges.filter(({ segment }) => segment.id.startsWith("rule:"))
    .sort((a, b) => a.from - b.from || a.to - b.to);
  const ruleIds = [...new Set(ruleRanges.map(({ segment }) => segment.id))];
  for (const { segment, from, to } of ruleRanges) {
    if (outline.some((entry) => entry.segmentId === segment.id && entry.from === from && entry.to === to)) continue;
    outline.push({ id: `segment:${segment.id}:${from}`, label: labels?.get(segment.id) ?? `Rule ${ruleIds.indexOf(segment.id) + 1}`,
      kind: "rule", from, to, depth: 0, segmentId: segment.id });
  }
  outline.sort((a, b) => a.from - b.from || b.to - a.to || a.kind.localeCompare(b.kind));
  for (const entry of outline) {
    entry.depth = outline.filter((parent) => parent !== entry && parent.from <= entry.from && parent.to >= entry.to
      && (parent.from < entry.from || parent.to > entry.to)).length;
  }
  return outline;
}

export function getKnownLiveCodeOutline(
  source: string,
  segments: CodeSegment[],
  labels?: ReadonlyMap<string, string>,
): LiveCodeOutlineEntry[] {
  return combineKnownLiveCodeOutline({ outline: [], folds: [], syntaxValid: false, pending: true }, source, segments, labels);
}

export function combineKnownLiveCodeStructure(
  structure: LiveCodeStructure,
  source: string,
  segments: CodeSegment[],
  labels?: ReadonlyMap<string, string>,
): LiveCodeStructure {
  const outline = combineKnownLiveCodeOutline(structure, source, segments, labels);
  const folds = !structure.pending && structure.syntaxValid ? [...structure.folds] : [];
  if (!structure.pending && structure.syntaxValid) {
    const doc = Text.of(source.split("\n"));
    for (const entry of outline) {
      if (entry.kind !== "rule") continue;
      const fold = getRuleFoldRange(doc, entry.from, entry.to);
      if (fold) folds.push(fold);
    }
  }
  const uniqueFolds = [...new Map(folds.map((range) => [`${range.from}:${range.to}`, range])).values()]
    .sort((a, b) => a.from - b.from || b.to - a.to);
  return { ...structure, outline, folds: uniqueFolds };
}

export function getLiveCodeStructure(
  source: string,
  segments: CodeSegment[] = [],
  labels?: ReadonlyMap<string, string>,
): LiveCodeStructure {
  const outline: LiveCodeOutlineEntry[] = [];
  const folds: LiveCodeFoldRange[] = [];
  let syntaxValid = false;
  const doc = Text.of(source.split("\n"));

  const addOutline = (node: SyntaxNode, label: string, kind: LiveCodeOutlineKind): void => {
    if (!node.range || node.range[1] <= node.range[0]) return;
    outline.push({ id: `${kind}:${node.range[0]}:${label}`, label, kind,
      from: node.range[0], to: node.range[1], depth: 0 });
  };

  try {
    const tree = parse(source, parserOptions) as unknown as SyntaxNode;
    const lexer = parse(source, { ...parserOptions, wait: true });
    const tokens: Token[] = [];
    for (let token = lexer.lex(); token.type !== 1; token = lexer.lex()) tokens.push(token);
    const tokensByEnd = new Map(tokens.map((token) => [token.range[1], token]));
    const ownedFunctions = new Set<SyntaxNode>();

    const tokenAfter = (position: number, end: number, value: string): Token | undefined => {
      let low = 0;
      let high = tokens.length;
      while (low < high) {
        const middle = (low + high) >>> 1;
        if (tokens[middle].range[0] < position) low = middle + 1;
        else high = middle;
      }
      for (let index = low; index < tokens.length && tokens[index].range[1] <= end; index += 1) {
        if (tokens[index].value === value) return tokens[index];
      }
      return undefined;
    };

    const addFold = (node: SyntaxNode): void => {
      if (!node.range) return;
      const [start, end] = node.range;
      let open: Token | undefined;
      let close: Token | undefined;
      const firstBody = childNodes(node.body)[0];
      const headerEnd = firstBody?.range?.[0] ?? end;

      if (node.type === "TableConstructorExpression") {
        open = tokenAfter(start, end, "{");
        const last = tokensByEnd.get(end);
        close = last?.value === "}" ? last : undefined;
      } else if (node.type === "FunctionDeclaration") {
        const opening = tokenAfter(start, headerEnd, "(");
        open = opening && tokenAfter(opening.range[1], headerEnd, ")");
        const last = tokensByEnd.get(end);
        close = last?.value === "end" ? last : undefined;
      } else if (node.type === "IfStatement") {
        const firstClause = childNodes(node.clauses)[0];
        const condition = isNode(firstClause?.condition) ? firstClause.condition : undefined;
        open = tokenAfter(condition?.range?.[1] ?? start, end, "then");
        const last = tokensByEnd.get(end);
        close = last?.value === "end" ? last : undefined;
      } else if (node.type === "RepeatStatement") {
        open = tokenAfter(start, end, "repeat");
        const body = childNodes(node.body);
        const condition = isNode(node.condition) ? node.condition : undefined;
        close = tokenAfter(body[body.length - 1]?.range?.[1] ?? start, condition?.range?.[0] ?? end, "until");
      } else if (["WhileStatement", "ForNumericStatement", "ForGenericStatement", "DoStatement"].includes(node.type)) {
        const headerNodes = node.type === "WhileStatement" ? childNodes(node.condition)
          : node.type === "ForNumericStatement" ? [...childNodes(node.end), ...childNodes(node.step)]
            : node.type === "ForGenericStatement" ? childNodes(node.iterators) : [];
        const conditionEnd = headerNodes[headerNodes.length - 1]?.range?.[1] ?? start;
        open = tokenAfter(conditionEnd, headerEnd, "do");
        const last = tokensByEnd.get(end);
        close = last?.value === "end" ? last : undefined;
      }
      if (!open || !close) return;
      const from = doc.lineAt(open.range[1]).to;
      const to = close.range[0];
      if (to > from && doc.lineAt(to).number > doc.lineAt(from).number) folds.push({ from, to });
    };

    const visit = (node: SyntaxNode): void => {
      if (node.type === "TableKeyString" || node.type === "TableKey") {
        const name = expressionName(node.key);
        const section = name && sectionLabel(name);
        if (section) addOutline(node, section.label, section.kind);
        const value = isNode(node.value) ? node.value : undefined;
        if (name && value?.type === "FunctionDeclaration") {
          addOutline(node, name, "callback");
          ownedFunctions.add(value);
        }
      } else if (node.type === "LocalStatement" || node.type === "AssignmentStatement") {
        const variables = childNodes(node.variables);
        childNodes(node.init).forEach((value, index) => {
          const name = expressionName(variables[index]);
          if (name && value.type === "FunctionDeclaration" && value.range) {
            addOutline({ ...value, range: [node.range?.[0] ?? value.range[0], value.range[1]] }, name, "callback");
            ownedFunctions.add(value);
          }
        });
      } else if (node.type === "FunctionDeclaration" && !ownedFunctions.has(node)) {
        const name = expressionName(node.identifier);
        if (name) addOutline(node, name, "callback");
      }
      addFold(node);
      for (const value of Object.values(node)) for (const child of childNodes(value)) visit(child);
    };
    visit(tree);
    syntaxValid = true;
  } catch {
    outline.length = 0;
    folds.length = 0;
  }

  return combineKnownLiveCodeStructure({ outline, folds, syntaxValid }, source, segments, labels);
}

export function getLiveCodeFoldRange(
  structure: LiveCodeStructure,
  lineStart: number,
  lineEnd: number,
): LiveCodeFoldRange | null {
  return structure.folds.find(({ from }) => from >= lineStart && from <= lineEnd) ?? null;
}

export const setLiveCodeStructureEffect = StateEffect.define<{ doc: Text; structure: LiveCodeStructure }>();

export const liveCodeStructureField = StateField.define<LiveCodeStructure>({
  create: (state) => getLiveCodeStructure(state.doc.toString()),
  update(structure, transaction) {
    let next = transaction.docChanged ? { outline: [], folds: [], syntaxValid: false, pending: true } : structure;
    for (const effect of transaction.effects) {
      if (effect.is(setLiveCodeStructureEffect) && effect.value.doc === transaction.newDoc) {
        next = { ...effect.value.structure, pending: false };
      }
    }
    return next;
  },
});

const liveCodeAnalysisPlugin = ViewPlugin.fromClass(class {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private destroyed = false;

  constructor(private view: EditorView) {}

  update(update: ViewUpdate) {
    if (!update.docChanged || this.destroyed) return;
    if (this.timer !== undefined) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.timer = undefined;
      if (this.destroyed) return;
      const doc = this.view.state.doc;
      const structure = getLiveCodeStructure(doc.toString());
      if (!this.destroyed && this.view.state.doc === doc) {
        this.view.dispatch({ effects: setLiveCodeStructureEffect.of({ doc, structure }) });
      }
    }, 180);
  }

  destroy() {
    this.destroyed = true;
    if (this.timer !== undefined) clearTimeout(this.timer);
  }
});

function getRuleFoldRange(doc: Text, from: number, to: number): LiveCodeFoldRange | null {
  const firstLine = doc.lineAt(from);
  let lastPosition = to - 1;
  while (lastPosition > from && /\s/.test(doc.sliceString(lastPosition, lastPosition + 1))) lastPosition -= 1;
  const lastLine = doc.lineAt(lastPosition);
  return lastLine.number > firstLine.number + 1 ? { from: firstLine.to, to: lastLine.from } : null;
}

const emptySegments: CodeSegment[] = [];

export function liveCodeFolding(getSegments: () => CodeSegment[] = () => emptySegments): Extension {
  let cachedDoc: Text | undefined;
  let cachedSegments: CodeSegment[] | undefined;
  let ruleFolds: LiveCodeFoldRange[] = [];
  return [liveCodeStructureField, liveCodeAnalysisPlugin, foldService.of((state, lineStart, lineEnd) => {
    const structure = state.field(liveCodeStructureField);
    const parsedFold = getLiveCodeFoldRange(structure, lineStart, lineEnd);
    if (!structure.syntaxValid) return parsedFold;
    const currentSegments = getSegments();
    if (cachedDoc !== state.doc || cachedSegments !== currentSegments) {
      cachedDoc = state.doc;
      cachedSegments = currentSegments;
      ruleFolds = getCodeSegmentRanges(state.doc.toString(), currentSegments).flatMap(({ segment, from, to }) => {
        const fold = segment.id.startsWith("rule:") ? getRuleFoldRange(state.doc, from, to) : null;
        return fold ? [fold] : [];
      });
    }
    const ruleFold = ruleFolds.filter(({ from }) => from >= lineStart && from <= lineEnd)
      .sort((a, b) => b.to - a.to)[0];
    return ruleFold && (!parsedFold || ruleFold.to > parsedFold.to) ? ruleFold : parsedFold;
  })];
}
