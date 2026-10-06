import { EditorState, StateEffect, type Text } from "@codemirror/state";
import { EditorView, runScopeHandlers, type Panel, type ViewUpdate } from "@codemirror/view";
import { SearchQuery, search, getSearchQuery, setSearchQuery, openSearchPanel, closeSearchPanel, searchPanelOpen,
  findNext, findPrevious, replaceNext, replaceAll } from "@codemirror/search";

const showReplaceEffect = StateEffect.define<boolean>();
let nextSearchPanelId = 0;

export interface LiveCodeSearchMatches {
  total: number;
  current: number;
  limited: boolean;
}

export function getLiveCodeSearchMatches(state: EditorState, query: SearchQuery, limit = 10000): Array<{ from: number; to: number }> {
  if (!query.valid) return [];
  const matches = [];
  const cursor = query.getCursor(state);
  for (let next = cursor.next(); !next.done && matches.length <= limit; next = cursor.next()) matches.push(next.value);
  return matches;
}

export function getLiveCodeSearchStatus(matches: readonly { from: number; to: number }[], state: EditorState, limit = 10000): LiveCodeSearchMatches {
  const selection = state.selection.main;
  let low = 0, high = matches.length - 1;
  let current = 0;
  while (low <= high) {
    const middle = (low + high) >>> 1;
    if (matches[middle].from < selection.from) low = middle + 1;
    else if (matches[middle].from > selection.from) high = middle - 1;
    else { if (matches[middle].to === selection.to && middle < limit) current = middle + 1; break; }
  }
  return { total: Math.min(matches.length, limit), current, limited: matches.length > limit };
}

class LiveCodeSearchPanel implements Panel {
  dom: HTMLDivElement;
  top = true;
  private query: SearchQuery;
  private doc: Text;
  private matches: Array<{ from: number; to: number }> = [];
  private searchField: HTMLInputElement;
  private replaceField: HTMLInputElement;
  private replaceRow: HTMLDivElement;
  private replacementToggle: HTMLButtonElement;
  private status: HTMLSpanElement;
  private caseToggle: HTMLButtonElement;
  private wordToggle: HTMLButtonElement;
  private regexToggle: HTMLButtonElement;
  private actions: HTMLButtonElement[] = [];
  private countTimer: ReturnType<typeof setTimeout> | undefined;
  private pending = false;

