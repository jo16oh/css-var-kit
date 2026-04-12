// Groups auto-classified from the `for` field
const FOR_FIELD_MAPPING: Record<string, string> = {
  transform: "transform-function",
  filter: "filter",
  "<basic-shape>": "basic-shape",
};

// Groups that require manual classification
const MANUAL_GROUPS: Record<string, string[]> = {
  color: [
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
  image: [
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
  "transform-list": ["transform-interpolate", "transform-mix"],
  "easing-function": ["cubic-bezier", "steps", "linear"],
  "animation-timeline": ["scroll", "view", "pointer"],
  url: ["url", "url-pattern"],
  length: ["anchor", "anchor-size", "fit-content"],
  number: ["progress"],
  integer: ["sibling-index", "sibling-count"],
  string: [
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
  snap: ["snap-block", "snap-inline"],
  "grid-track": ["minmax", "repeat"],
  "request-modifier": ["cross-origin", "integrity", "referrer-policy"],

  // Too niche or return type not statically determinable
  ignored: [
    "symbols", // @counter-style descriptor
    "ray", // offset-path ray()
    "wcag2", // contrast algorithm
    "fade", // text-overflow
    "control-value", // form control value

    // Return type depends on arguments (math functions)
    "abs",
    "acos",
    "asin",
    "atan",
    "atan2",
    "calc",
    "calc-interpolate",
    "calc-mix",
    "calc-size",
    "clamp",
    "cos",
    "exp",
    "hypot",
    "log",
    "max",
    "min",
    "mod",
    "pow",
    "rem",
    "round",
    "sign",
    "sin",
    "sqrt",
    "tan",
    "random",
    "interpolate",

    // Pass-through: return type depends on substituted value
    "var",
    "env",
    "attr",
    "if",
    "first-valid",
    "toggle",
    "inherit",
    "random-item",
    "param",

    // Conditional: return type depends on condition
    "media",
    "supports",

    // Meta: type() returns a type descriptor, not a value
    "type",
  ],
};

function stripParens(name: string): string {
  return name.replace(/\(\)$/, "");
}

export function buildFunctionToKinds(
  specFunctions: { name: string; for?: string[] }[],
): Record<string, string[]> {
  const result: Record<string, string[]> = {};

  // Auto-classify using the `for` field
  for (const fn of specFunctions) {
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

  // Verify all spec functions are covered
  const mapped = new Set(Object.keys(result));
  const unmapped = specFunctions
    .map((fn) => stripParens(fn.name))
    .filter((name) => !mapped.has(name));
  if (unmapped.length > 0) {
    throw new Error(`${unmapped.length} unmapped functions: ${unmapped.join(", ")}`);
  }

  // Filter out entries whose ALL kinds are ignored
  const filtered: Record<string, string[]> = {};
  for (const [name, kinds] of Object.entries(result)) {
    const kept = kinds.filter((k) => k !== "ignored");
    if (kept.length > 0) filtered[name] = kept;
  }

  return Object.fromEntries(Object.entries(filtered).sort(([a], [b]) => a.localeCompare(b)));
}
