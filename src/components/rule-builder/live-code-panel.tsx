import React, { useRef, useEffect, useState, useCallback, useMemo } from "react";
import {
  BracketsCurly,
  WarningCircle,
  ArrowCounterClockwise,
  ArrowsClockwise,
  ArrowsOutSimple,
  ArrowsInSimple,
} from "@phosphor-icons/react";
import { ListTree, Search, AlignLeft, Info, ChevronRight, ChevronDown, ChevronsUp, ChevronsDown,
  Braces, Languages, CodeXml, Layers, FoldVertical, UnfoldVertical } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  EditorState,
  Compartment,
  StateField,
  StateEffect,
  Transaction,
  Prec,
  type Range,
} from "@codemirror/state";
import {
  EditorView,
  Decoration,
  type DecorationSet,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  hoverTooltip,
  closeHoverTooltips,
  drawSelection,
  dropCursor,
  rectangularSelection,
  crosshairCursor,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, insertNewlineAndIndent, isolateHistory } from "@codemirror/commands";
import { StreamLanguage, bracketMatching, indentUnit, foldGutter, foldKeymap, foldAll, unfoldAll, foldedRanges, foldEffect, unfoldEffect } from "@codemirror/language";
import { lua } from "@codemirror/legacy-modes/mode/lua";
import { oneDark } from "@codemirror/theme-one-dark";
import { searchKeymap, closeSearchPanel } from "@codemirror/search";
import { linter } from "@codemirror/lint";
import {
  autocompletion,
  acceptCompletion,
  completionStatus,
  completionKeymap,
  closeBrackets,
  closeBracketsKeymap,
} from "@codemirror/autocomplete";
import { luaSmodsCompletions } from "@/lib/content/lua-completions";
import type { CodeSegment } from "@/lib/content/code-sections";
import { updateBoundRanges, type CodeEdit } from "@/lib/content/live-code-sync";
import { getLuaDiagnostics } from "@/lib/content/lua-diagnostics";
import type { LiveCodeFieldLink } from "@/lib/content/live-code-fields";
import { findCodeSegmentAt, getCodeSegmentRanges, mapCodeSegmentsThroughEdits } from "@/lib/content/live-code-navigation";
import type { LiveCodeExplanation } from "@/lib/content/live-code-explanations";
import { liveCodeStructureField, setLiveCodeStructureEffect, combineKnownLiveCodeStructure, liveCodeFolding, type LiveCodeStructure, type LiveCodeOutlineEntry } from "@/lib/content/live-code-structure";
import { formatLuaCode } from "@/lib/content/live-code-format";
import { formatIndentEffect, formatPresentationHistory, getFormatPresentationChange } from "@/lib/content/live-code-format-history";
import { liveCodeEditorKeymap } from "@/lib/content/live-code-editor-commands";
import { liveCodeEditorAppearance } from "@/lib/content/live-code-editor-appearance";
import { getLiveCodeCursorStatus, getLiveCodeGoToPosition, type LiveCodeCursorStatus } from "@/lib/content/live-code-editor-state";
import { liveCodeSearch, openLiveCodeSearch } from "@/lib/content/live-code-search";
import { buildLiveCodeOutlineTree, filterLiveCodeOutlineTree, flattenLiveCodeOutlineTree, getActiveLiveCodeOutlineEntry } from "@/lib/content/live-code-outline";

export interface LiveCodeChangeOptions {
  formatting?: boolean;
  indentUnit?: string;
}

interface LiveCodePanelProps {
  title: string;
  code: string;
  codeResetRevision?: number;
  isLoading: boolean;
  statusMessage?: string;
  isError?: boolean;
  errorDetails?: string;
  widthPercent: number;
  isBlockPreview: boolean;
  onBackToItem: () => void;
  onStartResize: (e: React.MouseEvent) => void;
  onCodeChange?: (code: string, changes?: CodeEdit[], options?: LiveCodeChangeOptions) => void;
  onResetCustomCode?: () => void;
  hasCustomCode?: boolean;
  segments?: CodeSegment[];
  fieldLinks?: LiveCodeFieldLink[];
  onNavigateToField?: (link: LiveCodeFieldLink, focus?: boolean) => void;
  onNavigateToSegment?: (segmentId: string, revealBuilder?: boolean) => void;
  revealSegmentId?: string;
  revealSelection?: object | null;
  selectedSegmentId?: string;
  hoveredSegmentId?: string;
  explanations?: Record<string, LiveCodeExplanation>;
}

// Theme that inherits the panel background (transparent)
const editorTheme = EditorView.theme({
  ".cm-jf-linked-field": {
    textDecorationLine: "underline",
    textDecorationStyle: "dotted",
    textDecorationColor: "color-mix(in srgb, var(--muted-foreground) 50%, transparent)",
    textUnderlineOffset: "3px",
    cursor: "pointer",
  },
  ".jf-field-tooltip": {
    backgroundColor: "var(--card)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
    borderRadius: "8px",
    padding: "10px 12px",
    maxWidth: "360px",
    maxHeight: "280px",
    overflowY: "auto",
    fontFamily: "ui-sans-serif, system-ui, sans-serif",
    fontSize: "12px",
    lineHeight: "1.5",
    boxShadow: "0 4px 16px rgba(0,0,0,0.3)",
  },
  ".jf-field-tooltip button": {
    display: "block",
    marginTop: "8px",
    padding: "3px 8px",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    cursor: "pointer",
    color: "var(--primary)",
  },
  ".jf-code-explanation": {
    marginTop: "6px",
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
  },
  ".cm-matchingBracket": {
    backgroundColor: "color-mix(in srgb, var(--primary) 22%, transparent)",
    outline: "1px solid var(--primary)",
  },
  ".cm-nonmatchingBracket": {
    color: "var(--destructive)",
    textDecoration: "underline wavy",
  },
  ".cm-foldGutter": { minWidth: "16px", cursor: "pointer" },
  ".cm-foldPlaceholder": {
    backgroundColor: "var(--muted)",
    color: "var(--muted-foreground)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    padding: "0 5px",
    cursor: "pointer",
  },
  ".cm-panels": {
    backgroundColor: "var(--card)",
    color: "var(--foreground)",
  },
  ".cm-panel.cm-search": {
    padding: "8px 24px 8px 8px",
    fontFamily: "ui-sans-serif, system-ui, sans-serif",
    fontSize: "12px",
  },
  ".cm-search .cm-textfield": {
    backgroundColor: "var(--background)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    maxWidth: "45%",
  },
  ".cm-search .cm-button": {
    background: "var(--muted)",
    color: "var(--foreground)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
  },
  "&": {
    height: "100%",
    backgroundColor: "transparent !important",
  },
  "&.cm-editor": {
    backgroundColor: "transparent !important",
    userSelect: "text",
    WebkitUserSelect: "text",
  },
  ".cm-content": {
    fontFamily:
      "'Cascadia Code', Consolas, 'SF Mono', Menlo, monospace",
    lineHeight: "1.55",
    padding: "8px 0",
    userSelect: "text",
    WebkitUserSelect: "text",
    cursor: "text",
  },
  ".cm-content[contenteditable='false']": {
    cursor: "text",
  },
  ".cm-line": {
    userSelect: "text",
    WebkitUserSelect: "text",
    cursor: "text",
  },
  ".cm-gutters": {
    backgroundColor: "var(--card) !important",
    borderRight: "1px solid var(--border)",
    color: "var(--muted-foreground)",
    minWidth: "3.4rem",
  },
  ".cm-scroller": {
    overflow: "auto",
    fontFamily: "'Cascadia Code', Consolas, 'SF Mono', Menlo, monospace",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul": {
    fontFamily:
      "'Cascadia Code', Consolas, 'SF Mono', Menlo, monospace",
  },
  ".cm-tooltip.cm-tooltip-autocomplete > ul > li": {
    padding: "3px 8px",
  },
  ".cm-completionDetail": {
    color: "var(--muted-foreground)",
    fontStyle: "normal",
    marginLeft: "8px",
  },
  ".cm-completionMatchedText": {
    color: "var(--primary)",
    textDecoration: "none",
    fontWeight: "600",
  },
  ".cm-line.jf-segment-hover-line": {
    backgroundColor: "rgba(34, 197, 94, 0.08) !important",
    boxShadow: "inset 2px 0 0 rgba(34, 197, 94, 0.38)",
  },
  ".cm-line.jf-segment-selected-line": {
    backgroundColor: "rgba(34, 197, 94, 0.12) !important",
    boxShadow: "inset 2px 0 0 rgba(34, 197, 94, 0.62)",
  },
  ".cm-jf-segment-hover-range": {
    backgroundColor: "rgba(34, 197, 94, 0.12)",
    borderRadius: "2px",
  },
  ".cm-jf-segment-selected-range": {
    backgroundColor: "rgba(34, 197, 94, 0.2)",
    boxShadow: "inset 0 0 0 1px rgba(34, 197, 94, 0.45)",
    borderRadius: "2px",
  },
});