  constructor(private view: EditorView) {
    const document = view.dom.ownerDocument;
    this.query = getSearchQuery(view.state);
    this.doc = view.state.doc;
    this.dom = document.createElement("div");
    this.dom.className = "cm-jf-search";
    this.dom.setAttribute("role", "search");
    this.dom.setAttribute("aria-label", "Find and replace");
    const row = document.createElement("div");
    row.className = "jf-search-row";
    this.dom.append(row);
    const button = (label: string, text: string, action: () => void) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "jf-search-button";
      button.setAttribute("aria-label", label);
      button.textContent = text;
      button.addEventListener("click", action);
      return button;
    };
    const input = (name: string, label: string) => {
      const field = document.createElement("input");
      field.type = "text";
      field.name = name;
      field.placeholder = label;
      field.setAttribute("aria-label", label);
      field.autocomplete = "off";
      field.spellcheck = false;
      field.addEventListener("input", () => this.commit());
      return field;
    };
    this.replacementToggle = button("Show replacement", "›", () => this.setReplaceVisible(this.replaceRow.hidden));
    this.replacementToggle.classList.add("jf-search-disclosure");
    this.replacementToggle.disabled = view.state.readOnly;
    this.replacementToggle.setAttribute("aria-expanded", "false");
    this.searchField = input("search", "Find");
    this.searchField.setAttribute("main-field", "true");
    const fieldGroup = document.createElement("div");
    fieldGroup.className = "jf-search-input";
    this.caseToggle = button("Match case", "Aa", () => this.toggle(this.caseToggle));
    this.wordToggle = button("Match whole word", "ab", () => this.toggle(this.wordToggle));
    this.wordToggle.classList.add("jf-search-word");
    this.regexToggle = button("Use regular expression", ".*", () => this.toggle(this.regexToggle));
    fieldGroup.append(this.searchField, this.caseToggle, this.wordToggle, this.regexToggle);
    this.status = document.createElement("span");
    this.status.className = "jf-search-count";
    this.status.id = `jf-live-code-search-status-${++nextSearchPanelId}`;
    this.status.setAttribute("role", "status");
    this.status.setAttribute("aria-live", "polite");
    this.status.setAttribute("aria-atomic", "true");
    this.searchField.setAttribute("aria-describedby", this.status.id);
    const previous = button("Previous match", "↑", () => findPrevious(view));
    const next = button("Next match", "↓", () => findNext(view));
    const close = button("Close find and replace", "×", () => { closeSearchPanel(view); view.focus(); });
    this.actions.push(previous, next);
    row.append(this.replacementToggle, fieldGroup, this.status, previous, next, close);
    this.replaceRow = document.createElement("div");
    this.replaceRow.className = "jf-search-row jf-replace-row";
    this.replaceRow.hidden = true;
    this.replaceField = input("replace", "Replace");
    const replace = button("Replace current match", "Replace", () => replaceNext(view));
    const all = button("Replace all matches", "All", () => replaceAll(view));
    replace.classList.add("jf-search-text-button");
    all.classList.add("jf-search-text-button");
    this.actions.push(replace, all);
    this.replaceRow.append(this.replaceField, replace, all);
    this.dom.append(this.replaceRow);
    this.dom.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && event.target === this.searchField) {
        event.preventDefault(); (event.shiftKey ? findPrevious : findNext)(view);
      } else if (event.key === "Enter" && event.target === this.replaceField) {
        event.preventDefault(); replaceNext(view);
      } else if (runScopeHandlers(view, event, "search-panel")) event.preventDefault();
    });
    this.setQuery(this.query);
    this.refreshMatches();
  }

  private toggle(button: HTMLButtonElement) {
    button.setAttribute("aria-pressed", String(button.getAttribute("aria-pressed") !== "true"));
    this.commit();
  }

  private commit() {
    const query = new SearchQuery({ search: this.searchField.value, replace: this.replaceField.value, literal: true,
      caseSensitive: this.caseToggle.getAttribute("aria-pressed") === "true",
      wholeWord: this.wordToggle.getAttribute("aria-pressed") === "true",
      regexp: this.regexToggle.getAttribute("aria-pressed") === "true" });
    if (!query.eq(this.query)) this.view.dispatch({ effects: setSearchQuery.of(query) });
  }

  private setQuery(query: SearchQuery) {
    this.query = query;
    if (this.searchField.value !== query.search) this.searchField.value = query.search;
    if (this.replaceField.value !== query.replace) this.replaceField.value = query.replace;
    this.caseToggle.setAttribute("aria-pressed", String(query.caseSensitive));
    this.wordToggle.setAttribute("aria-pressed", String(query.wholeWord));
    this.regexToggle.setAttribute("aria-pressed", String(query.regexp));
  }

  private refreshMatches() {
    this.pending = false;
    this.doc = this.view.state.doc;
    this.matches = getLiveCodeSearchMatches(this.view.state, this.query);
    this.refreshStatus();
  }

  private refreshStatus() {
    const status = getLiveCodeSearchStatus(this.matches, this.view.state);
    const invalid = !!this.query.search && !this.query.valid;
    this.status.textContent = invalid ? "Invalid regex" : !this.query.search ? "" : this.pending ? "…" : status.total === 0 ? "No results"
      : `${status.current ? `${status.current} of ` : ""}${status.total}${status.limited ? "+" : ""}`;
    this.searchField.setAttribute("aria-invalid", String(invalid));
    this.status.classList.toggle("jf-search-empty", invalid || !this.pending && !!this.query.search && status.total === 0);
    this.actions.forEach((button, index) => { button.disabled = !this.query.valid || !this.pending && status.total === 0 || index > 1 && this.view.state.readOnly; });
  }

  setReplaceVisible(visible: boolean) {
    visible = visible && !this.view.state.readOnly;
    this.replaceRow.hidden = !visible;
    this.replacementToggle.textContent = visible ? "⌄" : "›";
    this.replacementToggle.setAttribute("aria-expanded", String(visible));
    this.replacementToggle.setAttribute("aria-label", visible ? "Hide replacement" : "Show replacement");
    this.view.requestMeasure();
  }

  mount() {
    this.searchField.focus({ preventScroll: true });
    this.searchField.select();
  }

  update(update: ViewUpdate) {
    const query = getSearchQuery(update.state);
    const queryChanged = !query.eq(this.query);
    if (queryChanged) this.setQuery(query);
    if (this.doc !== update.state.doc || queryChanged) {
      this.pending = true;
      this.refreshStatus();
      if (this.countTimer !== undefined) clearTimeout(this.countTimer);
      this.countTimer = setTimeout(() => { this.countTimer = undefined; this.refreshMatches(); }, 120);
    }
    else if (update.selectionSet) this.refreshStatus();
    for (const transaction of update.transactions) for (const effect of transaction.effects) {
      if (effect.is(showReplaceEffect)) this.setReplaceVisible(effect.value);
    }
  }

  destroy() { if (this.countTimer !== undefined) clearTimeout(this.countTimer); }
}

