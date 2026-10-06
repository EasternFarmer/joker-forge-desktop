import { countColumn, type EditorState, type Extension } from "@codemirror/state";
import { getIndentUnit } from "@codemirror/language";
import { highlightSelectionMatches } from "@codemirror/search";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

export interface IndentationGuideRange {
  from: number;
  to: number;
  columns: number;
  indentUnit: number;
}

interface VisibleRange { from: number; to: number }

/** Leading whitespace only; wrapped code continuations never become indentation. */
export function getVisibleIndentationGuides(
  state: EditorState, visibleRanges: readonly VisibleRange[],
): IndentationGuideRange[] {
  const guides: IndentationGuideRange[] = [];
  const visitedLines = new Set<number>();
  const indentUnit = getIndentUnit(state);
  for (const visible of visibleRanges) {
    let line = state.doc.lineAt(visible.from);
    while (line.from < visible.to) {
      if (!visitedLines.has(line.from)) {
        visitedLines.add(line.from);
        const whitespace = /^[\t ]+/.exec(line.text)?.[0];
        if (whitespace) {
          const to = line.from + whitespace.length;
          const columns = countColumn(whitespace, state.tabSize);
          // If the viewport starts on a wrapped continuation, the original
          // indentation is above it and needs no decoration in this viewport.
          if (to > visible.from && columns >= indentUnit) guides.push({
            from: line.from, to, columns, indentUnit,
          });
        }
      }
      if (line.to >= visible.to || line.number === state.doc.lines) break;
      line = state.doc.line(line.number + 1);
    }
  }
  return guides;
}

type IndentationGuideUpdate = Pick<ViewUpdate,
  "docChanged" | "viewportChanged" | "startState" | "state" | "changes">;

/** Ordinary typing inside code can map existing guides without rescanning lines. */
export function shouldRebuildIndentationGuides(update: IndentationGuideUpdate): boolean {
  if (update.viewportChanged || update.startState.tabSize !== update.state.tabSize
    || getIndentUnit(update.startState) !== getIndentUnit(update.state)) return true;
  if (!update.docChanged) return false;
  let rebuild = false;
  update.changes.iterChanges((from, to, _fromNew, _toNew, inserted) => {
    if (rebuild) return;
    const line = update.startState.doc.lineAt(from);
    const indentationEnd = line.from + (/^[\t ]*/.exec(line.text)?.[0].length ?? 0);
    if (from <= indentationEnd || inserted.lines > 1
      || update.startState.doc.lineAt(to).number !== line.number) rebuild = true;
  });
  return rebuild;
}

const makeGuideDecorations = (view: EditorView): DecorationSet => Decoration.set(
  getVisibleIndentationGuides(view.state, view.visibleRanges).map((guide) => Decoration.mark({
    class: "cm-jf-indentation-guide",
    attributes: { style: `--jf-indent-step: ${guide.indentUnit}ch` },
  }).range(guide.from, guide.to)), true,
);

export const liveCodeIndentationGuides = ViewPlugin.fromClass(class {
  decorations: DecorationSet;

  constructor(view: EditorView) {
    this.decorations = makeGuideDecorations(view);
  }

  update(update: ViewUpdate) {
    if (shouldRebuildIndentationGuides(update)) this.decorations = makeGuideDecorations(update.view);
    else if (update.docChanged) this.decorations = this.decorations.map(update.changes);
  }
}, { decorations: (plugin) => plugin.decorations });

// App variables are complete hex/oklch colors, rather than HSL channels.
// Use them directly so the editor stays legible in both application themes.
export const liveCodeEditorAppearanceTheme = EditorView.theme({
  "&": { "--jf-code-font": '"Cascadia Code", "Cascadia Mono", Consolas, "Liberation Mono", monospace' },
  ".cm-content": {
    fontFamily: "var(--jf-code-font)", fontVariantLigatures: "none", caretColor: "var(--foreground)",
  },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--foreground)" },
  ".cm-jf-indentation-guide": {
    backgroundImage: "repeating-linear-gradient(to right, color-mix(in srgb, var(--muted-foreground) 18%, transparent) 0, color-mix(in srgb, var(--muted-foreground) 18%, transparent) 1px, transparent 1px, transparent var(--jf-indent-step))",
    backgroundPosition: "left center", backgroundRepeat: "repeat-x",
  },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--foreground) 3%, transparent)" },
  ".cm-gutters": { color: "color-mix(in srgb, var(--muted-foreground) 65%, transparent)", borderRight: "1px solid var(--border)" },
  ".cm-activeLineGutter": { color: "var(--foreground)", backgroundColor: "color-mix(in srgb, var(--foreground) 3%, transparent)" },
  ".cm-selectionLayer .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 30%, transparent) !important" },
  "&.cm-focused .cm-selectionLayer .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 40%, transparent) !important" },
  ".cm-content ::selection": { backgroundColor: "color-mix(in srgb, var(--primary) 40%, transparent)" },
  ".cm-selectionMatch": {
    backgroundColor: "color-mix(in srgb, var(--primary) 12%, transparent)",
    boxShadow: "inset 0 0 0 1px color-mix(in srgb, var(--primary) 25%, transparent)",
  },
  ".cm-selectionMatch-main": {
    backgroundColor: "transparent", boxShadow: "inset 0 0 0 1px color-mix(in srgb, var(--primary) 35%, transparent)",
  },
  ".cm-tooltip-autocomplete": {
    color: "var(--popover-foreground)", backgroundColor: "var(--popover)", border: "1px solid var(--border)",
    borderRadius: "5px", boxShadow: "0 6px 18px rgba(0, 0, 0, 0.25)",
  },
  ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--jf-code-font)" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
    color: "var(--foreground)", backgroundColor: "color-mix(in srgb, var(--primary) 20%, var(--popover))",
  },
  ".cm-completionInfo": { backgroundColor: "var(--popover)", color: "var(--popover-foreground)", borderColor: "var(--border)" },
});

/** Both guides and selected-word highlights stay bounded to the visible editor. */
export function liveCodeEditorAppearance(): Extension {
  return [liveCodeIndentationGuides, liveCodeEditorAppearanceTheme, highlightSelectionMatches({
    highlightWordAroundCursor: true, minSelectionLength: 2, wholeWords: true, maxMatches: 100,
  })];
}
