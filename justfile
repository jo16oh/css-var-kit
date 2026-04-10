pkgs := "packages/css-var-kit \
         packages/vscode \
         packages/cli-darwin-arm64 \
         packages/cli-darwin-x64 \
         packages/cli-linux-arm64 \
         packages/cli-linux-x64 \
         packages/cli-win32-x64"

bump-version level:
    #!/usr/bin/env sh
    for dir in {{pkgs}}; do
      (cd "$dir" && pnpm bumpp --release {{level}} --yes --no-commit --no-tag --no-push)
    done
    cargo set-version --bump {{level}} --workspace
    wait

    just sync-optional-deps
    pnpm install --lockfile-only
    cargo generate-lockfile

    version=$(node -p "require('./packages/css-var-kit/package.json').version")

    git add packages/*/package.json Cargo.toml Cargo.lock crates/*/Cargo.toml pnpm-lock.yaml
    git commit -m "chore: bump version to $version"
    git tag "v$version"

sync-optional-deps:
    @node -e "\
      const fs = require('fs');\
      const p = './packages/css-var-kit/package.json';\
      const pkg = JSON.parse(fs.readFileSync(p, 'utf8'));\
      for (const k of Object.keys(pkg.optionalDependencies || {})) {\
        pkg.optionalDependencies[k] = pkg.version;\
      }\
      fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');\
    "

gen-value-kind-set:
    @pnpm tsx scripts/gen-value-kind-set/main.ts crates/css-var-kit/generated/value_kind_set.rs
    @rustfmt crates/css-var-kit/generated/value_kind_set.rs

gen-value-kind-doc:
    @pnpm tsx scripts/gen-value-kind-set/main.ts --gen-doc docs/VALUE_KINDS.md
    @pnpm oxfmt docs/VALUE_KINDS.md
