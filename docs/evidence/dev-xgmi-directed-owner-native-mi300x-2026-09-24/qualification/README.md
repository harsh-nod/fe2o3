# Directed XGMI Shared-Source CPU Qualification

Status: CPU-tested runtime correction. This packet does not qualify native
execution, faults, formal correspondence, aggregate memory or performance.
The [original native refusal](../dev-xgmi-directed-owner-refused-mi300x-2026-09-24/README.md)
remains failed; the unchanged diamond requires a fresh signed native campaign.

## Implementation

The [contract](../../runtime-xgmi-shared-source-v1.md) describes the joint fix:
exact directed `Read`/`Read` owner admission, a bounded allocation-disjoint FIFO
publication prefix, complete-ready-set flush refusal before native effects, and
aggregate custody validation before healthy shared-roster Busy. Writer hazards
and legacy/ordered profiles remain conservative. No reader owner is discarded
and no false sibling dependency is added.

Sixteen new regressions cover the actual shared admission gate, provisional and
retained provenance, owner corruption/capacity/precedence, both physical
directions, mapping-prefix selection, the 63/64-entry bound, strict flush,
aggregate owner rosters, and Pending/recovered-publication scalar progress.
The prior test-only coarse overlap predicate is removed. Mapping occupancy and
native leaves remain scripted in CPU fixtures, not native authority. Aggregate
tests call production roster/selection helpers but do not construct native
sessions. Two independent read-only source reviews found no release blocker.

## Final Qualification

All fifteen serialized commands finish successfully with their owned child
process groups absent:

- GNU and musl all-feature runtime libraries each pass 1,341 tests; twenty
  existing hardware-only tests are ignored on each target, with none filtered.
- All sixteen new regression names appear as passed on both targets.
- Runtime doctests pass 4 plus 42, totaling 46, with no ignores or filters.
- The unchanged directed-owner example passes six tests on GNU and six on musl.
- The musl CLI builds; Python result/campaign/native-runner suites pass 5/4/5
  tests, including compiled invalid-CLI checks, with no skips.
- Runtime formatting and strict all-feature/all-target Clippy pass.
- Rust/Cargo before/after bytes match. All 3,904 selected source/build/harness
  inputs are unchanged across the run.

`inputs-before.json` and `inputs-after.json` bind the runner and input roster.
Each command directory contains its exact argv, status, wall-clock timestamps,
process group, absence result and raw stdout/stderr. Environment and timeout
policy are recorded in the included runner, not repeated in individual records.
Raw terminal blank lines are preserved. These are source-bound development
checks, not hermetic toolchain attestations or formal refinement.

The musl transcript has one child `running 1 test` banner interleaved between
the promotion diagnostic test's `...` and `ok`. It remains byte-exact; counting
only single-line result records would miss that one success. Child invocations
are not added to the top-level suite total.

Runner SHA-256:
`dcd707747794e763b18bd0e02b362ac541982960eb6ba1c3f209d3312e49fc5b`.

## Development History

Private logs remain under
`/home/harsh/.codex-tmp/fe2o3-xgmi-shared-read-20260924-YGPsyc0Y`.
The first focused attempt failed compilation because a test tried to Debug-format
a native submission record; its bracket also correctly rejected concurrent
formatting. Neither is qualification. The corrected preliminary focused run
passed fourteen selected tests. Strict Clippy then rejected a nested if, which
was corrected before this final frozen run. The final suite includes the later
owner-roster and progress regressions; earlier counts do not replace it.

The target cache is the exclusively owned directed-owner development target.
No shared cache or remote resource was modified by this CPU campaign. Native
R125, Admission R118B, Resources R116/V3 and A1/A2 acceptance remain unchanged.
