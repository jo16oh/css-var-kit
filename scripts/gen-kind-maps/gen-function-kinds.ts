import css from "@webref/css";

// Groups auto-classified from the `for` field
const FOR_FIELD_MAPPING: Record<string, string> = {
  "transform": "transform-function",
  "filter": "filter",
  "<basic-shape>": "basic-shape",
};

// Groups that require manual classification
const MANUAL_GROUPS: Record<string, string[]> = {
  "color": [
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "color-mix",
    "color-interpolate",
    "color-layers",
    "contrast-color",
    "device-cmyk",
    "light-dark",
    "dynamic-range-limit-mix",
    "hdr-color",
    "ictcp",
    "jzazbz",
    "jzczhz",
    "alpha",
    "palette-mix",
  ],
  "image": [
    "linear-gradient",
    "radial-gradient",
    "conic-gradient",
    "repeating-linear-gradient",
    "repeating-radial-gradient",
    "repeating-conic-gradient",
    "image",
    "image-set",
    "-webkit-image-set",
    "cross-fade",
    "element",
    "paint",
    "src",
    "stripes",
    "filter",
  ],
  "transform-function": [
    "matrix3d",
    "perspective",
    "rotate3d",
    "rotateX",
    "rotateY",
    "rotateZ",
    "scale3d",
    "scaleZ",
    "translate3d",
    "translateZ",
  ],
  "transform-list": [
    "transform-interpolate",
    "transform-mix",
  ],
  "easing-function": [
    "cubic-bezier",
    "steps",
    "linear",
  ],
  "animation-timeline": [
    "scroll",
    "view",
    "pointer",
  ],
  "url": ["url", "url-pattern"],
  "length": ["anchor", "anchor-size", "fit-content"],
  "number": ["progress"],
  "integer": ["sibling-index", "sibling-count"],
  "string": [
    "counter",
    "counters",
    "string",
    "content",
    "target-counter",
    "target-counters",
    "target-text",
    "leader",
  ],
  "custom-ident": ["ident", "running"],
  "corner-shape": ["superellipse"],
  "snap": ["snap-block", "snap-inline"],
  "symbols": ["symbols"],
  "ray": ["ray"],
  "grid-track": ["minmax", "repeat"],
  "request-modifier": ["cross-origin", "integrity", "referrer-policy"],
  "contrast-algorithm": ["wcag2"],
  "text-overflow": ["fade"],
  "form-control": ["control-value"],
  "unresolved-arg-dependent": [
    "abs", "acos", "asin", "atan", "atan2",
    "calc", "calc-interpolate", "calc-mix", "calc-size",
    "clamp", "cos", "exp", "hypot", "log",
    "max", "min", "mod", "pow", "rem", "round",
    "sign", "sin", "sqrt", "tan",
    "random",
    "interpolate",
  ],
  "unresolved-pass-through": [
    "var", "env", "attr",
    "if", "first-valid", "toggle", "inherit",
    "random-item", "param",
  ],
  "unresolved-conditional": ["media", "supports"],
  "unresolved-meta": ["type"],
};

function stripParens(name: string): string {
  return name.replace(/\(\)$/, "");
}

async function main() {
  const outPath = Deno.args[0];
  if (!outPath) {
    console.error("Usage: deno run gen-function-kinds.ts <output-path>");
    Deno.exit(1);
  }

  const data = await css.listAll();
  const functions: { name: string; for?: string[] }[] = data.functions ?? [];
  const result: Record<string, string[]> = {};

  // Auto-classify using the `for` field
  for (const fn of functions) {
    if (!fn.for) continue;
    for (const forValue of fn.for) {
      const type = FOR_FIELD_MAPPING[forValue];
      if (!type) continue;
      const name = stripParens(fn.name);
      if (!result[name]) result[name] = [];
      if (!result[name].includes(type)) result[name].push(type);
    }
  }

  // Apply manual groups
  for (const [type, names] of Object.entries(MANUAL_GROUPS)) {
    for (const name of names) {
      if (!result[name]) result[name] = [];
      if (!result[name].includes(type)) result[name].push(type);
    }
  }

  const sorted = Object.fromEntries(
    Object.entries(result).sort(([a], [b]) => a.localeCompare(b)),
  );

  await Deno.writeTextFile(outPath, JSON.stringify(sorted, null, 2));
  console.log(`Written ${Object.keys(sorted).length} functions to ${outPath}`);
}

await main();
