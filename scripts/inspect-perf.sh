#!/usr/bin/env bash
set -euo pipefail
test "${GITHUB_ACTIONS:-}" = true
revision=21984f1de592d3b19a0fc6914c4657a1927f4d36
root=evidence/perf-codegen
mkdir -p "$root"
printf '%s\n' "pku-minic/compiler-dev-test-cases $revision" > "$root/upstream.txt"
for name in 18_brainfuck-bootstrap 19_brainfuck-calculator; do
  curl --fail --location --retry 3 --max-time 120 \
    "https://raw.githubusercontent.com/pku-minic/compiler-dev-test-cases/$revision/testcases/perf/$name.c" \
    -o "$root/$name.c"
  printf '%s  %s\n' 0cf3ed2d0665016142ed6c556ddb8f1c23799baefe63414b1930d6e350a316de "$root/$name.c" | sha256sum -c -
  for mode in koopa riscv perf; do
    target/release/sysy_compiler "-$mode" "$root/$name.c" -o "$root/$name.$mode"
  done
  sha256sum "$root/$name.c" "$root/$name.koopa" "$root/$name.riscv" "$root/$name.perf" > "$root/$name.sha256"
  awk '/^  [a-z]/ { count[$1]++; total++ } END { print "static instruction count", total; for (op in count) print op, count[op] }' \
    "$root/$name.riscv" | sort > "$root/$name.static-counts.txt"
done
printf '%s\n' 'Static code-generation diagnostics only; not runtime profiling or performance acceptance. Pinned current upstream source may differ from the pinned official test image.' > "$root/scope.txt"
