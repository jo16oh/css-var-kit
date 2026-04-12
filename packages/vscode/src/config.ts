import { workspace, type WorkspaceConfiguration } from "vscode";

interface RawRules {
  "no-undefined-variable-use"?: string | undefined;
  "no-variable-type-mismatch"?: string | undefined;
  "no-inconsistent-variable-definition"?: string | undefined;
  "enforce-variable-use"?: unknown;
}

interface RawLspConfig {
  logFile?: string | undefined;
}

interface InitializationOptions {
  rootDir?: string | undefined;
  lookupFiles?: string[] | undefined;
  excludeFiles?: string[] | undefined;
  rules?: RawRules | undefined;
  lsp?: RawLspConfig | undefined;
}

const CONFIG_KEYS = [
  "rootDir",
  "lookupFiles",
  "excludeFiles",
  "rules.noUndefinedVariableUse",
  "rules.noVariableTypeMismatch",
  "rules.noInconsistentVariableDefinition",
  "rules.enforceVariableUse",
  "lsp.logFile",
] as const;

type ConfigKey = (typeof CONFIG_KEYS)[number];

function isExplicitlySet(config: WorkspaceConfiguration, key: ConfigKey): boolean {
  const inspect = config.inspect(key);
  if (!inspect) return false;
  return (
    inspect.globalValue !== undefined ||
    inspect.workspaceValue !== undefined ||
    inspect.workspaceFolderValue !== undefined
  );
}

export function buildInitializationOptions(): InitializationOptions | undefined {
  const config = workspace.getConfiguration("cvk");
  const opts: InitializationOptions = {};
  let hasAny = false;

  if (isExplicitlySet(config, "rootDir")) {
    opts.rootDir = config.get<string>("rootDir");
    hasAny = true;
  }

  if (isExplicitlySet(config, "lookupFiles")) {
    opts.lookupFiles = config.get<string[]>("lookupFiles");
    hasAny = true;
  }

  if (isExplicitlySet(config, "excludeFiles")) {
    opts.excludeFiles = config.get<string[]>("excludeFiles");
    hasAny = true;
  }

  const rules: RawRules = {};
  let hasRules = false;

  if (isExplicitlySet(config, "rules.noUndefinedVariableUse")) {
    rules["no-undefined-variable-use"] = config.get("rules.noUndefinedVariableUse");
    hasRules = true;
  }
  if (isExplicitlySet(config, "rules.noVariableTypeMismatch")) {
    rules["no-variable-type-mismatch"] = config.get("rules.noVariableTypeMismatch");
    hasRules = true;
  }
  if (isExplicitlySet(config, "rules.noInconsistentVariableDefinition")) {
    rules["no-inconsistent-variable-definition"] = config.get(
      "rules.noInconsistentVariableDefinition",
    );
    hasRules = true;
  }
  if (isExplicitlySet(config, "rules.enforceVariableUse")) {
    rules["enforce-variable-use"] = config.get("rules.enforceVariableUse");
    hasRules = true;
  }

  if (hasRules) {
    opts.rules = rules;
    hasAny = true;
  }

  if (isExplicitlySet(config, "lsp.logFile")) {
    opts.lsp = { logFile: config.get<string>("lsp.logFile") };
    hasAny = true;
  }

  return hasAny ? opts : undefined;
}
