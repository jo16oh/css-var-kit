bump-version level:
    #!/usr/bin/env sh
    pnpm bumpp --recursive --release {{level}} --yes --no-commit --no-tag --no-push & \
    cargo set-version --bump {{level}} --workspace & \
    wait
    cargo generate-lockfile

    version=$(node -p "require('./packages/css-var-kit/package.json').version")

    git add packages/*/package.json Cargo.toml Cargo.lock crates/*/Cargo.toml
    git commit -m "chore: bump version to $version"
    git tag "v$version"

gen-value-kind-set:
    @pnpm tsx scripts/gen-value-kind-set/main.ts crates/css-var-kit/generated/value_kind_set.rs
    @rustfmt crates/css-var-kit/generated/value_kind_set.rs

gen-value-kind-doc:
    @pnpm tsx scripts/gen-value-kind-set/main.ts --gen-doc docs/VALUE_KINDS.md
    @pnpm oxfmt docs/VALUE_KINDS.md
