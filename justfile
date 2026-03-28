gen-value-kind-set:
    @deno run --allow-read --allow-write \
        scripts/gen-value-kind-set/main.ts \
        crates/css-var-kit/generated/value_kind_set.rs
    @rustfmt crates/css-var-kit/generated/value_kind_set.rs

gen-value-kind-doc:
    @deno run --allow-read --allow-write \
        scripts/gen-value-kind-set/main.ts \
        --gen-doc VALUE_KINDS.md
