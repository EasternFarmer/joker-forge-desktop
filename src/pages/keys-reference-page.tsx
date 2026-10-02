import { useMemo, useRef, useState } from "react";
import { Check, Copy, Key, MagnifyingGlass, X } from "@phosphor-icons/react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  getKeyReferenceEntries,
  KEY_REFERENCE_CATEGORIES,
} from "@/lib/balatro/keys-reference";
import { normalizeSearchText } from "@/lib/core/search";
import { useProjectData } from "@/lib/services/storage";

const PAGE_SIZE = 50;

export default function KeysReferencePage() {
  const { data, isHydrating } = useProjectData();
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("all");
  const [source, setSource] = useState("all");
  const [page, setPage] = useState(0);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const tableRef = useRef<HTMLDivElement>(null);

  const entries = useMemo(() => getKeyReferenceEntries(data), [data]);
  const filteredEntries = useMemo(() => {
    const terms = search.trim().split(/\s+/).map(normalizeSearchText);
    return entries.filter((entry) => {
      if (category !== "all" && entry.category !== category) return false;
      if (source !== "all" && entry.source !== source) return false;
      const text = normalizeSearchText(
        `${entry.name} ${entry.key} ${entry.category}`,
      );
      return terms.every((term) => text.includes(term));
    });
  }, [entries, search, category, source]);

  const pageCount = Math.max(1, Math.ceil(filteredEntries.length / PAGE_SIZE));
  const currentPage = Math.min(page, pageCount - 1);
  const start = currentPage * PAGE_SIZE;
  const visibleEntries = filteredEntries.slice(start, start + PAGE_SIZE);
  const hasFilters = Boolean(search || category !== "all" || source !== "all");

  const clearFilters = () => {
    setSearch("");
    setCategory("all");
    setSource("all");
    setPage(0);
  };

  const changePage = (nextPage: number) => {
    setPage(nextPage);
    tableRef.current?.scrollIntoView({ block: "start" });
  };

  const copyKey = async (key: string) => {
    try {
      await navigator.clipboard.writeText(key);
      setCopiedKey(key);
      toast.success(`Copied ${key}`);
    } catch {
      toast.error("Could not copy the key. Select the key and copy it manually.");
    }
  };

  return (
    <div className="mx-auto max-w-6xl space-y-6 pb-8">
      <div className="flex items-start gap-4">
        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-yellow-400/10 text-yellow-500">
          <Key className="h-7 w-7" weight="duotone" />
        </div>
        <div className="space-y-2">
          <h1 className="font-game text-3xl tracking-tight">Keys Reference</h1>
          <p className="text-muted-foreground">
            Find and copy keys for cards, sounds and other game objects to use in
            your rules. Your mod's entries show the full keys used in the game.
          </p>
        </div>
      </div>

      <div className="space-y-4 rounded-xl border border-border bg-card p-4 sm:p-6">
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-[1fr_220px_180px]">
          <div className="space-y-2 sm:col-span-2 lg:col-span-1">
            <label htmlFor="key-search" className="text-sm font-medium">
              Search keys
            </label>
            <div className="relative">
              <MagnifyingGlass className="pointer-events-none absolute left-3 top-3 h-4 w-4 text-muted-foreground" />
              <Input
                id="key-search"
                type="search"
                placeholder="Search by name or key…"
                value={search}
                onChange={(event) => {
                  setSearch(event.target.value);
                  setPage(0);
                }}
                className="h-10 pl-9 pr-10"
              />
              {search && (
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Clear search"
                  onClick={() => {
                    setSearch("");
                    setPage(0);
                  }}
                  className="absolute right-1 top-1"
                >
                  <X />
                </Button>
              )}
            </div>
          </div>
          <div className="space-y-2">
            <label htmlFor="key-category" className="text-sm font-medium">
              Category
            </label>
            <Select
              value={category}
              onValueChange={(value) => {
                setCategory(value);
                setPage(0);
              }}
            >
              <SelectTrigger id="key-category" className="h-10 w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All categories</SelectItem>
                {KEY_REFERENCE_CATEGORIES.map((value) => (
                  <SelectItem key={value} value={value}>
                    {value}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-2">
            <label htmlFor="key-source" className="text-sm font-medium">
              Source
            </label>
            <Select
              value={source}
              onValueChange={(value) => {
                setSource(value);
                setPage(0);
              }}
            >
              <SelectTrigger id="key-source" className="h-10 w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All keys</SelectItem>
                <SelectItem value="vanilla">Vanilla</SelectItem>
                <SelectItem value="mod">This mod</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <div className="flex min-h-8 items-center justify-between gap-3">
          <p className="text-sm text-muted-foreground" role="status">
            {filteredEntries.length}{" "}
            {filteredEntries.length === 1 ? "key" : "keys"}
            {hasFilters ? ` found · ${entries.length} total` : " available"}
          </p>
          {hasFilters && (
            <Button variant="ghost" size="sm" onClick={clearFilters}>
              <X /> Clear filters
            </Button>
          )}
        </div>
      </div>

      <div ref={tableRef} className="overflow-hidden rounded-xl border border-border bg-card">
        <Table>
          <TableCaption className="sr-only">
            Game object names and keys, with buttons to copy each key.
          </TableCaption>
          <TableHeader className="bg-muted/40">
            <TableRow>
              <TableHead scope="col" className="px-4">
                Name
              </TableHead>
              <TableHead scope="col">Category</TableHead>
              <TableHead scope="col">Source</TableHead>
              <TableHead scope="col" className="px-4">
                Key
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {visibleEntries.map((entry) => (
              <TableRow key={entry.id}>
                <TableCell className="px-4 font-medium">
                  {entry.name}
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {entry.category}
                </TableCell>
                <TableCell>
                  <span
                    className={`rounded-md px-2 py-1 text-xs ${entry.source === "mod" ? "bg-primary/10 text-primary" : "bg-muted text-muted-foreground"}`}
                  >
                    {entry.source === "mod" ? "This mod" : "Vanilla"}
                  </span>
                </TableCell>
                <TableCell className="px-4">
                  <div className="flex items-center justify-between gap-4">
                    <code className="select-all font-mono text-sm">{entry.key}</code>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Copy ${entry.key}`}
                      title={`Copy ${entry.key}`}
                      onClick={() => void copyKey(entry.key)}
                    >
                      {copiedKey === entry.key ? (
                        <Check className="text-primary" />
                      ) : (
                        <Copy />
                      )}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
            {visibleEntries.length === 0 && (
              <TableRow>
                <TableCell
                  colSpan={4}
                  className="px-4 py-12 text-center whitespace-normal"
                >
                  <p className="font-medium">
                    {isHydrating && source === "mod"
                      ? "Loading your mod's keys…"
                      : "No keys found"}
                  </p>
                  <p className="mt-2 text-muted-foreground">
                    {source === "mod" && !search
                      ? "Keys appear here as you add items to your mod."
                      : "Try a different search or clear your filters."}
                  </p>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
        {filteredEntries.length > PAGE_SIZE && (
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-4 py-3">
            <p className="text-sm text-muted-foreground">
              Showing {start + 1}–
              {Math.min(start + PAGE_SIZE, filteredEntries.length)} of{" "}
              {filteredEntries.length}
            </p>
            <div className="flex items-center gap-3">
              <Button
                variant="outline"
                size="sm"
                disabled={currentPage === 0}
                onClick={() => changePage(currentPage - 1)}
              >
                Previous
              </Button>
              <span className="text-sm text-muted-foreground">
                Page {currentPage + 1} of {pageCount}
              </span>
              <Button
                variant="outline"
                size="sm"
                disabled={currentPage === pageCount - 1}
                onClick={() => changePage(currentPage + 1)}
              >
                Next
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
