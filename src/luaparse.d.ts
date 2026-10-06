import type { Chunk, Options, Parser } from "luaparse";

declare module "luaparse" {
  export type LuaParserOptions = Partial<Omit<Options, "extendedIdentifiers">> & {
    extendedIdentifiers?: boolean;
  };

  export function parse(code: string, options: LuaParserOptions & { wait: true }): Parser;
  export function parse(code: string, options?: LuaParserOptions): Chunk;
  export function parse(options?: LuaParserOptions): Parser;
}
