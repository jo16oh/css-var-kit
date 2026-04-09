import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";

import { workspace } from "vscode";

const PLATFORM_PACKAGES: Record<string, string> = {
  "darwin-arm64": "@css-var-kit/cli-darwin-arm64",
  "darwin-x64": "@css-var-kit/cli-darwin-x64",
  "linux-arm64": "@css-var-kit/cli-linux-arm64",
  "linux-x64": "@css-var-kit/cli-linux-x64",
  "win32-x64": "@css-var-kit/cli-win32-x64",
};

const BIN_NAME = process.platform === "win32" ? "cvk.exe" : "cvk";

function fromConfig(): string | undefined {
  const configured = workspace.getConfiguration("cvk").get<string | null>("path", null);
  if (configured && existsSync(configured)) {
    return configured;
  }
  return undefined;
}

function fromNodeModules(): string | undefined {
  const pkg = PLATFORM_PACKAGES[`${process.platform}-${process.arch}`];
  if (!pkg) return undefined;

  const workspaceRoot = workspace.workspaceFolders?.[0]?.uri.fsPath;
  if (!workspaceRoot) return undefined;

  try {
    const pkgJson = require.resolve(`${pkg}/package.json`, { paths: [workspaceRoot] });
    const binPath = join(dirname(pkgJson), BIN_NAME);
    if (existsSync(binPath)) return binPath;
  } catch {
    // package not installed
  }
  return undefined;
}

function fromPath(): string | undefined {
  try {
    const result = execFileSync(process.platform === "win32" ? "where" : "which", ["cvk"], {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    const resolved = result.trim().split("\n")[0];
    if (resolved && existsSync(resolved)) return resolved;
  } catch {
    // not on PATH
  }
  return undefined;
}

function fromBundle(): string | undefined {
  const bundled = join(__dirname, "..", "bin", BIN_NAME);
  if (existsSync(bundled)) return bundled;
  return undefined;
}

function fromDevBuild(): string | undefined {
  let dir = __dirname;
  for (let i = 0; i < 5; i++) {
    dir = dirname(dir);
    const candidate = join(dir, "target", "debug", BIN_NAME);
    if (existsSync(candidate)) return candidate;
  }
  return undefined;
}

export function resolveBinary(): string | undefined {
  return fromConfig() ?? fromNodeModules() ?? fromPath() ?? fromBundle() ?? fromDevBuild();
}
