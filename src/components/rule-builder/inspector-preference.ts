const INSPECTOR_DISMISSED_KEY = "jokerforge-rule-builder-inspector-dismissed";
let dismissedInMemory = false;
export function wasInspectorDismissed(): boolean {
  try {
    if (typeof window !== "undefined") {
      const stored = window.localStorage.getItem(INSPECTOR_DISMISSED_KEY);
      if (stored !== null) dismissedInMemory = stored === "true";
    }
  } catch {
  }
  return dismissedInMemory;
}

export function rememberInspectorDismissal(dismissed: boolean): void {
  dismissedInMemory = dismissed;
  try {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(INSPECTOR_DISMISSED_KEY, String(dismissed));
    }
  } catch {
  }
}
