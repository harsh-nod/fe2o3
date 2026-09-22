# Actual-Type Production Enrollment Sorting

Production enrollment now invokes the shared, verified adaptive sort. This is a
verification/performance tradeoff, **not acceptance of the earlier no-regression
gate**. Measured overhead on some valid workloads remains. See
[RESULTS.md](RESULTS.md) for every qualified case, including regressions.
This packet is not HIP/HSA parity evidence or native GPU qualification.

## Scope

The shared executable bodies implement monotonic-run recognition, disjoint-half
reversal, hole-based insertion, median-of-three/ninther selection, three-way and
binary partitioning, a cyclic moving-gap partition, depth-limited recursion and
the previously proved heap fallback. The production Rust module and Verus
instantiate these bodies over the actual allocation-reference declarations.
Production enrollment calls the module through a visibility-only re-export,
without an additional executable adapter. Heap-only comparison remains test-only.

The sort requires only an all-Some slice. It proves ascending slots, unchanged
length, preservation of the complete reference multiset (including keys), and
exact identity for initially sorted input. Duplicate slots, arbitrary keys and
empty input are admitted. Reversal proves exact reversed contents. Pivot
selection proves the exact scalar decision and membership in the input. The
cyclic partition maintains the full-reference multiset through its held record
and moving gap. Termination follows the depth budget regardless of balance.
No new assumptions, axioms or external-body contracts were introduced.

The pinned Verus/vstd/Z3 distribution, existing standard slice/split/mem::swap
contracts, Rust compiler, host and review of the macro adapters remain trusted.
This is not machine-code refinement or a complexity theorem. Physical stack/Vec
capacity, allocation failure and panic/unwind behavior remain unproved. Tests
check pointer/capacity stability but do not prove these physical properties.
The production caller's all-Some precondition is established by code inspection
and tests here, not yet by an actual-type full enrollment composition proof.

## Qualification

- Two whole-root positives bracket 19 scoped executable mutations. Positives
  must be exactly 59 verified and zero errors. Each negative must match the
  source-bound expected diagnostics, not merely return a failure code.
- Controls cover run direction, reverse pairing/split, insertion shift/place,
  median/ninther choice, every partition, cyclic gap/restoration, partition
  output, depth calculation, recursion/fallback and both top-level sort paths.
- Eight proof inputs are replayed exactly from the signed source commit.
  Thirteen adaptive and eight ordering macros, both proof-body files, include
  envelopes and actual declarations are authenticated. The source policy and
  190-file verifier closure are checked before and after the campaign.
- CPU qualification includes the complete model unit/doctest suite, formatting,
  all-target Clippy with warnings denied, and three explicit release tests.
  Tests cover exhaustive small ternary inputs, complete-reference preservation,
  exact odd/even reversal, forced heap fallbacks, extreme slots, threshold
  boundaries, and malformed enrollment error/rollback/storage behavior.
- Complete enrollment timing invokes the actual production method against the
  frozen `f78dd2f23` batch body selecting the standard sort. Both retain the
  current shared searches. Fixtures compare result, allocations, free stack and
  output. Builds include test-only read instrumentation in both paths.
- Separate partition measurements compare the cyclic implementation with the
  frozen simple unconditional-swap partition from `5fa588909`. Untimed copied
  recursion counts explore three seeds and both pivot/partition choices; these
  are algorithm diagnostics, not production measurements or complexity proofs.

Benchmark processes are pinned to logical CPU 31 on a shared host, without an
exclusive reservation, sibling-core isolation or frequency control. The owned
build and solver campaigns finish before timing. Seven alternating rounds are
retained for every case. Full-sort and enrollment shuffle use one fixed seed,
and enrollment arenas start empty. Occupied/replay workloads and broader input
families remain future qualification work. Gains on duplicate or sorted inputs
do not cancel valid shuffled-input regressions or establish universal parity.

## Reproduction

Use a fresh owned scratch directory and the pinned Verus installation. Source
must match `SOURCE`; CPU 31 must be available to the benchmark process.

```sh
python3 docs/evidence/dev-journal-enrollment-production-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-enrollment-production-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-enrollment-production-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-production-2026-09-21 \
  --selftest
```

The offline auditor reconstructs inputs from Git objects; checks exact source,
artifact, command, diagnostic, workload and process-terminal rosters; and
replays the summary. It rejects developer probes and rehashed substitutions.
An independent bare/shared Git object database suffices: no original worktree,
scratch data, Cargo, GPU or live solver is needed. Verify commit signatures
separately against the owner's trusted SSH signer configuration.

Gate 1 advances from a test-only sort to a production-connected verified sort.
Construction, full enrollment header/rollback/refill/commit composition, Begin,
reader admission/release, public-wrapper correspondence, physical storage and
normal/unwind semantics remain open. Native pending-consumer admission stays
closed, and performance parity remains open.
