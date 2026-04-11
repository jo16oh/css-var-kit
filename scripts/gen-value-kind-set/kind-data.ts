// @ts-expect-error: no definetely typed
import css from "@webref/css";

import { buildDimensionUnitToKinds } from "./dimension-unit-kinds.ts";
import { buildFunctionToKinds } from "./function-kinds.ts";
import { buildKeywordToTypes, extractTerminalTypes } from "./keyword-kinds.ts";

// lightningcss SyntaxComponentKind variants that share a bit with keyword kinds.
// Names must match the keyword-kinds naming convention so that overlapping
// kinds (e.g. "color") naturally deduplicate.
export const SYNTAX_COMPONENT_KINDS: { kind: string; variant: string }[] = [
  { kind: "length", variant: "Length" },
  { kind: "number", variant: "Number" },
  { kind: "percentage", variant: "Percentage" },
  { kind: "color", variant: "Color" },
  { kind: "image", variant: "Image" },
  { kind: "url", variant: "Url" },
  { kind: "integer", variant: "Integer" },
  { kind: "angle", variant: "Angle" },
  { kind: "time", variant: "Time" },
  { kind: "resolution", variant: "Resolution" },
  { kind: "transform-function", variant: "TransformFunction" },
  { kind: "transform-list", variant: "TransformList" },
  { kind: "string", variant: "String" },
  { kind: "custom-ident", variant: "CustomIdent" },
];

export interface KindData {
  allKinds: string[];
  keywordMap: Record<string, string[]>;
  functionMap: Record<string, string[]>;
  dimensionUnitMap: Record<string, string[]>;
}

function collectAllKinds(...maps: Record<string, string[]>[]): string[] {
  const kinds = new Set<string>();

  for (const map of maps) {
    for (const values of Object.values(map)) {
      for (const kind of values) kinds.add(kind);
    }
  }
  for (const { kind } of SYNTAX_COMPONENT_KINDS) {
    kinds.add(kind);
  }

  return [...kinds].sort();
}

export async function loadKindData(): Promise<KindData> {
  const data = await css.listAll();
  const terminalTypes = extractTerminalTypes(data.types ?? []);
  const keywordMap = buildKeywordToTypes(terminalTypes);
  const functionMap = buildFunctionToKinds(data.functions ?? []);
  const dimensionUnitMap = buildDimensionUnitToKinds();
  const allKinds = collectAllKinds(keywordMap, functionMap, dimensionUnitMap);

  return { allKinds, keywordMap, functionMap, dimensionUnitMap };
}