const readOnlyCompartment = new Compartment();
const fontSizeCompartment = new Compartment();
const tooltipCompartment = new Compartment();
const wrappingCompartment = new Compartment();
const tooltipsOffTheme = EditorView.theme({
  ".cm-tooltip-lint, .cm-completionInfo": { display: "none !important" },
});

const setSegmentDecorationsEffect = StateEffect.define<DecorationSet>();

const segmentHighlightField = StateField.define<DecorationSet>({
  create() {
    return Decoration.none;
  },
  update(value, tr) {
    let next = value.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setSegmentDecorationsEffect)) {
        next = effect.value;
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

const setLinkedFieldsEffect = StateEffect.define<DecorationSet>();
const linkedFieldsField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(value, transaction) {
    let next = value.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(setLinkedFieldsEffect)) next = effect.value;
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

const makeFontSizeTheme = (size: number) =>
  EditorView.theme({
    "&": { fontSize: `${size}px` },
    ".cm-tooltip.cm-tooltip-autocomplete > ul": {
      fontSize: `${Math.max(size - 1, 9)}px`,
    },
  });

const MIN_FONT_SIZE = 8;
const MAX_FONT_SIZE = 24;
const DEFAULT_FONT_SIZE = 14;
const EDITOR_PREFERENCES_KEY = "jokerforge.live-code.appearance";
const readEditorPreferences = (): { fontSize: number; wordWrap: boolean } => {
  try {
    const saved = JSON.parse(localStorage.getItem(EDITOR_PREFERENCES_KEY) ?? "null");
    return {
      fontSize: Number.isInteger(saved?.fontSize) ? Math.max(MIN_FONT_SIZE, Math.min(MAX_FONT_SIZE, saved.fontSize)) : DEFAULT_FONT_SIZE,
      wordWrap: typeof saved?.wordWrap === "boolean" ? saved.wordWrap : false,
    };
  } catch {
    return { fontSize: DEFAULT_FONT_SIZE, wordWrap: false };
  }
};
const MAX_SEGMENT_LINE_DECORATIONS = 1200;

const luaParserLinter = linter((view) => getLuaDiagnostics(view.state.doc.toString()));
const fieldSegmentId = (field: LiveCodeFieldLink) => {
  const { type, ruleId, itemId } = field.target;
  return type === "trigger" ? `trigger:${ruleId}`
    : (type === "condition" || type === "effect") ? `${type}:${ruleId}:${itemId}` : `rule:${ruleId}`;
};

const LiveCodePanel: React.FC<LiveCodePanelProps> = ({
  title,
  code,
  codeResetRevision = 0,
  isLoading,
  statusMessage,
  isError = false,
  errorDetails,
  widthPercent,
  isBlockPreview,
  onBackToItem,
  onStartResize,
  onCodeChange,
  onResetCustomCode,
  hasCustomCode = false,
  segments: _segments,
  selectedSegmentId,
  hoveredSegmentId,
  fieldLinks = [],
  onNavigateToField,
  onNavigateToSegment,
  revealSegmentId,
  revealSelection,
  explanations = {},
}) => {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const editorCodeRef = useRef(code);
  const recentLocalCodeRef = useRef<string[]>([]);
  const lastPropCodeRef = useRef(code);
  const lastCodeResetRevisionRef = useRef(codeResetRevision);
  const footerUpdateTimerRef = useRef<number | null>(null);
  const onCodeChangeRef = useRef(onCodeChange);
  const isExternalUpdateRef = useRef(false);
  const fieldLinksRef = useRef(fieldLinks);
  const segmentsRef = useRef(_segments ?? []);
  const explanationsRef = useRef(explanations);
  const hasCustomCodeRef = useRef(hasCustomCode);
  const onNavigateToFieldRef = useRef(onNavigateToField);
  const onNavigateToSegmentRef = useRef(onNavigateToSegment);
  const lastRevealedSegmentRef = useRef<string | undefined>(undefined);
  const lastRevealedSelectionRef = useRef<object | null | undefined>(undefined);
  const [cursorPosition, setCursorPosition] = useState<number | null>(null);
  const [cursorStatus, setCursorStatus] = useState<LiveCodeCursorStatus>({ line: 1, column: 1, cursors: 1, selected: 0 });
  const [focused, setFocused] = useState(false);
  const [goToOpen, setGoToOpen] = useState(false);
  const [goToValue, setGoToValue] = useState("");
  const [goToError, setGoToError] = useState(false);
  const goToInputRef = useRef<HTMLInputElement>(null);
  const [outlineOpen, setOutlineOpen] = useState(false);
  const [outlineQuery, setOutlineQuery] = useState("");
  const [collapsedSymbols, setCollapsedSymbols] = useState<ReadonlySet<string>>(new Set());
  const [outlineFocusedId, setOutlineFocusedId] = useState<string | null>(null);
  const outlineTreeRef = useRef<HTMLDivElement>(null);
  const outlineOpenRef = useRef(false);
  const [parsedStructure, setParsedStructure] = useState<LiveCodeStructure>({ outline: [], folds: [], syntaxValid: true });
  const parsedStructureRef = useRef(parsedStructure);
  const [tooltipsEnabled, setTooltipsEnabled] = useState(false);
  const [resetTooltipOpen, setResetTooltipOpen] = useState(false);
  const [collapsedRanges, setCollapsedRanges] = useState<{ from: number; to: number }[]>([]);
  const collapsedRangesRef = useRef(collapsedRanges);
  const [editorMessage, setEditorMessage] = useState<string | null>(null);

  // Font size for Ctrl+scroll zoom
  const [preferences] = useState(readEditorPreferences);
  const [fontSize, setFontSize] = useState(preferences.fontSize);
  const [wordWrap, setWordWrap] = useState(preferences.wordWrap);

  onCodeChangeRef.current = onCodeChange;
  if (code === editorCodeRef.current) {
    fieldLinksRef.current = fieldLinks;
    segmentsRef.current = _segments ?? [];
  }
  outlineOpenRef.current = outlineOpen;
  explanationsRef.current = explanations;
  hasCustomCodeRef.current = hasCustomCode;
  onNavigateToFieldRef.current = onNavigateToField;
  onNavigateToSegmentRef.current = onNavigateToSegment;

  const activeField = cursorPosition === null ? undefined : fieldLinksRef.current.find((link) =>
    link.from <= cursorPosition && cursorPosition <= link.to);
  const activeSegment = cursorPosition === null ? undefined : findCodeSegmentAt(editorCodeRef.current, segmentsRef.current, cursorPosition);

  const navigateToField = useCallback((link: LiveCodeFieldLink, focus = false) => {
    if (focus) setFocused(false);
    onNavigateToFieldRef.current?.(link, focus);
  }, []);

  const navigateToSegment = useCallback((segmentId: string, revealBuilder = false) => {
    if (revealBuilder) setFocused(false);
    onNavigateToSegmentRef.current?.(segmentId, revealBuilder);
  }, []);

  const openSearch = useCallback((replace = false) => {
    const view = viewRef.current;
    if (!view) return false;
    return openLiveCodeSearch(view, replace);
  }, []);

  const openGoTo = useCallback(() => {
    const view = viewRef.current;
    if (!view) return false;
    setGoToValue(String(view.state.doc.lineAt(view.state.selection.main.head).number));
    setGoToError(false);
    setGoToOpen(true);
    return true;
  }, []);

  useEffect(() => {
    if (goToOpen) {
      goToInputRef.current?.focus();
      goToInputRef.current?.select();
    }
  }, [goToOpen]);

  const formatCode = useCallback(() => {
    const view = viewRef.current;
    if (!view || view.state.readOnly) return false;
    const selection = view.state.selection.main;
    const currentIndentUnit = view.state.facet(indentUnit);
    const result = formatLuaCode(view.state.doc.toString(), {
      indentUnit: currentIndentUnit,
      ...(!selection.empty ? { from: selection.from, to: selection.to } : {}),
    });
    if (result.error) {
      setEditorMessage(result.error);
      return true;
    }
    if (result.changed) {
      view.dispatch({ changes: result.edits, effects: formatIndentEffect.of(currentIndentUnit), userEvent: "input.format", annotations: isolateHistory.of("full"), scrollIntoView: true });
    }
    setEditorMessage(null);
    view.focus();
    return true;
  }, []);

  const revealCode = useCallback((from: number, to = from, select = false) => {
    const view = viewRef.current;
    if (!view) return;
    const effects: StateEffect<unknown>[] = [];
    foldedRanges(view.state).between(0, view.state.doc.length, (foldFrom, foldTo) => {
      if (foldFrom <= to && foldTo >= from) effects.push(unfoldEffect.of({ from: foldFrom, to: foldTo }));
    });
    effects.push(EditorView.scrollIntoView(from, { y: "center" }));
    view.dispatch({ effects, ...(select ? { selection: { anchor: from } } : {}) });
    if (select) view.focus();
  }, []);

  // Ctrl+scroll to change font size
  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!e.ctrlKey && !e.metaKey) return;
    if (e.cancelable) {
      e.preventDefault();
    }
    setFontSize((prev) => {
      const delta = e.deltaY > 0 ? -1 : 1;
      return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, prev + delta));
    });
  }, []);

  // Update CM font size when state changes
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: fontSizeCompartment.reconfigure(makeFontSizeTheme(fontSize)),
    });
  }, [fontSize]);

  useEffect(() => {
    viewRef.current?.dispatch({ effects: wrappingCompartment.reconfigure(wordWrap ? EditorView.lineWrapping : []) });
  }, [wordWrap]);

  useEffect(() => {
    try { localStorage.setItem(EDITOR_PREFERENCES_KEY, JSON.stringify({ fontSize, wordWrap })); } catch { /* Storage may be unavailable. */ }
  }, [fontSize, wordWrap]);

  useEffect(() => { viewRef.current?.requestMeasure(); }, [focused]);

  // Editable when not in block preview and onCodeChange is provided
  const isEditable = !isBlockPreview && !!onCodeChange;

  const displayCode = code;
  const segments = _segments ?? [];
  const outlineLabels = useMemo(() => new Map(Object.values(explanations)
    .filter((explanation) => explanation.blockType === "rule")
    .map((explanation) => [explanation.segmentId, explanation.title])), [explanations]);
  const structure = useMemo(() => {
    if (!outlineOpen) return { ...parsedStructure, outline: [] };
    const current = viewRef.current?.state.field(liveCodeStructureField) ?? parsedStructure;
    return combineKnownLiveCodeStructure(current, editorCodeRef.current, segmentsRef.current, outlineLabels);
  }, [outlineOpen, parsedStructure, displayCode, _segments, outlineLabels]);
  const outlineTree = useMemo(() => buildLiveCodeOutlineTree(structure.outline), [structure.outline]);
  const filteredOutline = useMemo(() => filterLiveCodeOutlineTree(outlineTree, outlineQuery), [outlineTree, outlineQuery]);
  const visibleOutline = useMemo(() => flattenLiveCodeOutlineTree(filteredOutline, outlineQuery.trim() ? undefined : collapsedSymbols),
    [filteredOutline, outlineQuery, collapsedSymbols]);
  const activeOutline = cursorPosition === null ? undefined : getActiveLiveCodeOutlineEntry(filteredOutline, cursorPosition, outlineQuery.trim() ? undefined : collapsedSymbols);
  const outlineTabId = visibleOutline.find((entry) => entry.id === outlineFocusedId)?.id ?? activeOutline?.id ?? visibleOutline[0]?.id;
  const toggleSymbol = (id: string, collapsed?: boolean) => setCollapsedSymbols((previous) => {
    const next = new Set(previous);
    if (collapsed ?? !next.has(id)) next.add(id); else next.delete(id);
    return next;
  });
  const focusOutlineEntry = (id: string) => {
    setOutlineFocusedId(id);
    const row = Array.from(outlineTreeRef.current?.querySelectorAll<HTMLDivElement>("[data-outline-id]") ?? []).find((element) => element.dataset.outlineId === id);
    row?.focus();
  };
  const outlineFold = (entry: LiveCodeOutlineEntry) => structure.folds.find((range) => range.from >= entry.from && range.to <= entry.to);
  const toggleOutlineFold = (entry: LiveCodeOutlineEntry) => {
    const view = viewRef.current;
    if (!view) return;
    const range = outlineFold(entry);
    if (!range) return;
    const collapsed = collapsedRanges.some((fold) => fold.from === range.from && fold.to === range.to);
    view.dispatch({ effects: collapsed ? unfoldEffect.of(range) : foldEffect.of(range) });
  };
  const selectedSegmentExists = !!(
    selectedSegmentId && (segments.some((segment) => segment.id === selectedSegmentId)
      || fieldLinks.some((field) => fieldSegmentId(field) === selectedSegmentId))
  );
  const hoveredSegmentExists = !!(
    hoveredSegmentId && (segments.some((segment) => segment.id === hoveredSegmentId)
      || fieldLinks.some((field) => fieldSegmentId(field) === hoveredSegmentId))
  );

  const builderHoverTooltips = useMemo(() => hoverTooltip((view, position, side) => {
    const link = fieldLinksRef.current.find((field) => field.from <= position && position <= field.to
      && !(position === field.from && side < 0) && !(position === field.to && side > 0));
    const segment = findCodeSegmentAt(view.state.doc.toString(), segmentsRef.current, position);
    const explanation = explanationsRef.current[link ? fieldSegmentId(link) : segment?.id ?? ""];
    if (!link && !explanation) return null;
    const segmentRange = segment && getCodeSegmentRanges(view.state.doc.toString(), [segment])[0];
    return {
      pos: link?.from ?? segmentRange?.from ?? position,
      end: link?.to ?? segmentRange?.to ?? position,
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "jf-field-tooltip";
        const label = dom.appendChild(document.createElement("strong"));
        label.textContent = link?.label ?? explanation?.title ?? "Builder rule";
        if (explanation) {
          const summary = dom.appendChild(document.createElement("div"));
          summary.className = "jf-code-explanation";
          summary.textContent = explanation.lines.join("\n");
          if (hasCustomCodeRef.current) {
            const note = dom.appendChild(document.createElement("div"));
            note.className = "jf-code-explanation";
            note.textContent = "Describes the builder rule. Edited Lua may behave differently.";
          }
        }
        if (link) {
          const allowed = dom.appendChild(document.createElement("div"));
          allowed.textContent = `Allowed: ${link.allowedValues}`;
        }
        if (link && onNavigateToFieldRef.current) {
          const button = dom.appendChild(document.createElement("button"));
          button.type = "button";
          button.textContent = "Open field in builder";
          button.addEventListener("click", () => navigateToField(link, true));
        } else if (explanation && onNavigateToSegmentRef.current) {
          const button = dom.appendChild(document.createElement("button"));
          button.type = "button";
          button.textContent = "Open block in builder";
          button.addEventListener("click", () => navigateToSegment(explanation.segmentId, true));
        }
        return { dom };
      },
    };
  }, { hoverTime: 250 }), [navigateToField, navigateToSegment]);

  const tooltipExtensions = useCallback((enabled: boolean) => [
    linter(null, { tooltipFilter: enabled ? null : () => [], hideOn: enabled ? undefined : () => true }),
    enabled ? builderHoverTooltips : tooltipsOffTheme,
  ], [builderHoverTooltips]);

  const buildSegmentHighlightDecorations = useCallback(() => {
    if (!segments.length || (!selectedSegmentId && !hoveredSegmentId)) {
      return Decoration.none;
    }
    const lineStarts = [0];
    for (let i = 0; i < displayCode.length; i += 1) {
      if (displayCode[i] === "\n") lineStarts.push(i + 1);
    }
    const numericField = (
      segment: CodeSegment,
      camelCaseKey: keyof CodeSegment,
      snakeCaseKey: string,
    ): number | null => {
      const raw =
        (segment as unknown as Record<string, unknown>)[camelCaseKey as string] ??
        (segment as unknown as Record<string, unknown>)[snakeCaseKey];

      if (typeof raw === "number") {
        return Number.isFinite(raw) ? raw : null;
      }

      if (typeof raw === "string") {
        const parsed = Number(raw);
        return Number.isFinite(parsed) ? parsed : null;
      }

      return null;
    };

    const ranges: Range<Decoration>[] = [];
    let targetedSegments = 0;
    let lineDecorationCount = 0;
    for (const segment of segments) {
      let lineClass = "";
      if (selectedSegmentId && segment.id === selectedSegmentId) {
        lineClass = "jf-segment-selected-line";
      } else if (hoveredSegmentId && segment.id === hoveredSegmentId) {
        lineClass = "jf-segment-hover-line";
      }
      if (!lineClass) continue;
      targetedSegments += 1;

      const startLine = numericField(segment, "startLine", "start_line");
      const startColumn = numericField(segment, "startColumn", "start_column");
      const endLine = numericField(segment, "endLine", "end_line");
      const endColumn = numericField(segment, "endColumn", "end_column");
      if (startLine === null || endLine === null) {
        continue;
      }
      const normalizedStartLine = Math.max(1, Math.floor(startLine));
      const normalizedEndLine = Math.max(1, Math.floor(endLine));
      const normalizedStartColumn = Math.max(
        1,
        Math.floor(startColumn ?? 1),
      );
      const normalizedEndColumn = Math.max(1, Math.floor(endColumn ?? 1));

      const maxLine = lineStarts.length;
      const fromLine = Math.max(1, Math.min(maxLine, normalizedStartLine));
      const toLine = Math.max(fromLine, Math.min(maxLine, normalizedEndLine));

      const fromLineStart = lineStarts[fromLine - 1];
      const afterToLineStart =
        toLine < maxLine ? lineStarts[toLine] : displayCode.length;
      if (fromLineStart === undefined || afterToLineStart === undefined) {
        continue;
      }

      const fromIndex = Math.min(
        displayCode.length,
        Math.max(0, fromLineStart + (normalizedStartColumn - 1)),
      );
      const toLineStart = lineStarts[toLine - 1] ?? fromLineStart;
      const toIndexExclusive = Math.min(
        displayCode.length,
        Math.max(0, toLineStart + (normalizedEndColumn - 1)),
      );
      const toIndex = Math.max(fromIndex, toIndexExclusive);
      let clippedFromIndex = fromIndex;
      let clippedToIndex = Math.min(afterToLineStart, toIndex);

      // Trim leading/trailing whitespace so we only highlight text.
      while (
        clippedFromIndex < clippedToIndex &&
        /\s/.test(displayCode[clippedFromIndex] ?? "")
      ) {
        clippedFromIndex += 1;
      }
      while (
        clippedToIndex > clippedFromIndex &&
        /\s/.test(displayCode[clippedToIndex - 1] ?? "")
      ) {
        clippedToIndex -= 1;
      }

      if (clippedFromIndex >= clippedToIndex) {
        continue;
      }

      const startLineIdx = (() => {
        let idx = 0;
        while (idx + 1 < lineStarts.length && lineStarts[idx + 1] <= clippedFromIndex) {
          idx += 1;
        }
        return idx + 1;
      })();
      const endLineIdx = (() => {
        let idx = 0;
        const inclusiveEnd = Math.max(clippedFromIndex, clippedToIndex - 1);
        while (idx + 1 < lineStarts.length && lineStarts[idx + 1] <= inclusiveEnd) {
          idx += 1;
        }
        return idx + 1;
      })();

      if (lineDecorationCount < MAX_SEGMENT_LINE_DECORATIONS) {
        if (startLineIdx === endLineIdx) {
          ranges.push(
            Decoration.mark({
              class:
                lineClass === "jf-segment-selected-line"
                  ? "cm-jf-segment-selected-range"
                  : "cm-jf-segment-hover-range",
            }).range(clippedFromIndex, clippedToIndex),
          );
          lineDecorationCount += 1;
        } else {
          for (let line = startLineIdx; line <= endLineIdx; line += 1) {
            if (lineDecorationCount >= MAX_SEGMENT_LINE_DECORATIONS) break;
            const lineStart = lineStarts[line - 1];
            if (lineStart === undefined) continue;
            ranges.push(Decoration.line({ class: lineClass }).range(lineStart));
            lineDecorationCount += 1;
          }
        }
      }
    }

    return Decoration.set(ranges, true);
  }, [
    displayCode,
    hoveredSegmentId,
    segments,
    selectedSegmentId,
  ]);

  // Create editor on mount
  useEffect(() => {
    if (!editorRef.current) return;

    const refreshCursorFooter = () => {
      const view = viewRef.current;
      if (!view) return;
      setCursorPosition(view.state.selection.main.head);
      const status = getLiveCodeCursorStatus(view.state);
      setCursorStatus((previous) => previous.line === status.line && previous.column === status.column &&
        previous.cursors === status.cursors && previous.selected === status.selected ? previous : status);
      const ranges: { from: number; to: number }[] = [];
      foldedRanges(view.state).between(0, view.state.doc.length, (from, to) => { ranges.push({ from, to }); });
      const previous = collapsedRangesRef.current;
      if (previous.length !== ranges.length || ranges.some((range, index) =>
        range.from !== previous[index].from || range.to !== previous[index].to)) {
        collapsedRangesRef.current = ranges;
        setCollapsedRanges(ranges);
      }
    };

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        if (footerUpdateTimerRef.current !== null) window.clearTimeout(footerUpdateTimerRef.current);
        footerUpdateTimerRef.current = window.setTimeout(() => {
          footerUpdateTimerRef.current = null;
          refreshCursorFooter();
        }, 120);
      } else if (update.selectionSet || update.transactions.some((transaction) => transaction.effects.some((effect) =>
        effect.is(foldEffect) || effect.is(unfoldEffect)))) {
        refreshCursorFooter();
      }
      const structureUpdated = update.transactions.some((transaction) => transaction.effects.some((effect) =>
        effect.is(setLiveCodeStructureEffect)));
      const nextStructure = update.state.field(liveCodeStructureField);
      if (outlineOpenRef.current && (structureUpdated || (nextStructure.pending && !parsedStructureRef.current.pending))) {
        parsedStructureRef.current = nextStructure;
        setParsedStructure(nextStructure);
      }
      if (update.docChanged && !isExternalUpdateRef.current) {
        setEditorMessage(null);
        const newCode = update.state.doc.toString();
        const changes: CodeEdit[] = [];
        update.changes.iterChanges((from, to, _fromNew, _toNew, inserted) => {
          changes.push({ from, to, insert: inserted.toString() });
        });
        fieldLinksRef.current = updateBoundRanges(editorCodeRef.current, newCode, fieldLinksRef.current, changes) as LiveCodeFieldLink[];
        segmentsRef.current = mapCodeSegmentsThroughEdits(editorCodeRef.current, newCode, segmentsRef.current, changes);
        editorCodeRef.current = newCode;
        recentLocalCodeRef.current.push(newCode);
        if (recentLocalCodeRef.current.length > 64) recentLocalCodeRef.current.shift();
        onCodeChangeRef.current?.(newCode, changes, getFormatPresentationChange(update.transactions));
      }
    });

    const state = EditorState.create({
      doc: displayCode,
      extensions: [
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        drawSelection(),
        dropCursor(),
        rectangularSelection(),
        crosshairCursor({ key: "Alt" }),
        history(),
        formatPresentationHistory,
        StreamLanguage.define(lua),
        indentUnit.of("  "),
        EditorState.tabSize.of(2),
        bracketMatching(),
        liveCodeFolding(() => segmentsRef.current),
        foldGutter({ openText: "▾", closedText: "▸", foldingChanged: (update) => update.transactions.some((transaction) =>
          transaction.effects.some((effect) => effect.is(setSegmentDecorationsEffect) || effect.is(setLiveCodeStructureEffect))) }),
        liveCodeSearch(),
        EditorView.contentAttributes.of({ "aria-label": `${title} Lua code` }),
        oneDark,
        editorTheme,
        Prec.high(liveCodeEditorAppearance()),
        fontSizeCompartment.of(makeFontSizeTheme(fontSize)),
        keymap.of([
          { key: "Mod-f", scope: "editor search-panel", run: () => openSearch() },
          { key: "Mod-h", scope: "editor search-panel", run: () => openSearch(true) },
          { key: "Mod-g", run: openGoTo, preventDefault: true },
          { key: "Mod-Shift-f", run: formatCode },
          { key: "Alt-Shift-f", run: formatCode, preventDefault: true },
          {
            key: "F12",
            preventDefault: true,
            run: (view) => {
              const position = view.state.selection.main.head;
              const link = fieldLinksRef.current.find((field) => field.from <= position && position <= field.to);
              if (link && onNavigateToFieldRef.current) {
                navigateToField(link, true);
                return true;
              }
              const segment = findCodeSegmentAt(view.state.doc.toString(), segmentsRef.current, position);
              if (segment && onNavigateToSegmentRef.current) {
                navigateToSegment(segment.id, true);
                return true;
              }
              return false;
            },
          },
          {
            key: "Enter",
            run: (view) => {
              if (view.state.readOnly) return false;
              if (completionStatus(view.state) !== "active") {
                return insertNewlineAndIndent(view);
              }
              return acceptCompletion(view);
            },
          },
          ...liveCodeEditorKeymap,
          ...closeBracketsKeymap,
          ...defaultKeymap,
          ...historyKeymap,
          ...searchKeymap,
          ...foldKeymap,
          ...completionKeymap,
        ]),
        closeBrackets(),
        autocompletion({
          override: [luaSmodsCompletions],
          activateOnTyping: true,
          maxRenderedOptions: 30,
        }),
        luaParserLinter,
        tooltipCompartment.of(tooltipExtensions(false)),
        EditorView.domEventHandlers({
          mouseup(event, view) {
            const target = event.target;
            if (!(target instanceof Element) || !view.contentDOM.contains(target) || target.closest(".cm-foldPlaceholder")) return false;
            if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey ||
              view.state.selection.ranges.length > 1 || !view.state.selection.main.empty) return false;
            const position = view.posAtCoords({ x: event.clientX, y: event.clientY });
            if (position === null) return false;
            const link = fieldLinksRef.current.find((field) => field.from <= position && position < field.to);
            if (link) navigateToField(link);
            else {
              const segment = findCodeSegmentAt(view.state.doc.toString(), segmentsRef.current, position);
              if (segment) navigateToSegment(segment.id);
            }
            return false;
          },
        }),
        EditorState.allowMultipleSelections.of(true),
        readOnlyCompartment.of(EditorState.readOnly.of(!isEditable)),
        segmentHighlightField,
        linkedFieldsField,
        updateListener,
        wrappingCompartment.of(wordWrap ? EditorView.lineWrapping : []),
      ],
    });

    const view = new EditorView({
      state,
      parent: editorRef.current,
    });

    viewRef.current = view;
    refreshCursorFooter();

    return () => {
      if (footerUpdateTimerRef.current !== null) window.clearTimeout(footerUpdateTimerRef.current);
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  // Update editor content when code changes
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;

    const currentCode = view.state.doc.toString();

    const explicitReset = lastCodeResetRevisionRef.current !== codeResetRevision;
    lastCodeResetRevisionRef.current = codeResetRevision;
    if (explicitReset) recentLocalCodeRef.current = [];
    const propCodeChanged = lastPropCodeRef.current !== displayCode;
    lastPropCodeRef.current = displayCode;
    if (!explicitReset && currentCode !== displayCode && (recentLocalCodeRef.current.includes(displayCode)
      || (!propCodeChanged && recentLocalCodeRef.current.length > 0))) return;

    if (currentCode !== displayCode) {
      let from = 0;
      while (from < currentCode.length && from < displayCode.length && currentCode[from] === displayCode[from]) {
        from += 1;
      }
      let oldEnd = currentCode.length;
      let newEnd = displayCode.length;
      while (oldEnd > from && newEnd > from && currentCode[oldEnd - 1] === displayCode[newEnd - 1]) {
        oldEnd -= 1;
        newEnd -= 1;
      }
      isExternalUpdateRef.current = true;
      view.dispatch({
        changes: {
          from,
          to: oldEnd,
          insert: displayCode.slice(from, newEnd),
        },
        annotations: Transaction.addToHistory.of(false),
      });
      isExternalUpdateRef.current = false;
      editorCodeRef.current = displayCode;
      fieldLinksRef.current = fieldLinks;
      segmentsRef.current = segments;

    }
    recentLocalCodeRef.current = [];
  }, [displayCode, codeResetRevision]);

  useEffect(() => {
    if (!outlineOpen || !viewRef.current) return;
    const cached = viewRef.current.state.field(liveCodeStructureField);
    parsedStructureRef.current = cached;
    setParsedStructure(cached);
  }, [outlineOpen]);

  // Toggle read-only based on block preview / onCodeChange
  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: readOnlyCompartment.reconfigure(
        EditorState.readOnly.of(!isEditable),
      ),
    });
    closeSearchPanel(view);
  }, [isEditable]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: [tooltipCompartment.reconfigure(tooltipExtensions(tooltipsEnabled)), closeHoverTooltips],
    });
    if (!tooltipsEnabled) setResetTooltipOpen(false);
  }, [tooltipsEnabled, tooltipExtensions]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || displayCode !== editorCodeRef.current) return;
    view.dispatch({
      effects: setSegmentDecorationsEffect.of(buildSegmentHighlightDecorations()),
    });
  }, [buildSegmentHighlightDecorations]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || displayCode !== editorCodeRef.current) return;
    const marks = fieldLinks.filter((range) =>
      range.from >= 0 && range.to > range.from && range.to <= displayCode.length,
    ).map((range) => Decoration.mark({
      class: "cm-jf-linked-field",
      attributes: { "aria-label": `${range.label}. Allowed: ${range.allowedValues}` },
    }).range(range.from, range.to));
    view.dispatch({ effects: setLinkedFieldsEffect.of(Decoration.set(marks, true)) });
  }, [displayCode, fieldLinks]);

  useEffect(() => {
    const view = viewRef.current;
    if (!revealSegmentId) {
      lastRevealedSegmentRef.current = undefined;
      lastRevealedSelectionRef.current = undefined;
      return;
    }
    if (!view || (lastRevealedSegmentRef.current === revealSegmentId
      && lastRevealedSelectionRef.current === revealSelection)) return;
    const target = getCodeSegmentRanges(editorCodeRef.current, segmentsRef.current).find(({ segment }) => segment.id === revealSegmentId)
      ?? fieldLinksRef.current.find((field) => fieldSegmentId(field) === revealSegmentId);
    if (!target) return;
    lastRevealedSegmentRef.current = revealSegmentId;
    lastRevealedSelectionRef.current = revealSelection;
    revealCode(target.from, target.to);
  }, [displayCode, revealSegmentId, revealSelection, segments, fieldLinks, revealCode]);

  return (
    <aside
      data-rb-live-code="true"
      data-editor-focused={focused}
      aria-busy={isLoading}
      className={`${focused ? "absolute inset-0 z-40 bg-card" : "relative bg-card/95 backdrop-blur-md border-l border-border"} h-full`}
      style={{ width: focused ? "100%" : `${widthPercent}%` }}
    >
      {/* Full-height resize edge */}
      {!focused && <div
        className="absolute left-0 top-0 bottom-0 -translate-x-1/2 z-20 w-3 cursor-col-resize group"
        onMouseDown={onStartResize}
      >
        <div className="mx-auto h-full w-px bg-border/70 group-hover:bg-primary/55 group-active:bg-primary/75 transition-colors duration-150" />
      </div>}

      <div className="h-full flex flex-col">
        {/* Header bar */}
        <div className="min-h-10 px-3 py-1.5 border-b border-border/80 flex items-center justify-between gap-3 bg-card/70">
          <div className="flex items-center gap-2 min-w-0">
            <BracketsCurly className="h-4 w-4 text-muted-foreground shrink-0" />
            <span className="text-xs font-medium text-foreground truncate">{title}</span>
            {hasCustomCode && !isBlockPreview && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-400 font-medium shrink-0">
                Edited
              </span>
            )}
            {!isBlockPreview && (selectedSegmentId || hoveredSegmentId) && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-primary/15 text-primary font-medium shrink-0">
                {selectedSegmentExists || hoveredSegmentExists ? "Linked" : "No Link"}
              </span>
            )}
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            {/* Reset custom code */}
            {hasCustomCode && onResetCustomCode && !isBlockPreview && (
              <Tooltip open={tooltipsEnabled && resetTooltipOpen} onOpenChange={(open) => setResetTooltipOpen(tooltipsEnabled && open)}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="icon"
                    variant="outline"
                    aria-label="Reset all custom code changes"
                    onClick={onResetCustomCode}
                    className="h-8 w-8 rounded-lg border-2 transition-all duration-200 cursor-pointer bg-card/90 border-amber-400/40 text-amber-400 hover:bg-amber-400/10 hover:border-amber-400/60"
                    icon={<ArrowsClockwise className="h-3.5 w-3.5" />}
                  />
                </TooltipTrigger>
                <TooltipContent
                  side="bottom"
                  sideOffset={6}
                  className="text-xs font-medium"
                >
                  Reset all custom code changes
                </TooltipContent>
              </Tooltip>
            )}

            {/* Back to full item view */}
            {isBlockPreview && (
              <Button
                variant="outline"
                size="sm"
                className="h-7 text-xs cursor-pointer"
                onClick={onBackToItem}
              >
                <ArrowCounterClockwise className="h-3.5 w-3.5 mr-1" />
                Full Item View
              </Button>
            )}

            <Button type="button" variant="ghost" size="icon" className="h-7 w-7" aria-label={focused ? "Exit focused editor" : "Focus editor"}
              aria-pressed={focused} title={tooltipsEnabled ? (focused ? "Exit focused editor" : "Focus editor") : undefined}
              onClick={() => { setFocused((value) => !value); viewRef.current?.focus(); }}>
              {focused ? <ArrowsInSimple className="h-4 w-4" /> : <ArrowsOutSimple className="h-4 w-4" />}
            </Button>
          </div>
        </div>

        {goToOpen && <form className="absolute z-30 right-3 top-12 w-64 max-w-[calc(100%_-_1.5rem)] p-3 rounded-md border border-border bg-card shadow-xl"
          onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget)) setGoToOpen(false); }}
          aria-label="Go to line" onSubmit={(event) => {
            event.preventDefault();
            const view = viewRef.current;
            const position = view ? getLiveCodeGoToPosition(view.state.doc, goToValue, view.state.tabSize) : null;
            if (position === null) { setGoToError(true); return; }
            revealCode(position, position, true);
            setGoToOpen(false);
          }} onKeyDown={(event) => {
            if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setGoToOpen(false); viewRef.current?.focus(); }
          }}>
          <label htmlFor="live-code-go-to" className="block mb-2 text-xs font-medium">Go to line</label>
          <div className="flex gap-2">
            <input id="live-code-go-to" ref={goToInputRef} value={goToValue} placeholder="Line:column" autoComplete="off" spellCheck={false}
              aria-invalid={goToError} aria-describedby={goToError ? "live-code-go-to-error" : undefined}
              className="min-w-0 flex-1 h-8 px-2 text-xs font-mono rounded border border-border bg-background focus-visible:outline focus-visible:outline-primary"
              onChange={(event) => { setGoToValue(event.target.value); setGoToError(false); }} />
            <Button type="submit" size="sm" className="h-8 text-xs">Go</Button>
          </div>
          {goToError && <p id="live-code-go-to-error" role="alert" className="mt-2 text-xs text-destructive">Enter a line number or line:column.</p>}
        </form>}

        <div role="toolbar" aria-label="Lua editing controls" className="px-2 py-1 flex items-center gap-1 border-b border-border/60 bg-muted/10">
          <Button size="sm" variant={outlineOpen ? "secondary" : "ghost"} className="h-7 text-xs"
            aria-expanded={outlineOpen} aria-controls="live-code-outline" onClick={() => setOutlineOpen((open) => !open)}>
            <ListTree className="h-3.5 w-3.5 mr-1.5" />Outline
          </Button>
          <Button size="sm" variant="ghost" className="h-7 text-xs" aria-label="Find and replace" title={tooltipsEnabled ? "Find and replace (Ctrl+F / Ctrl+H)" : undefined} onClick={() => openSearch()}>
            <Search className="h-3.5 w-3.5 mr-1.5" />Find & replace
          </Button>
          <div className="flex-1" />
          <Button size="icon" variant="outline" className="h-7 w-7 rounded-md" disabled={!isEditable} aria-label="Format Lua code"
            title={tooltipsEnabled ? "Format code (Shift+Alt+F)" : undefined} onClick={formatCode}><AlignLeft className="h-3.5 w-3.5" /></Button>
          <Button size="icon" variant={tooltipsEnabled ? "secondary" : "ghost"} className="h-7 w-7 rounded-md"
            aria-label="Live Code tooltips" aria-pressed={tooltipsEnabled} title={tooltipsEnabled ? "Disable tooltips" : undefined}
            onClick={() => setTooltipsEnabled((enabled) => !enabled)}><Info className="h-3.5 w-3.5" /></Button>
        </div>

        {editorMessage && <p role="status" aria-live="polite" className="px-4 py-2 text-xs text-muted-foreground border-b border-border/50">{editorMessage}</p>}

        {statusMessage ? (
          <div
            className={`mx-4 mt-4 rounded-md border px-3 py-2 text-xs flex items-start gap-2 ${
              isError
                ? "border-transparent bg-transparent text-destructive"
                : "border-amber-400/40 bg-amber-400/10 text-amber-200"
            }`}
          >
            <WarningCircle className="h-4 w-4 mt-0.5 shrink-0" />
            <div className="min-w-0 w-full">
              <div className="font-semibold">{statusMessage}</div>
              {isError && errorDetails ? (
                <pre className="mt-2 max-h-56 overflow-auto whitespace-pre-wrap wrap-break-word px-0 py-0 text-[11px] leading-relaxed text-destructive/90">
                  {errorDetails}
                </pre>
              ) : null}
            </div>
          </div>
        ) : null}

        <div className="flex flex-1 min-h-0" onWheel={handleWheel}>
          {outlineOpen && (
            <nav id="live-code-outline" aria-label="Lua code outline" className="w-56 max-w-[40%] shrink-0 border-r border-border/60 flex flex-col bg-muted/10">
              <div className="pl-3 pr-1 py-1.5 flex items-center gap-0.5">
                <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground flex-1">Outline</span>
                <Button size="icon" variant="ghost" className="h-6 w-6" aria-label="Collapse outline tree" disabled={!outlineTree.length || !!outlineQuery.trim()}
                  onClick={() => setCollapsedSymbols(new Set(flattenLiveCodeOutlineTree(outlineTree).filter((entry) => entry.children.length).map((entry) => entry.id)))}><ChevronsUp className="h-3.5 w-3.5" /></Button>
                <Button size="icon" variant="ghost" className="h-6 w-6" aria-label="Expand outline tree" disabled={!collapsedSymbols.size || !!outlineQuery.trim()}
                  onClick={() => setCollapsedSymbols(new Set())}><ChevronsDown className="h-3.5 w-3.5" /></Button>
                <Button size="icon" variant="ghost" className="h-6 w-6" aria-label="Fold all code sections" title={tooltipsEnabled ? "Fold all code sections" : undefined}
                  onClick={() => { if (viewRef.current) foldAll(viewRef.current); }}><FoldVertical className="h-3.5 w-3.5" /></Button>
                <Button size="icon" variant="ghost" className="h-6 w-6" aria-label="Unfold all code sections" title={tooltipsEnabled ? "Unfold all code sections" : undefined}
                  onClick={() => { if (viewRef.current) unfoldAll(viewRef.current); }}><UnfoldVertical className="h-3.5 w-3.5" /></Button>
              </div>
              <div className="px-2 pb-2">
                <input type="search" aria-label="Filter outline symbols" placeholder="Filter symbols…" value={outlineQuery} spellCheck={false}
                  className="h-7 w-full min-w-0 px-2 text-xs rounded border border-border/70 bg-background/60 focus-visible:outline focus-visible:outline-primary"
                  onChange={(event) => setOutlineQuery(event.target.value)} onKeyDown={(event) => {
                    if (event.key === "ArrowDown" && visibleOutline.length) { event.preventDefault(); focusOutlineEntry(outlineTabId ?? visibleOutline[0].id); }
                    if (event.key === "Escape" && outlineQuery) { event.preventDefault(); event.stopPropagation(); setOutlineQuery(""); }
                  }} />
              </div>
              <div ref={outlineTreeRef} role="tree" aria-label="Lua symbols" className="flex-1 min-h-0 overflow-auto pb-2">
                {visibleOutline.map((entry, index) => {
                  const fold = outlineFold(entry);
                  const collapsed = !!fold && collapsedRanges.some((range) => range.from === fold.from && range.to === fold.to);
                  const symbolCollapsed = !outlineQuery.trim() && collapsedSymbols.has(entry.id);
                  const Icon = entry.kind === "configuration" ? Braces : entry.kind === "localization" ? Languages : entry.kind === "rule" ? Layers : CodeXml;
                  const iconColor = entry.kind === "configuration" ? "text-sky-400" : entry.kind === "localization" ? "text-amber-400" : entry.kind === "rule" ? "text-primary" : "text-violet-400";
                  const line = viewRef.current?.state.doc.lineAt(Math.min(entry.from, viewRef.current.state.doc.length)).number;
                  return (
                    <div key={entry.id} data-outline-id={entry.id} role="treeitem" aria-level={entry.depth + 1} aria-label={`${entry.label}, ${entry.kind}${line ? `, line ${line}` : ""}`}
                      aria-selected={activeOutline?.id === entry.id} aria-expanded={entry.children.length ? !symbolCollapsed : undefined}
                      tabIndex={outlineTabId === entry.id ? 0 : -1}
                      className={`group h-7 flex items-center gap-1 pr-1 cursor-pointer outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-primary ${activeOutline?.id === entry.id ? "bg-primary/12 text-foreground" : "text-muted-foreground hover:bg-muted/60 hover:text-foreground"}`}
                      style={{ paddingLeft: `${Math.min(entry.depth, 8) * 12 + 4}px` }} onFocus={() => setOutlineFocusedId(entry.id)}
                      onClick={() => revealCode(entry.from, fold?.to ?? entry.from, true)} onKeyDown={(event) => {
                        if (event.target !== event.currentTarget) return;
                        const key = event.key;
                        if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End") {
                          event.preventDefault();
                          const target = key === "Home" ? 0 : key === "End" ? visibleOutline.length - 1 : Math.max(0, Math.min(visibleOutline.length - 1, index + (key === "ArrowDown" ? 1 : -1)));
                          focusOutlineEntry(visibleOutline[target].id);
                        } else if (key === "ArrowRight") {
                          event.preventDefault();
                          if (entry.children.length) { if (symbolCollapsed) toggleSymbol(entry.id, false); else focusOutlineEntry(entry.children[0].id); }
                        } else if (key === "ArrowLeft") {
                          event.preventDefault();
                          if (entry.children.length && !symbolCollapsed && !outlineQuery.trim()) toggleSymbol(entry.id, true); else if (entry.parentId) focusOutlineEntry(entry.parentId);
                        } else if (key === "Enter" || key === " ") { event.preventDefault(); revealCode(entry.from, fold?.to ?? entry.from, true); }
                      }}>
                      {entry.children.length ? <button type="button" tabIndex={-1} aria-label={`${symbolCollapsed ? "Expand" : "Collapse"} outline ${entry.label}`}
                        disabled={!!outlineQuery.trim()}
                        className="h-6 w-4 shrink-0 flex items-center justify-center" onClick={(event) => { event.stopPropagation(); toggleSymbol(entry.id); }}>
                        {symbolCollapsed ? <ChevronRight className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                      </button> : <span className="w-4 shrink-0" />}
                      <Icon className={`h-3.5 w-3.5 shrink-0 ${iconColor}`} />
                      <span className="truncate min-w-0 flex-1 text-xs" title={tooltipsEnabled ? entry.label : undefined}>{entry.label}</span>
                      <span className="text-[10px] tabular-nums text-muted-foreground/60 group-hover:hidden">{line}</span>
                      {fold && <button type="button" tabIndex={-1} aria-label={`${collapsed ? "Unfold" : "Fold"} code for ${entry.label}`}
                        className="hidden group-hover:flex group-focus-within:flex items-center justify-center h-6 w-5 shrink-0 text-muted-foreground hover:text-foreground"
                        onClick={(event) => { event.stopPropagation(); toggleOutlineFold(entry); }}>
                        {collapsed ? <UnfoldVertical className="h-3 w-3" /> : <FoldVertical className="h-3 w-3" />}
                      </button>}
                    </div>
                  );
                })}
                {visibleOutline.length === 0 && <p className="px-3 py-2 text-xs text-muted-foreground">{outlineQuery ? "No matching symbols." : "No symbols found."}</p>}
              </div>
              {!structure.pending && !structure.syntaxValid && <p className="px-2 py-2 text-[10px] text-muted-foreground border-t border-border/50">
                Complete Lua syntax to see all symbols.
              </p>}
            </nav>
          )}
          <div
            ref={editorRef}
            className="h-full flex-1 min-w-0 overflow-hidden [&_.cm-editor]:h-full [&_.cm-editor]:bg-transparent!"
          />
        </div>
        <div role="toolbar" aria-label="Editor status and appearance" className="min-h-7 px-2 border-t border-border/60 bg-muted/20 flex flex-wrap items-center justify-between gap-x-3 text-[11px] text-muted-foreground">
          <div className="flex items-center min-w-0 max-w-full">
          <button type="button" aria-label={`Go to line. Line ${cursorStatus.line}, column ${cursorStatus.column}`} className="h-7 px-1.5 hover:bg-muted focus-visible:outline focus-visible:outline-primary tabular-nums"
            title={tooltipsEnabled ? "Go to line (Ctrl+G / ⌘G)" : undefined} onClick={openGoTo}>
            Ln {cursorStatus.line}, Col {cursorStatus.column}{cursorStatus.cursors > 1 ? ` · ${cursorStatus.cursors} cursors` : cursorStatus.selected ? ` · ${cursorStatus.selected} selected` : ""}
          </button>
          {!isBlockPreview && activeField && onNavigateToField && <button type="button" className="h-7 px-1.5 hover:bg-muted focus-visible:outline focus-visible:outline-primary text-primary whitespace-nowrap"
            aria-label="Open field" title={tooltipsEnabled ? `${activeField.label}. Allowed: ${activeField.allowedValues}` : undefined}
            onClick={() => navigateToField(activeField, true)}>Open field</button>}
          {!isBlockPreview && !activeField && activeSegment && onNavigateToSegment && <button type="button" className="h-7 px-1.5 hover:bg-muted focus-visible:outline focus-visible:outline-primary text-primary whitespace-nowrap"
            aria-label="Open block" title={tooltipsEnabled ? explanations[activeSegment.id]?.summary || activeSegment.name : undefined}
            onClick={() => navigateToSegment(activeSegment.id, true)}>Open block</button>}
          </div>
          <div className="flex items-center gap-1">
            <span className="px-1">{isEditable ? "Lua" : "Lua · read only"}</span>
            <span className="px-1">Spaces: 2</span>
            <button type="button" aria-label="Toggle word wrap" aria-pressed={wordWrap} onClick={() => setWordWrap((value) => !value)}
              className={`h-7 px-1.5 hover:bg-muted focus-visible:outline focus-visible:outline-primary ${wordWrap ? "text-foreground" : ""}`}>Wrap: {wordWrap ? "on" : "off"}</button>
            <button type="button" aria-label="Decrease editor font size" disabled={fontSize <= MIN_FONT_SIZE} onClick={() => setFontSize((size) => Math.max(MIN_FONT_SIZE, size - 1))}
              className="h-7 w-6 hover:bg-muted disabled:opacity-40 focus-visible:outline focus-visible:outline-primary">−</button>
            <button type="button" aria-label="Reset editor font size" onClick={() => setFontSize(DEFAULT_FONT_SIZE)}
              className="h-7 px-1 tabular-nums hover:bg-muted focus-visible:outline focus-visible:outline-primary">{fontSize}px</button>
            <button type="button" aria-label="Increase editor font size" disabled={fontSize >= MAX_FONT_SIZE} onClick={() => setFontSize((size) => Math.min(MAX_FONT_SIZE, size + 1))}
              className="h-7 w-6 hover:bg-muted disabled:opacity-40 focus-visible:outline focus-visible:outline-primary">+</button>
          </div>
        </div>
      </div>
    </aside>
  );
};

export default LiveCodePanel;
