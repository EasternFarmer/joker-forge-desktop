import { countColumn, findColumn, type EditorState, type Text } from "@codemirror/state";

export interface LiveCodeCursorStatus {
  line: number;
  column: number;
  cursors: number;
  selected: number;
}

export function getLiveCodeCursorStatus(state: EditorState): LiveCodeCursorStatus {
  const line = state.doc.lineAt(state.selection.main.head);
  return {
    line: line.number,
    column: countColumn(line.text.slice(0, state.selection.main.head - line.from), state.tabSize) + 1,
    cursors: state.selection.ranges.length,
    selected: state.selection.ranges.reduce((total, range) => total + range.to - range.from, 0),
  };
}

export function getLiveCodeGoToPosition(doc: Text, input: string, tabSize = 2): number | null {
  const match = /^\s*(\d+)(?::(\d+))?\s*$/.exec(input);
  if (!match) return null;
  const lineNumber = Number(match[1]);
  const column = match[2] === undefined ? 1 : Number(match[2]);
  if (!Number.isSafeInteger(lineNumber) || !Number.isSafeInteger(column) || lineNumber < 1 || column < 1) return null;
  const line = doc.line(Math.min(doc.lines, lineNumber));
  return line.from + findColumn(line.text, column - 1, tabSize);
}