const createLiveCodeSearchPanel = (view: EditorView) => new LiveCodeSearchPanel(view);

export function openLiveCodeSearch(view: EditorView, replace = false): boolean {
  const scroll = view.scrollSnapshot();
  if (!searchPanelOpen(view.state)) {
    const replacement = getSearchQuery(view.state).replace;
    openSearchPanel(view);
    const query = getSearchQuery(view.state);
    if (query.replace !== replacement) view.dispatch({ effects: setSearchQuery.of(new SearchQuery({ ...query, replace: replacement })) });
  }
  if (replace && !view.state.readOnly) view.dispatch({ effects: showReplaceEffect.of(true) });
  const input = view.dom.querySelector<HTMLInputElement>(replace && !view.state.readOnly ? '.cm-jf-search input[name="replace"]' : '.cm-jf-search input[name="search"]');
  input?.focus({ preventScroll: true }); input?.select();
  view.dispatch({ effects: scroll });
  return true;
}

export function liveCodeSearch() {
  return [search({ top: true, literal: true, createPanel: createLiveCodeSearchPanel,
    scrollToMatch: (range) => EditorView.scrollIntoView(range, { y: "center" }) }), EditorView.theme({
    ".cm-panels-top:has(.cm-jf-search)": { position: "absolute", top: "8px", right: "10px", left: "auto", width: "min(530px, calc(100% - 20px))", zIndex: "20",
      border: "1px solid var(--border)", borderRadius: "6px", boxShadow: "0 6px 20px rgba(0,0,0,.22)" },
    ".cm-jf-search": { padding: "6px", font: "12px ui-sans-serif, system-ui, sans-serif", color: "var(--foreground)", backgroundColor: "var(--card)", borderRadius: "6px" },
    ".jf-search-row": { display: "flex", alignItems: "center", gap: "3px" },
    ".jf-search-input": { display: "flex", flex: "1", minWidth: "0", alignItems: "center", border: "1px solid var(--border)", borderRadius: "3px", backgroundColor: "var(--background)" },
    ".jf-search-input:focus-within": { borderColor: "var(--primary)" },
    ".cm-jf-search input": { width: "0", minWidth: "0", flex: "1", padding: "5px 6px", height: "28px", font: "inherit", color: "var(--foreground)", outline: "none", background: "transparent", border: "0" },
    ".cm-jf-search input[aria-invalid=true]": { color: "var(--destructive)" },
    ".jf-search-button": { height: "26px", minWidth: "24px", padding: "0 4px", border: "0", borderRadius: "3px", background: "transparent", color: "var(--muted-foreground)", font: "inherit", cursor: "pointer" },
    ".jf-search-button:hover": { backgroundColor: "var(--muted)", color: "var(--foreground)" },
    ".jf-search-button:focus-visible": { outline: "1px solid var(--primary)" },
    ".jf-search-button[aria-pressed=true]": { backgroundColor: "color-mix(in srgb, var(--primary) 20%, transparent)", color: "var(--foreground)" },
    ".jf-search-button:disabled": { opacity: ".4", cursor: "default" },
    ".jf-search-word": { textDecoration: "underline" },
    ".jf-search-count": { minWidth: "44px", padding: "0 3px", whiteSpace: "nowrap", fontSize: "11px", color: "var(--muted-foreground)", fontVariantNumeric: "tabular-nums" },
    ".jf-search-empty": { color: "var(--destructive)" },
    ".jf-replace-row": { marginTop: "4px", paddingLeft: "27px" },
    ".jf-replace-row[hidden]": { display: "none" },
    ".jf-replace-row input": { border: "1px solid var(--border)", borderRadius: "3px", backgroundColor: "var(--background)" },
    ".jf-search-text-button": { padding: "0 7px" },
    ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--primary) 20%, transparent)", outline: "1px solid color-mix(in srgb, var(--primary) 35%, transparent)" },
    ".cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--primary) 35%, transparent)" },
  })];
}
