import css from "@webref/css";

interface TypeEntry {
  name: string;
  syntax?: string;
  extended: unknown[];
}

async function main() {
  const outPath = Deno.args[0];
  if (!outPath) {
    console.error("Usage: deno run gen-keyword-kinds.ts <output-path>");
    Deno.exit(1);
  }

  const data = await css.listAll();
  const terminalTypes = extractTerminalTypes(data.types ?? []);
  const keywordToTypes = buildKeywordToTypes(terminalTypes);

  await Deno.writeTextFile(outPath, JSON.stringify(keywordToTypes, null, 2));
  console.log(
    `Written ${Object.keys(keywordToTypes).length} keywords to ${outPath}`,
  );
}

function extractKeywords(syntax: string): string[] {
  return syntax
    .split("|")
    .map((s) => s.trim())
    .map((s) => s.replace(/^\[|\]$/g, "").trim())
    .filter((s) =>
      s.length > 0 &&
      !s.startsWith("<") &&
      !s.includes("(") &&
      !s.includes(" ") &&
      !s.includes("*") &&
      !s.includes("+") &&
      !s.includes("?") &&
      !s.includes("#") &&
      !s.includes("&")
    );
}

function extractTerminalTypes(
  types: TypeEntry[],
): Map<string, string[]> {
  const result = new Map<string, string[]>();

  for (const type of types) {
    if (!type.syntax) continue;
    const keywords = extractKeywords(type.syntax);
    if (keywords.length === 0) continue;

    const existing = result.get(type.name);
    if (existing) {
      for (const kw of keywords) {
        if (!existing.includes(kw)) existing.push(kw);
      }
    } else {
      result.set(type.name, keywords);
    }
  }

  return result;
}

function buildKeywordToTypes(
  terminalTypes: Map<string, string[]>,
): Record<string, string[]> {
  const result: Record<string, string[]> = {};

  for (const [typeName, keywords] of terminalTypes) {
    for (const kw of keywords) {
      if (result[kw]) {
        if (!result[kw].includes(typeName)) result[kw].push(typeName);
      } else {
        result[kw] = [typeName];
      }
    }
  }

  return Object.fromEntries(
    Object.entries(result).sort(([a], [b]) => a.localeCompare(b)),
  );
}

await main();
