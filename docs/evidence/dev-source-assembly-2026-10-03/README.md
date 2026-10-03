# Retained Source-Span Assembly Qualification

Local CPU/Verus checkpoint, 2026-10-03, based on
`38da718e5eef2793f113a1f6a3f1f183d64cbf70`. The sealed archive contains the
candidate source patch, full command/source receipts, proof inputs/outputs,
diagnostic attempts, executable identities, replay auditor and per-file manifest.
`SHA256SUMS` binds this record and `qualification.tar.xz`.

## Production Change

The checked-u32 prefix checker now executes a shared source-span assembler over
the original borrowed semantic statements and retained correspondence spans.
One scan selects exact root/function/block coordinates and records original
slice indices in private ordinal scratch, bounded to 256 entries. Duplicates
reject; input roster order is irrelevant. The subsequent ordinal walk rejects
missing rows, wrong KIR blocks, discontinuities, overflowing operation counts
and incorrect terminal count/add coordinates. It invokes the existing typed
source normalizer on each preceding original statement, in order.

This replaces the ordinal BTreeMap with O(retained spans + prefix length) work
and O(prefix length) scratch. Sparse KIR value IDs retain their existing tree map.
No new public constructor, normalized-row input or authority path is introduced.
Private step value types now derive equality for exact sequence tests.

## Proof and Structural Bridge

Ordinary Rust and Verus include the same selection/walk body. Its contract binds
both acceptance and the returned step sequence/end offset to a recursive
specification. Reject-all cannot satisfy it. A separate theorem establishes
two-way original-row correspondence. Direct AST evaluation of the statements
**before the captured add** agrees with the assembled steps and composes with
the existing symbolic/concrete fold theorem.

The terminal span is checked, but this theorem intentionally excludes evaluation
or validation of the terminal AST node. That remains a separate `check_parts`
check, along with actual KIR constant/add assembly and terminal correspondence.
ABI discovery, compiler authenticity, physical entry, continuation, memory,
completion, protected Worker admission and launch authority remain unproved here.
Full-normalization and authority flags remain false.

Source guards bind the original container schemas and projections through
functions/blocks/statements, the span roster, capture coordinates, semantic IDs,
KIR BlockId, actual wrapper and caller. Uninspected AST payload erasure retains
its previously documented boundary. These are reviewed structural/source checks,
not a theorem about Rust parsing, name resolution, layout or the complete compiler.
The pinned vstd models of Vec/slices and checked arithmetic remain trusted.

## Accepted Evidence

- Pinned Verus `0.2026.08.09.92f466f`: 71 obligations before/after 48 intended
  logical mutants, including 15 new assembly mutants and a reject-all case.
  Twelve runner controls and both 190-file release-closure checks pass. All 53
  stages retain exact commands, source continuity and absent owned process groups.
- Verifier library: 129 passed, four ignored. Public checked-u32 integration:
  seven passed. All 140 roster entries match outcomes. Four new tests cover
  original-row permutations, duplicate/missing ordinals, later-row exclusion,
  cross-profile counts/gaps, capacity and the terminal-AST proof boundary.
- Thirty documentation tests, strict production-library Clippy,
  no-default-feature library check, changed Rust formatting, shell syntax,
  diff whitespace and local CI-dispatch tests pass.
- Genuine pinned-rustc extraction passes two positive profiles and four expected
  rejections, with distinct source/KIR identities and conditional boundary-value
  checks. Its owned scratch directory is absent after completion.

Final acceptance uses `proof-02`/`solver-02` and the CPU cohort in `final_cpu.py`,
independently replayed by `audit.py`. Whole-source snapshots and stdout/stderr
hashes bind each bounded command. Cargo JSON receipts identify test executables,
fixtures and the compiler wrapper. No GPU run, GitHub Actions execution, matched
performance result, complete KIR proof or runtime milestone closure is claimed.

## Diagnostics and Limits

The initial new Rust tests lacked equality derives for private step values;
that compile failure is retained. The first full campaign overlapped the test
fix and lost source continuity, so it is not accepted. Diagnostic-only mutant
probes found root/function/block changes fail the exact postcondition rather
than the loop invariant. A wrong-statement probe produced two logical diagnostics
and was not accepted by the strict classifier; the final campaign instead uses
a wrong-destination mutant with one intended invariant failure.

The four ignored verifier cases remain subprocess helpers and a pinned runtime
closure test, not hardware tests. Strict Clippy here covers the production
library; previously recorded unrelated test-target warning debt remains outside
this claim. Existing rustc target-feature warnings remain.

For multiply-invalid private inputs, an early source error can now be returned
before a later missing-span error; acceptance is unchanged. No valid public
owner path was found to depend on that diagnostic ordering. Earlier native
multi-GPU evidence retains its own source/binary scope. Next: actual KIR assembly,
terminal AST and ABI/machine obligations, followed by production Worker admission.
