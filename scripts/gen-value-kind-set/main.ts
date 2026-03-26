import css from "@webref/css";
import { buildKeywordToTypes, extractTerminalTypes } from "./keyword-kinds.ts";
import { buildFunctionToKinds } from "./function-kinds.ts";
import { buildDimensionUnitToKinds } from "./dimension-unit-kinds.ts";

// lightningcss SyntaxComponentKind variants that share a bit with keyword kinds.
// Names must match the keyword-kinds naming convention so that overlapping
// kinds (e.g. "color") naturally deduplicate.
const SYNTAX_COMPONENT_KINDS: { kind: string; variant: string }[] = [
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

function kindToConstName(kind: string): string {
  return kind.replaceAll("-", "_").replaceAll("+", "_plus_").toUpperCase();
}

function collectAllKinds(
  ...maps: Record<string, string[]>[]
): string[] {
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

function generateBitflags(allKinds: string[]): string {
  const lines: string[] = [];
  lines.push("bitflags::bitflags! {");
  lines.push("    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]");
  lines.push("    pub struct ValueKindSet: u128 {");

  for (let i = 0; i < allKinds.length; i++) {
    lines.push(`        const ${kindToConstName(allKinds[i])} = 1 << ${i};`);
  }

  lines.push("");
  lines.push(
    "        // Composite alias: LengthPercentage = Length | Percentage",
  );
  lines.push(
    "        const LENGTH_PERCENTAGE = ValueKindSet::LENGTH.bits() | ValueKindSet::PERCENTAGE.bits();",
  );

  lines.push("    }");
  lines.push("}");
  return lines.join("\n");
}

function generateFromSyntaxComponentKind(): string {
  const lines: string[] = [];
  lines.push(
    "pub fn from_syntax_component_kind(kind: &lightningcss::values::syntax::SyntaxComponentKind) -> ValueKindSet {",
  );
  lines.push(
    "    use lightningcss::values::syntax::SyntaxComponentKind;",
  );
  lines.push("    match kind {");

  for (const { variant, kind } of SYNTAX_COMPONENT_KINDS) {
    if (kind === "custom-ident" || kind === "string") continue;
    const constName = kindToConstName(kind);
    lines.push(
      `        SyntaxComponentKind::${variant} => ValueKindSet::${constName},`,
    );
  }
  lines.push(
    "        // LengthPercentage maps to the composite LENGTH | PERCENTAGE",
  );
  lines.push(
    "        SyntaxComponentKind::LengthPercentage => ValueKindSet::LENGTH_PERCENTAGE,",
  );

  lines.push("        _ => ValueKindSet::empty(),");
  lines.push("    }");
  lines.push("}");
  return lines.join("\n");
}

function generateKindNames(allKinds: string[]): string {
  const lines: string[] = [];
  lines.push("const KIND_NAMES: &[(ValueKindSet, &str)] = &[");
  for (const kind of allKinds) {
    const constName = kindToConstName(kind);
    lines.push(`    (ValueKindSet::${constName}, "${kind}"),`);
  }
  lines.push("];");
  lines.push("");
  lines.push("impl ValueKindSet {");
  lines.push(
    "    pub fn iter_kind_names(self) -> impl Iterator<Item = &'static str> {",
  );
  lines.push("        KIND_NAMES.iter()");
  lines.push("            .filter(move |(flag, _)| self.contains(*flag))");
  lines.push("            .map(|(_, name)| *name)");
  lines.push("    }");
  lines.push("}");
  return lines.join("\n");
}

function generateLookupKindByName(allKinds: string[]): string {
  const lines: string[] = [];
  lines.push(
    "pub fn lookup_kind_by_name(name: &str) -> Option<ValueKindSet> {",
  );
  lines.push("    match &*name.to_ascii_lowercase() {");

  for (const kind of allKinds) {
    const constName = kindToConstName(kind);
    lines.push(`        "${kind}" => Some(ValueKindSet::${constName}),`);
  }

  // Composite alias
  lines.push(
    `        "length-percentage" => Some(ValueKindSet::LENGTH_PERCENTAGE),`,
  );

  lines.push("        _ => None,");
  lines.push("    }");
  lines.push("}");
  return lines.join("\n");
}

function generateLookupFn(
  fnName: string,
  map: Record<string, string[]>,
): string {
  // CSS keywords and function names are ASCII case-insensitive per spec.
  // Merge entries that collide after lowercasing (e.g. "menu" and "Menu").
  const merged: Record<string, string[]> = {};
  for (const [name, kinds] of Object.entries(map)) {
    const lower = name.toLowerCase();
    if (merged[lower]) {
      for (const k of kinds) {
        if (!merged[lower].includes(k)) merged[lower].push(k);
      }
    } else {
      merged[lower] = [...kinds];
    }
  }
  const entries = Object.entries(merged).sort(([a], [b]) => a.localeCompare(b));

  const lines: string[] = [];
  lines.push(`pub fn ${fnName}(name: &str) -> Option<ValueKindSet> {`);
  lines.push("    match &*name.to_ascii_lowercase() {");

  for (const [name, kinds] of entries) {
    const consts = kinds.map((k) => `ValueKindSet::${kindToConstName(k)}`);
    const combined = consts.join(" | ");
    const escaped = name.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
    lines.push(`        "${escaped}" => Some(${combined}),`);
  }

  lines.push("        _ => None,");
  lines.push("    }");
  lines.push("}");
  return lines.join("\n");
}

async function main() {
  const outPath = Deno.args[0];
  if (!outPath) {
    console.error("Usage: deno run main.ts <output-path>");
    Deno.exit(1);
  }

  const data = await css.listAll();
  const terminalTypes = extractTerminalTypes(data.types ?? []);
  const keywordMap = buildKeywordToTypes(terminalTypes);
  const functionMap = buildFunctionToKinds(data.functions ?? []);
  const dimensionUnitMap = buildDimensionUnitToKinds();
  const allKinds = collectAllKinds(keywordMap, functionMap, dimensionUnitMap);

  if (allKinds.length > 128) {
    console.error(
      `Error: too many kinds (${allKinds.length}) for u128 bitflags`,
    );
    Deno.exit(1);
  }

  const sections = [
    "// Generated by scripts/gen-value-kind-set/main.ts",
    "// Do not edit manually.",
    "",
    generateBitflags(allKinds),
    "",
    generateKindNames(allKinds),
    "",
    generateFromSyntaxComponentKind(),
    "",
    generateLookupKindByName(allKinds),
    "",
    generateLookupFn("lookup_keyword_kinds", keywordMap),
    "",
    generateLookupFn("lookup_function_kinds", functionMap),
    "",
    generateLookupFn("lookup_dimension_unit_kinds", dimensionUnitMap),
    "",
  ];

  const code = sections.join("\n");
  await Deno.writeTextFile(outPath, code);
  console.log(
    `Written ValueKindSet (${allKinds.length} kinds) + lookup functions to ${outPath}`,
  );
}

if (import.meta.main) {
  await main();
}
