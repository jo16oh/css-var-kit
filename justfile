gen-kind-set:
    @deno run --allow-read --allow-write \
        scripts/gen-kind-set/main.ts \
        crates/css-var-kit/generated/kind_set.rs
    @rustfmt crates/css-var-kit/generated/kind_set.rs
