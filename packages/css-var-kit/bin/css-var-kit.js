#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");

const PLATFORMS = {
  "darwin-arm64": "@css-var-kit/cli-darwin-arm64",
  "darwin-x64": "@css-var-kit/cli-darwin-x64",
  "linux-x64": "@css-var-kit/cli-linux-x64",
  "linux-arm64": "@css-var-kit/cli-linux-arm64",
  "win32-x64": "@css-var-kit/cli-win32-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkg = PLATFORMS[platformKey];

if (!pkg) {
  console.error(`css-var-kit: unsupported platform ${process.platform}-${process.arch}`);
  process.exit(1);
}

const ext = process.platform === "win32" ? ".exe" : "";
const binPath = path.join(path.dirname(require.resolve(`${pkg}/package.json`)), `cvk${ext}`);

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  if (e.status !== null) {
    process.exit(e.status);
  }
  throw e;
}
