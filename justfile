package-jsons := "packages/css-var-kit/package.json \
                 packages/vscode/package.json \
                 packages/cli-darwin-arm64/package.json \
                 packages/cli-darwin-x64/package.json \
                 packages/cli-linux-arm64/package.json \
                 packages/cli-linux-x64/package.json \
                 packages/cli-win32-x64/package.json"

zed-pkg := "crates/zed-extension"

# Bumps the version on a release branch cut from the latest main and opens its PR; without `level`, bumpp prompts for it.
bump-version level="":
    #!/usr/bin/env sh
    set -eu

    git diff --quiet HEAD -- || { echo 'tracked files differ from HEAD; commit them first'; exit 1; }

    git switch main
    git pull --ff-only

    pnpm bumpp {{package-jsons}} --no-commit --no-tag --no-push {{ if level == "" { "" } else { "--release " + level } }}
    version=$(node -p "require('./packages/css-var-kit/package.json').version")
    cargo set-version --workspace "$version"

    just sync-optional-deps
    pnpm install --lockfile-only
    cargo generate-lockfile

    # bumpp only picks the version; the branch is named after it, so it is cut afterwards.
    git switch -c "chore/release-$version"
    git add packages/*/package.json Cargo.toml Cargo.lock crates/*/Cargo.toml pnpm-lock.yaml
    git commit -m "chore: release v$version"
    git push -u origin HEAD
    gh pr create --fill --label skip-changelog

# Tags the merged release on main; the pushed tag triggers the release workflow.
push-tag:
    #!/usr/bin/env sh
    set -eu

    git diff --quiet HEAD -- || { echo 'tracked files differ from HEAD; commit them before tagging'; exit 1; }

    git switch main
    git pull --ff-only

    tag="v$(node -p "require('./packages/css-var-kit/package.json').version")"
    git tag "$tag"
    git push origin "$tag"

bump-zed-version level:
    #!/usr/bin/env sh
    set -e
    (cd {{zed-pkg}} && cargo set-version --bump {{level}})
    version=$(grep '^version' {{zed-pkg}}/Cargo.toml | head -1 | cut -d'"' -f2)
    sed -i.bak "s/^version = \".*\"/version = \"$version\"/" {{zed-pkg}}/extension.toml
    rm {{zed-pkg}}/extension.toml.bak
    (cd {{zed-pkg}} && cargo generate-lockfile)
    tombi format {{zed-pkg}}/Cargo.toml {{zed-pkg}}/extension.toml {{zed-pkg}}/Cargo.lock

    git add {{zed-pkg}}/Cargo.toml {{zed-pkg}}/extension.toml {{zed-pkg}}/Cargo.lock
    git commit -m "chore(zed): bump version to $version"
    git tag "zed-v$version"

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

gen-value-kinds: gen-value-kind-set gen-value-kind-doc gen-value-kind-schema

gen-value-kind-set:
    @pnpm node scripts/gen-value-kind-set/main.ts crates/css-var-kit/generated/value_kind_set.rs
    @rustfmt crates/css-var-kit/generated/value_kind_set.rs

gen-value-kind-doc:
    @pnpm node scripts/gen-value-kind-set/main.ts --gen-doc docs/VALUE_KINDS.md
    @pnpm oxfmt docs/VALUE_KINDS.md

gen-value-kind-schema:
    @pnpm node scripts/gen-value-kind-set/main.ts --gen-schema packages/css-var-kit/value-kinds.schema.json
