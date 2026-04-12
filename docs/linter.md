# Linter

```sh
cvk lint
```

By default, `cvk lint` looks for `cvk.json` (or `cvk.jsonc`) in the current project and lints all css files. Run `cvk lint -h` for available options.

## Configuration

Create a `cvk.json` (or `cvk.jsonc`) file in your project root.

```jsonc
// Default configuration (all fields are optional).
{
  "rootDir": ".",
  "definitionFiles": ["**/*.css"],
  "include": [
    "!**/node_modules/**",
    "!**/target/**",
    "!**/.git/**",
    "!**/dist/**",
    "!**/build/**",
    "!**/vendor/**",
  ],
  "rules": {
    "no-undefined-variable-use": "error",
    "no-variable-type-mismatch": "error",
    "no-inconsistent-variable-definition": "error",
    // enforce-variable-use is "off" by default.
    // When enabled, the following defaults apply to its fields:
    "enforce-variable-use": {
      "severity": "error",
      "types": [],
      "allowedFunctions": ["calc", "min", "max", "clamp", "env"],
      "allowedValues": [
        "inherit",
        "initial",
        "unset",
        "revert",
        "revert-layer",
        "currentColor",
        "transparent",
      ],
      "allowedProperties": [],
    },
  },
}
```

### `definitionFiles`

Glob patterns that determine which files are scanned for CSS variable **definitions** and are also **linted**. Defaults to `["**/*.css"]`.

Supports negation patterns (e.g. `"!**/vendor/**"`). The last matching pattern wins.

```jsonc
{
  "definitionFiles": ["**/*.css", "**/*.scss"],
}
```

> `lookupFiles` is accepted as a legacy alias and is overridden when `definitionFiles` is present.

### `include`

Additional glob patterns for definition-only sources (not linted). Supports negation patterns (`!`) to exclude files from linting and definition collection. The last matching pattern wins.

Default configurations are prepended to user-supplied patterns, so they can be selectively overridden. For example, `"node_modules/my-ui-lib/dist/tokens.css"` adds that file as a definition source despite the `!**/node_modules/**` default.

### Rule severity

Each rule can be set to one of the following severity levels:

| Value               | Description                                    |
| ------------------- | ---------------------------------------------- |
| `"error"` or `"on"` | Report as an error (causes non-zero exit code) |
| `"warn"`            | Report as a warning                            |
| `"off"`             | Disable the rule                               |

## Rules

### `no-undefined-variable-use`

Reports usages of CSS variables that are not defined anywhere in the lookup files.

```css
/* ✗ BAD — --accent is never defined */
.btn {
  color: var(--accent);
}

/* ✓ GOOD */
:root {
  --accent: #07f;
}
.btn {
  color: var(--accent);
}
```

### `no-inconsistent-variable-definition`

Reports when the same CSS variable is defined with conflicting value types across different selectors or files.

```css
/* ✗ BAD — --x is defined as both a color and a length */
:root {
  --x: red;
}
.dark {
  --x: 16px;
}

/* ✓ GOOD — both definitions are colors */
:root {
  --x: red;
}
.dark {
  --x: blue;
}
```

### `no-variable-type-mismatch`

Reports when a CSS variable's resolved type does not match the property it is used in.

This rule requires both `no-undefined-variable-use` and `no-inconsistent-variable-definition` to be enabled.

```css
:root {
  --size: 16px;
  --accent: #07f;
}

/* ✗ BAD — --size is a length, not a color */
.btn {
  color: var(--size);
}

/* ✓ GOOD */
.btn {
  color: var(--accent);
}
```

### `enforce-variable-use`

Enforces that literal values of the specified types must use CSS variables instead of being written inline. This rule helps maintain consistency in design systems by ensuring that values like colors and sizes are centralized as variables.

```jsonc
"enforce-variable-use": {
  "severity": "warn",
  // Value types to enforce (see "Value types" below)
  "types": ["color", "length"],
  // Functions whose arguments are not checked
  "allowedFunctions": ["calc", "min", "max", "clamp", "env", "linear-gradient"],
  // Literal values that are always allowed
  "allowedValues": ["inherit", "initial", "unset", "revert", "revert-layer", "currentColor", "transparent", "none"],
  // Properties to exempt from this rule.
  // A string exempts the property entirely.
  // An object with allowedKinds exempts only the specified types.
  "allowedProperties": [
    "display",
    { "propertyName": "border", "allowedKinds": ["length"] }
  ]
}
```

```css
/* With types: ["color"] */

/* ✗ BAD */
.btn {
  color: red;
  background: #fff;
}

/* ✓ GOOD */
.btn {
  color: var(--text);
  background: var(--bg);
  color: inherit; /* allowed value */
}
```

#### Value types

Common types for the `types` and `allowedKinds` fields:

`color`, `length`, `length-percentage`, `percentage`, `number`, `integer`, `angle`, `time`, `resolution`, `string`, `url`, `image`, `display`, `position`, `line-style`, `easing-function`, `filter`, `transform-function`, `font-weight-absolute`, `generic-family`, `flex`, `frequency`

See [VALUE_KINDS.md](VALUE_KINDS.md) for the full list of supported types.

## Suppressing diagnostics

Use `/* cvk-ignore */` comments to suppress diagnostics for the next declaration:

```css
/* Suppress all rules */
/* cvk-ignore */
.btn {
  color: var(--undefined);
}

/* Suppress a specific rule */
/* cvk-ignore: no-undefined-variable-use */
.btn {
  color: var(--undefined);
}
```
