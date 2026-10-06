import { ChangeSet, Text } from "@codemirror/state";
import type { CodeSegment } from "./code-sections";
import type { CodeEdit } from "./live-code-sync";

export interface CodeSegmentRange {
  segment: CodeSegment;
  from: number;
  to: number;
}

export function getCodeSegmentRanges(code: string, segments: CodeSegment[]): CodeSegmentRange[] {
  const doc = Text.of(code.split("\n"));
  const offset = (line: number, column: number): number | null => {
    if (!Number.isInteger(line) || !Number.isInteger(column) || line < 1 || line > doc.lines || column < 1) return null;
    const row = doc.line(line);
    return column <= row.length + 1 ? row.from + column - 1 : null;
  };
  return segments.flatMap((segment) => {
    const from = offset(segment.startLine, segment.startColumn);
    const to = offset(segment.endLine, segment.endColumn);
    return from !== null && to !== null && to > from ? [{ segment, from, to }] : [];
  });
}

export function findCodeSegmentAt(code: string, segments: CodeSegment[], position: number): CodeSegment | undefined {
  return getCodeSegmentRanges(code, segments)
    .filter(({ segment, from, to }) => /^(trigger|condition|effect|rule):/.test(segment.id) && from <= position && position < to)
    .sort((a, b) => (a.to - a.from) - (b.to - b.from))[0]?.segment;
}

export function mapCodeSegmentsThroughEdits(
  oldCode: string,
  newCode: string,
  segments: CodeSegment[],
  edits?: CodeEdit[],
): CodeSegment[] {
  if (!edits || oldCode === newCode) return segments;
  const changes = ChangeSet.of(edits, oldCode.length);
  const doc = Text.of(newCode.split("\n"));
  return getCodeSegmentRanges(oldCode, segments).flatMap(({ segment, from, to }) => {
    if (edits.some((edit) => edit.from <= from && edit.to >= to && edit.to > edit.from)) return [];
    const start = changes.mapPos(from, 1);
    const end = changes.mapPos(to, -1);
    if (end <= start) return [];
    const startLine = doc.lineAt(start);
    const endLine = doc.lineAt(end);
    return [{ ...segment, startLine: startLine.number, startColumn: start - startLine.from + 1,
      endLine: endLine.number, endColumn: end - endLine.from + 1 }];
  });
}
