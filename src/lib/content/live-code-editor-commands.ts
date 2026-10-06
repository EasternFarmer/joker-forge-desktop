import { EditorSelection, countColumn, type StateCommand } from "@codemirror/state";
import { type Command, type KeyBinding } from "@codemirror/view";
import {
  addCursorAbove,
  addCursorBelow,
  copyLineDown,
  copyLineUp,
  deleteLine,
  indentLess,
  indentMore,
  insertBlankLine,
  moveLineDown,
  moveLineUp,
  toggleLineComment,
} from "@codemirror/commands";
import { acceptCompletion, completionStatus, hasNextSnippetField, hasPrevSnippetField, nextSnippetField, prevSnippetField } from "@codemirror/autocomplete";
import { SearchCursor, selectNextOccurrence } from "@codemirror/search";
import { IndentContext, getIndentation, indentString, indentUnit } from "@codemirror/language";

export const smartTab: Command = (view) => {
  const { state } = view;
  if (state.readOnly) return false;
  if (completionStatus(state) === "active") {
    acceptCompletion(view);
    return true;
  }
  if (hasNextSnippetField(state)) return nextSnippetField(view);
  if (state.selection.ranges.some((range) => !range.empty)) return indentMore(view);

  const unit = state.facet(indentUnit);
  const indentedLines = new Set<number>();
  const changes = [];
  for (const range of state.selection.ranges) {
    const line = state.doc.lineAt(range.head);
    const prefix = line.text.slice(0, range.head - line.from);
    if (/^[\t ]*$/.test(prefix)) {
      if (!indentedLines.has(line.from)) {
        indentedLines.add(line.from);
        changes.push({ from: line.from, insert: unit });
      }
    } else {
      const column = countColumn(prefix, state.tabSize);
      changes.push({ from: range.head, insert: " ".repeat(state.tabSize - column % state.tabSize) });
    }
  }
  const changeSet = state.changes(changes);
  view.dispatch(state.update({
    changes: changeSet,
    selection: state.selection.map(changeSet, 1),
    scrollIntoView: true,
    userEvent: "input.indent",
  }));
  return true;
};

export const smartShiftTab: Command = (view) => {
  if (view.state.readOnly) return false;
  return hasPrevSnippetField(view.state) ? prevSnippetField(view) : indentLess(view);
};

export const insertLineAbove: StateCommand = ({ state, dispatch }) => {
  if (state.readOnly) return false;
  const lines = new Map<number, string>();
  for (const range of state.selection.ranges) {
    const line = state.doc.lineAt(range.from);
    if (!lines.has(line.from)) {
      const context = new IndentContext(state, { simulateBreak: line.from });
      const columns = getIndentation(context, line.from);
      lines.set(line.from, columns === null
        ? /^[\t ]*/.exec(line.text)![0]
        : indentString(state, Math.max(0, columns)));
    }
  }
  const changes = state.changes(Array.from(lines, ([from, indentation]) => ({ from, insert: indentation + state.lineBreak })));
  const selection = EditorSelection.create(state.selection.ranges.map((range) => {
    const from = state.doc.lineAt(range.from).from;
    return EditorSelection.cursor(changes.mapPos(from, -1) + lines.get(from)!.length);
  }), state.selection.mainIndex);
  dispatch(state.update({ changes, selection, scrollIntoView: true, userEvent: "input" }));
  return true;
};

const editable = (command: Command): Command => (view) => !view.state.readOnly && command(view);

export const selectAllOccurrences: StateCommand = ({ state, dispatch }) => {
  const main = state.selection.main;
  const selected = main.empty ? state.wordAt(main.head) : main;
  if (!selected || selected.empty) return false;
  const ranges = [];
  let mainIndex = 0;
  const matches = new SearchCursor(state.doc, state.sliceDoc(selected.from, selected.to));
  while (!matches.next().done) {
    if (ranges.length >= 1000) return false;
    if (matches.value.from === selected.from) mainIndex = ranges.length;
    ranges.push(EditorSelection.range(matches.value.from, matches.value.to));
  }
  dispatch(state.update({ selection: EditorSelection.create(ranges, mainIndex), userEvent: "select.search.matches" }));
  return true;
};

export const liveCodeEditorKeymap: readonly KeyBinding[] = [
  { key: "Tab", run: smartTab },
  { key: "Shift-Tab", run: smartShiftTab },
  { key: "Alt-ArrowUp", run: editable(moveLineUp), preventDefault: true },
  { key: "Alt-ArrowDown", run: editable(moveLineDown), preventDefault: true },
  { key: "Shift-Alt-ArrowUp", run: editable(copyLineUp), preventDefault: true },
  { key: "Shift-Alt-ArrowDown", run: editable(copyLineDown), preventDefault: true },
  { key: "Mod-Shift-k", run: editable(deleteLine), preventDefault: true },
  { key: "Mod-/", run: editable(toggleLineComment), preventDefault: true },
  { key: "Mod-d", run: selectNextOccurrence, preventDefault: true },
  { key: "Mod-Shift-l", run: selectAllOccurrences, preventDefault: true },
  { key: "Mod-Alt-ArrowUp", run: addCursorAbove, preventDefault: true },
  { key: "Mod-Alt-ArrowDown", run: addCursorBelow, preventDefault: true },
  { key: "Mod-Enter", run: editable(insertBlankLine), preventDefault: true },
  { key: "Mod-Shift-Enter", run: editable(insertLineAbove), preventDefault: true },
];
