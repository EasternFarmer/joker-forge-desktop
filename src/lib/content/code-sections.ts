export interface CodeSegment {
  id: string;
  segmentType: string;
  name: string;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
}

const STRUCTURAL_SEGMENTS = new Set(["config", "loc_txt", "props", "loc_vars"]);

const lineOffsets = (code: string): number[] => {
  const offsets: number[] = [0];
  for (let i = 0; i < code.length; i += 1) {
    if (code[i] === "\n") offsets.push(i + 1);
  }
  return offsets;
};

const lineColToIndex = (
  offsets: number[],
  line: number,
  column: number,
  codeLength: number,
): number => {
  const lineStart = offsets[Math.max(0, line - 1)] ?? codeLength;
  return Math.min(codeLength, Math.max(0, lineStart + Math.max(0, column - 1)));
};

const indexToLineCol = (
  offsets: number[],
  index: number,
): { line: number; column: number } => {
  let low = 0;
  let high = offsets.length - 1;
  while (low <= high) {
    const mid = Math.floor((low + high) / 2);
    const start = offsets[mid];
    const next = offsets[mid + 1] ?? Number.POSITIVE_INFINITY;
    if (index < start) {
      high = mid - 1;
      continue;
    }
    if (index >= next) {
      low = mid + 1;
      continue;
    }
    return { line: mid + 1, column: index - start + 1 };
  }
  const last = Math.max(0, offsets.length - 1);
  return { line: last + 1, column: 1 };
};

interface SegmentRange {
  id: string;
  index: number;
  start: number;
  end: number;
}

const toRanges = (code: string, segments: CodeSegment[]): SegmentRange[] => {
  const offsets = lineOffsets(code);
  return segments
    .map((segment, index) => ({
      id: segment.id,
      index,
      start: lineColToIndex(offsets, segment.startLine, segment.startColumn, code.length),
      end: lineColToIndex(offsets, segment.endLine, segment.endColumn, code.length),
    }))
    .sort((a, b) => a.start - b.start);
};

const ruleIdFromSegment = (segmentId: string): string | null => {
  if (!segmentId.startsWith("rule:")) return null;
  return segmentId.slice(5);
};

const isMergeManagedSegment = (segmentId: string): boolean =>
  STRUCTURAL_SEGMENTS.has(segmentId) || segmentId.startsWith("rule:");

export function mergeWithGeneratedSegments(
  userCode: string,
  oldGeneratedCode: string,
  oldSegments: CodeSegment[],
  newGeneratedCode: string,
  newSegments: CodeSegment[],
  changedRuleIds: Set<string>,
): string {
  const oldRanges = toRanges(
    oldGeneratedCode,
    oldSegments.filter((segment) => isMergeManagedSegment(segment.id)),
  );
  const newRanges = toRanges(
    newGeneratedCode,
    newSegments.filter((segment) => isMergeManagedSegment(segment.id)),
  );
  const newById = new Map(newRanges.map((range) => [range.id, range]));

  const oldSnippets = new Map(
    oldRanges.map((range) => [range.id, oldGeneratedCode.slice(range.start, range.end)]),
  );

  const replacements: Array<{ start: number; end: number; text: string }> = [];
  let searchFrom = 0;

  for (const oldRange of oldRanges) {
    const oldSnippet = oldSnippets.get(oldRange.id);
    if (!oldSnippet) continue;

    const userIndex = userCode.indexOf(oldSnippet, searchFrom);
    if (userIndex < 0) continue;

    const userStart = userIndex;
    const userEnd = userIndex + oldSnippet.length;
    searchFrom = userEnd;

    const newRange = newById.get(oldRange.id);
    const isStructural = STRUCTURAL_SEGMENTS.has(oldRange.id);
    const ruleId = ruleIdFromSegment(oldRange.id);
    const ruleChanged = !!ruleId && changedRuleIds.has(ruleId);

    if (!newRange) {
      if (isStructural || ruleChanged) {
        replacements.push({ start: userStart, end: userEnd, text: "" });
      }
      continue;
    }

    const newSnippet = newGeneratedCode.slice(newRange.start, newRange.end);
    if (isStructural || ruleChanged) {
      replacements.push({ start: userStart, end: userEnd, text: newSnippet });
    }
  }

  replacements.sort((a, b) => b.start - a.start);

  let merged = userCode;
  for (const replacement of replacements) {
    merged =
      merged.slice(0, replacement.start) +
      replacement.text +
      merged.slice(replacement.end);
  }

  const oldIds = new Set(oldRanges.map((range) => range.id));
  const appended = newRanges
    .filter((range) => !oldIds.has(range.id))
    .map((range) => newGeneratedCode.slice(range.start, range.end))
    .filter((text) => text.trim().length > 0);

  if (appended.length > 0) {
    merged = `${merged.trimEnd()}\n${appended.join("\n")}\n`;
  }

  return merged;
}

export function remapSegmentsToCode(
  displayCode: string,
  generatedCode: string,
  generatedSegments: CodeSegment[],
): CodeSegment[] {
  if (displayCode === generatedCode) return generatedSegments;
  if (!displayCode || !generatedCode || generatedSegments.length === 0) return [];

  const generatedRanges = toRanges(generatedCode, generatedSegments);
  const displayOffsets = lineOffsets(displayCode);
  const remappedByIndex = new Map<number, CodeSegment>();
  const occurrencesBySnippet = new Map<string, { before: number[]; after: number[] }>();
  const mappedSourceRegions = new Map<string, number>();
  const occurrences = (code: string, snippet: string) => {
    const positions: number[] = [];
    let position = code.indexOf(snippet);
    while (position >= 0) {
      positions.push(position);
      position = code.indexOf(snippet, position + snippet.length);
    }
    return positions;
  };

  for (const range of generatedRanges) {
    const segment = generatedSegments[range.index];

    const snippet = generatedCode.slice(range.start, range.end);
    if (!snippet) continue;
    let matches = occurrencesBySnippet.get(snippet);
    if (!matches) {
      matches = { before: occurrences(generatedCode, snippet), after: occurrences(displayCode, snippet) };
      occurrencesBySnippet.set(snippet, matches);
    }
    if (matches.before.length !== matches.after.length) continue;
    const occurrence = matches.before.indexOf(range.start);
    if (occurrence < 0) continue;

    const sourceRegion = `${range.start}:${range.end}`;
    const matchIndex = mappedSourceRegions.get(sourceRegion)
      ?? matches.after[occurrence];
    if (matchIndex < 0) continue;

    const start = indexToLineCol(displayOffsets, matchIndex);
    const end = indexToLineCol(
      displayOffsets,
      matchIndex + snippet.length,
    );
    remappedByIndex.set(range.index, {
      ...segment,
      startLine: start.line,
      startColumn: start.column,
      endLine: end.line,
      endColumn: end.column,
    });
    mappedSourceRegions.set(sourceRegion, matchIndex);
  }

  return generatedSegments.flatMap((_segment, index) => {
    const mapped = remappedByIndex.get(index);
    return mapped ? [mapped] : [];
  });
}
