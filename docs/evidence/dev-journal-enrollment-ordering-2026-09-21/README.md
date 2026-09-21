# Actual-Type Enrollment Ordering

This packet qualifies the production batch enrollment **key and slot searches**,
plus a **test-only sorting candidate**. Production keeps `sort_unstable_by_key`:
the proved heap candidate did not pass the CPU performance gate. This is not
complete enrollment refinement or a GPU/HIP/HSA performance result.

## Contracts

Rust and Verus compile the same enrollment declaration and executable helper
macros. Proof instantiations use the actual production key/reference types; no
historical Vec witness, derived trait contract or global journal validity premise
is assumed. The reusable proof-body file can be included in a future production
module without declaring a second set of nominal types.

- Key search is exactly lexicographic membership on a nondecreasing roster,
  including duplicates and arbitrary identity coordinates.
- Slot search is exactly membership on a sorted, all-Some reference slice.
- Candidate swap preserves complete references, including self-swap.
- Candidate sift preserves the outside range, multiplicities and interval origins.
- Candidate sort preserves the complete-reference multiset and yields sorted slots.
- The linear sorted predicate is an equivalence; already-sorted slices are unchanged.

The sort is unshipped, not an alternative hidden production path. Its helpers are
test-only in the Rust adapter and verified unconditionally in the standalone root.
All proof-hook arguments are erased annotations, proof blocks or ghost snapshots.
Source pins and replay bind these adapters to the shared macros.

Full enrollment still needs caller-precondition derivation, header/error ordering,
temporary-output rollback, canonical plan restoration, commit/frame composition,
and a performant verified production sort. Slice contracts do not prove physical
Vec storage, compiler lowering, fallible construction or panic/unwind behavior.

## Qualification

`SOURCE` identifies the signed source commit. `check.py` stages only five Rust
inputs and an authenticated policy projection; it does not restage the historical
proof graph. Two whole-root runs use the pinned Verus release, default proof
limits, four threads and `--no-cheating`. Nine scoped controls mutate the shared
executable body directly expanded by the selected function. Expected diagnostics
were inspected and frozen from development probes, then replayed in the final
campaign. They cover payload corruption, false sorted acceptance, heap child/build/
extraction errors, skipped sorting, key comparison and both membership results.
All summary fields, diagnostics, source spans, expansion trees and JSON types are
checked; only the case directory is normalized. A failed function may emit multiple
diagnostics while the Verus result reports one failed verification unit.

CPU qualification includes the complete runtime-model unit/doctest suite, format
and all-target Clippy checks, and three release-mode paired comparisons. Sorting
fixtures cover ascending, descending, shuffled and duplicate-heavy inputs. Search
queries are batched under one timer, include sizes 0 through 4096, and mix hits and
misses. Complete enrollment compares with the frozen pre-change body on fresh and
half-occupied journals, early/late replay, capacity rejection and prefix aliasing.
Fixture setup/reset is outside timing; results are checked before timing; measurement
order alternates for seven rounds. Sort and enrollment timings include timer
overhead. CPU contention and lack of processor affinity limit performance inference.

`RESULTS.md` reports the final observations without treating CPU microbenchmarks as
GPU parity evidence. The immutable malformed-state oracle, free-stack permutation
tests and indexed-access tests remain the behavioral regression authority; the
frozen benchmark body is not an independent correctness oracle.

## Replay

Run from a checkout containing the source commit and packet:

```sh
python3 -I docs/evidence/dev-journal-enrollment-ordering-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-ordering-2026-09-21
python3 -I docs/evidence/dev-journal-enrollment-ordering-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-ordering-2026-09-21 --selftest
```

The offline audit reconstructs all source inputs from Git objects, checks the exact
case/file roster, terminal receipts, tool closure transcripts, negative diagnostics
and CPU workload rows. It does not rerun the solver or depend on deleted scratch.
Manifest integrity and source replay are not substitutes for verifying the source
and artifact commit signatures with the trusted signing key.
