import type { LiveCodeOutlineEntry } from "./live-code-structure";

export interface LiveCodeOutlineNode extends LiveCodeOutlineEntry {
  children: LiveCodeOutlineNode[];
  parentId?: string;
}

const emptyCollapsedIds: ReadonlySet<string> = new Set();

const contains = (parent: LiveCodeOutlineEntry, child: LiveCodeOutlineEntry): boolean =>
  parent.from <= child.from && parent.to >= child.to && (parent.from < child.from || parent.to > child.to);

export function buildLiveCodeOutlineTree(
  entries: readonly LiveCodeOutlineEntry[],
  documentLength = Infinity,
): LiveCodeOutlineNode[] {
  const seenIds = new Set<string>();
  const sorted = entries.filter((entry) => {
    if (!entry.id || seenIds.has(entry.id) || !Number.isInteger(entry.from) || !Number.isInteger(entry.to)
      || entry.from < 0 || entry.to <= entry.from || entry.to > documentLength) return false;
    seenIds.add(entry.id);
    return true;
  }).sort((a, b) => a.from - b.from || b.to - a.to);
  const roots: LiveCodeOutlineNode[] = [];
  const ancestors: LiveCodeOutlineNode[] = [];
  for (const entry of sorted) {
    while (ancestors.length && !contains(ancestors[ancestors.length - 1], entry)) ancestors.pop();
    const parent = ancestors[ancestors.length - 1];
    const node: LiveCodeOutlineNode = { ...entry, depth: parent ? parent.depth + 1 : 0, children: [] };
    if (parent) {
      node.parentId = parent.id;
      parent.children.push(node);
    } else {
      roots.push(node);
    }
    ancestors.push(node);
  }
  return roots;
}

export function filterLiveCodeOutlineTree(
  tree: readonly LiveCodeOutlineNode[],
  query: string,
): readonly LiveCodeOutlineNode[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return tree;
  const filter = (nodes: readonly LiveCodeOutlineNode[]): LiveCodeOutlineNode[] => nodes.flatMap((node) => {
    const text = `${node.label} ${node.kind}`.toLocaleLowerCase();
    if (terms.every((term) => text.includes(term))) return [node];
    const children = filter(node.children);
    return children.length ? [{ ...node, children }] : [];
  });
  return filter(tree);
}

export function flattenLiveCodeOutlineTree(
  tree: readonly LiveCodeOutlineNode[],
  collapsedIds: ReadonlySet<string> = emptyCollapsedIds,
): LiveCodeOutlineNode[] {
  const visible: LiveCodeOutlineNode[] = [];
  const pending = [...tree].reverse();
  while (pending.length) {
    const node = pending.pop()!;
    visible.push(node);
    if (!collapsedIds.has(node.id)) {
      for (let index = node.children.length - 1; index >= 0; index -= 1) pending.push(node.children[index]);
    }
  }
  return visible;
}

export function getActiveLiveCodeOutlineEntry(
  tree: readonly LiveCodeOutlineNode[],
  position: number,
  collapsedIds: ReadonlySet<string> = emptyCollapsedIds,
): LiveCodeOutlineNode | undefined {
  if (!Number.isInteger(position) || position < 0) return undefined;
  let active: LiveCodeOutlineNode | undefined;
  const pending = [...tree].reverse();
  while (pending.length) {
    const node = pending.pop()!;
    if (position < node.from || position >= node.to) continue;
    if (!active || node.to - node.from < active.to - active.from
      || (node.to - node.from === active.to - active.from && (node.depth > active.depth
        || (node.depth === active.depth && node.kind === "rule" && active.kind !== "rule")))) active = node;
    if (!collapsedIds.has(node.id)) {
      for (let index = node.children.length - 1; index >= 0; index -= 1) pending.push(node.children[index]);
    }
  }
  return active;
}
