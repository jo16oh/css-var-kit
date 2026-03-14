import css from "@webref/css";

interface TypeEntry {
  name: string;
  syntax?: string;
  extended: unknown[];
}

// Aggregation map: terminal type → semantic kind.
// Only types that merge into a different name are listed here.
// Unlisted types pass through with their original name.
//
// Aggregation criteria: merge types when a user would naturally consider
// their keywords to be "the same kind of value" in a CSS variable definition.
//   - Same semantic category (different ways to define the same thing)
//   - Same slot in the same property (mutually exclusive alternatives)
//   - Spec-internal names normalized to user-facing names
//   - Type hierarchy where child is always valid in parent position
const AGGREGATION_MAP: Record<string, string> = {
  // All color definition methods resolve to the same <color> type:
  // named-color (red, blue), system-color (Canvas), deprecated-color (ActiveBorder),
  // color (currentColor), color-base (transparent)
  "named-color": "color",
  "system-color": "color",
  "deprecated-color": "color",
  "color": "color",
  "color-base": "color",

  // All color space identifiers appear at the same <color-space> position:
  // predefined-rgb (srgb, display-p3), rectangular (lab, oklab),
  // polar (hsl, oklch), xyz (xyz, xyz-d50)
  "predefined-rgb": "color-space",
  "rectangular-color-space": "color-space",
  "polar-color-space": "color-space",
  "xyz-space": "color-space",

  // All easing function keywords appear at the same <easing-function> position:
  // linear-easing (linear), cubic-bezier (ease, ease-in-out),
  // step-easing (step-start, step-end)
  "linear-easing-function": "easing-function",
  "cubic-bezier-easing-function": "easing-function",
  "step-easing-function": "easing-function",

  // Both generic font family types appear at the same <generic-family> position:
  // generic-complete (serif, sans-serif), generic-incomplete (ui-serif, ui-monospace)
  "generic-complete": "generic-family",
  "generic-incomplete": "generic-family",

  // Spec uses "font-weight-absolute" and "font-width-css3" as internal names
  "font-weight-absolute": "font-weight",
  "font-width-css3": "font-width",
  "font-variant-css2": "font-variant",

  // All box model types share a hierarchy and their keywords
  // are interchangeable where a parent type is expected:
  // geometry-box → shape-box → visual-box, coord-box → paint-box → visual-box
  "visual-box": "box",
  "shape-box": "box",
  "paint-box": "box",
  "coord-box": "box",
  "layout-box": "box",
  "geometry-box": "box",

  // <repetition> is a child of <repeat-style>, both for background-repeat
  "repetition": "repeat-style",

  // All display subtypes are valid as the sole value of `display`:
  // display-outside (block, inline), display-inside (flex, grid),
  // display-box (none, contents), display-internal (table-cell),
  // display-legacy (inline-block, inline-flex)
  "display-outside": "display",
  "display-inside": "display",
  "display-box": "display",
  "display-internal": "display",
  "display-legacy": "display",

  // All alignment keywords are interchangeable across alignment properties
  // (justify-content, align-items, justify-self, etc.)
  "content-position": "alignment",
  "self-position": "alignment",
  "content-distribution": "alignment",
  "overflow-position": "alignment",
  "baseline-position": "alignment",

  // Spec prefixes animation/transition descriptor names with "single-"
  "single-animation-direction": "animation-direction",
  "single-animation-fill-mode": "animation-fill-mode",
  "single-animation-play-state": "animation-play-state",
  "single-animation-composition": "animation-composition",
  "single-animation-timeline": "animation-timeline",
  "single-animation-iteration-count": "animation-iteration-count",
  "single-animation": "single-animation",
  "single-transition": "single-transition",
  "single-transition-property": "transition-property",
  "transition-behavior-value": "transition-behavior",

  // All position sub-types and side-or-corner share the same position keywords
  // (left, right, top, bottom, center, etc.)
  "position-one": "position",
  "position-two": "position",
  "position-three": "position",
  "position-four": "position",
  "side-or-corner": "position",

  // position-area and position-area-query share the same anchor positioning keywords
  "position-area-query": "position-area",

  // All three ligature value types appear at the same position in font-variant-ligatures
  "common-lig-values": "ligature-values",
  "discretionary-lig-values": "ligature-values",
  "historical-lig-values": "ligature-values",

  // Spec uses "cursor-predefined" internally
  "cursor-predefined": "cursor",

  // <ray-size> keywords (closest-side, farthest-corner, etc.) are a subset of <radial-extent>
  "ray-size": "radial-extent",

  // <inflexible-breadth> is a subset of <track-breadth> (min-content, max-content, auto)
  "inflexible-breadth": "track-breadth",

  // scroller (scroll(), view()) and pointer-source (pointer()) share
  // identical keywords: root, nearest, self
  "scroller": "source",
  "pointer-source": "source",

  // axis and pointer-axis share identical keywords: block, inline, x, y
  "pointer-axis": "axis",

  // outline-style accepts the same keywords as border-style (+ auto)
  "outline-line-style": "line-style",

  // Both shape command position types describe positions within CSS shapes
  "horizontal-line-command": "shape-command-position",
  "vertical-line-command": "shape-command-position",

  // Spec uses "corner-shape-value" internally
  "corner-shape-value": "corner-shape",

  // Spec uses "legacy-pseudo-element-selector" internally
  "legacy-pseudo-element-selector": "pseudo-element",

  // source-size and source-size-value both describe <source-size> for <img srcset>
  "source-size-value": "source-size",

  // Spec uses "navigation-*-keyword" internally
  "navigation-location-keyword": "navigation-location",
  "navigation-type-keyword": "navigation-type",

  // "an+b" contains "+" which is not valid in Rust identifiers
  "an+b": "an-plus-b",
};

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

function aggregateType(terminalType: string): string {
  return AGGREGATION_MAP[terminalType] ?? terminalType;
}

function buildKeywordToTypes(
  terminalTypes: Map<string, string[]>,
): Record<string, string[]> {
  const result: Record<string, string[]> = {};

  for (const [typeName, keywords] of terminalTypes) {
    const aggregated = aggregateType(typeName);
    for (const kw of keywords) {
      if (result[kw]) {
        if (!result[kw].includes(aggregated)) result[kw].push(aggregated);
      } else {
        result[kw] = [aggregated];
      }
    }
  }

  return Object.fromEntries(
    Object.entries(result).sort(([a], [b]) => a.localeCompare(b)),
  );
}

await main();
