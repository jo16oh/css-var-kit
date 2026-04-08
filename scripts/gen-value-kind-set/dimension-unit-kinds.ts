// @ts-expect-error: no @types
import { lexer } from "css-tree";

// lexer.units is typed as Record<string, string[]> at runtime:
//   { length: ["px", "em", ...], angle: ["deg", ...], time: ["s", "ms"], ... }
// @types/css-tree does not include this property, so we use `as` to assert.
export function buildDimensionUnitToKinds(): Record<string, string[]> {
  const units = lexer.units as Record<string, string[]>;
  return Object.fromEntries(
    Object.entries(units).flatMap(([kind, unitList]) => unitList.map((unit) => [unit, [kind]])),
  );
}
