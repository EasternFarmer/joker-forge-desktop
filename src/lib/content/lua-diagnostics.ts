import type { Diagnostic } from "@codemirror/lint";
import { parse } from "luaparse";

type LuaParserError = {
  index?: number;
  message?: string;
};

export function getLuaDiagnostics(source: string): Diagnostic[] {
  try {
    parse(source, {
      luaVersion: "LuaJIT",
      extendedIdentifiers: true,
      encodingMode: "none",
      comments: false,
    });
    return [];
  } catch (error) {
    const problem = error as LuaParserError | null;
    if (typeof problem?.index !== "number" || !Number.isFinite(problem.index)) {
      return [{
        from: 0,
        to: 0,
        severity: "warning",
        source: "Lua parser",
        message: "Unable to check Lua syntax. The document may be too deeply nested.",
      }];
    }

    const from = Math.max(0, Math.min(source.length, problem.index));
    const width = (source.codePointAt(from) ?? 0) > 0xffff ? 2 : 1;
    return [{
      from,
      to: Math.min(source.length, from + width),
      severity: "error",
      source: "Lua syntax",
      message: (problem.message ?? "Invalid Lua syntax").replace(/^\[\d+:\d+\]\s*/, ""),
    }];
  }
}
