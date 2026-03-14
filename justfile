[group('codegen')]
gen-kind-maps: gen-keyword-kinds gen-function-kinds

[group('codegen')]
gen-keyword-kinds:
    @deno run --allow-read --allow-write \
        scripts/gen-kind-maps/gen-keyword-kinds.ts \
        crates/css-var-kit/generated/keyword-kinds.json

[group('codegen')]
gen-function-kinds:
    @deno run --allow-read --allow-write \
        scripts/gen-kind-maps/gen-function-kinds.ts \
        crates/css-var-kit/generated/function-kinds.json
