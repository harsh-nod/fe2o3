# Actual-Type Adaptive Enrollment Sorting

This packet qualifies a **test-only** sorting candidate. Production continues to
use `sort_unstable_by_key`. See [RESULTS.md](RESULTS.md) for the recorded CPU
tradeoffs; this is not HIP/HSA parity evidence or native GPU qualification.

## Scope

The shared executable macros implement monotonic-run recognition and reversal,
hole-based insertion sorting, median-of-three selection, three-way/binary/Lomuto
partitioning, depth-limited recursive composition and the already proved heap
fallback. Rust tests and Verus instantiate the same executable bodies over the
actual journal allocation-reference declarations.

The entry contract requires only an all-Some slice. It proves ascending slots,
unchanged length, preservation of the complete reference multiset (including key
payloads), and exact identity when the input was already sorted. Duplicate slots,
arbitrary keys and empty input are admitted. Recursive termination follows the
depth budget independently of partition balance. No new assumptions, axioms or
external-body contracts were introduced.

The pinned Verus/vstd/Z3 distribution, standard slice contracts, Rust compiler,
host and reviewer assessment of the macro adapters remain trusted. The proof
does not establish machine-code refinement, a complexity theorem, physical
stack/Vec capacity, allocation failure, panic/unwind behavior, or complete batch
enrollment admission/rollback/commit correspondence. Tests check pointer/capacity
stability but are not a proof of these physical properties.

## Qualification

- Two whole-root positives bracket 15 scoped executable mutations. Positive
  results must be exactly 57 verified and zero errors. Each negative must match
  the source-bound expected diagnostics, not merely return a failure code.
- Negative controls cover run direction, reversal, insertion shift/placement,
  median selection, each partition, partition output, depth calculation,
  recursive progress/fallback/composition and both top-level sorting paths.
- Eight proof inputs are replayed exactly from the signed source commit. Both
  shared macro files and both proof-body files pass the pinned source policy.
  Both include envelopes and actual declarations are source-bound.
- The 190-file verifier closure and source input hashes are checked before and
  after the campaign. Receipts record normal terminal exits and absent owned
  process groups. Development probes are rejected by the offline auditor.
- CPU qualification covers the complete model test/doctest suite, formatting,
  all-target Clippy with warnings denied, and two explicit release benchmarks.
  Full-reference preservation, forced heap fallbacks, dispatch boundaries,
  mixed extreme slots and exhaustive ternary small inputs are tested.
- The full-enrollment control is the frozen `f78dd2f23` batch body with receiver
  substitution and a sort selector. Both branches retain current shared
  searches. Fixtures compare return values, allocations, free stack and output
  with the unchanged production entry point before measurement.

The host is shared and CPU affinity/frequency are not isolated. Seven alternating
rounds per case do not justify statistical-significance or universal-speedup
claims. End-to-end validation overhead can hide sort regressions. Before any
production promotion, rerun the actual release production entry point against
the frozen baseline, not only the copied selector control.

## Reproduction

From the repository root, with a new owned scratch directory and the pinned
Verus installation:

```sh
python3 docs/evidence/dev-journal-enrollment-adaptive-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-enrollment-adaptive-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-enrollment-adaptive-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-adaptive-2026-09-21 \
  --selftest
```

The source must match `SOURCE`. The auditor reconstructs inputs from Git objects,
checks exact artifact/command/diagnostic/workload rosters, replays the summary,
and rejects rehashed source, receipt, probe and timing substitutions. It can run
from an independent bare/shared Git object database without scratch directories,
the original worktree, GPU hardware, Cargo or a live solver. Commit signatures
are verified separately with the owner's trusted SSH signer configuration.

This advances Gate 1's actual-type candidate coverage. Production sorting and
full lifecycle composition remain open; native pending-consumer admission stays
closed.
