# Actual-Type Batch Enrollment Composition

This packet qualifies the complete shared enrollment execution function on the
actual journal declarations. Unlike the preceding sort packet, the batch theorem
has **no admission precondition**: it derives every sort/search/commit premise.
This advances Gate 1, but does not establish full runtime or HIP/HSA parity.

## Contract and Integration

The source-shared indexed helpers cover header validation, replay scanning,
selected-slot vacancy, reverse-free-stack plan filling, duplicate and retained
alias checks, clearing, refill and commit. They call the previously verified
adaptive sort and binary searches. No journal mutation precedes all fallible
checks, and commit consumes the refilled canonical plan, not sorted scratch.

The top contract proves the exact error decision, unchanged journal/output on
rejection, and exact canonical output, initialized allocations, free-stack
prefix and other-field frames on success. Header errors retain per-entry
precedence; empty batches bypass free-stack corruption; replay wins over later
capacity/corruption errors. Bounds use the actual allocation arena. Unrelated
invalid or duplicate retained-prefix slots remain permissible unless they alias
a selected slot. No global partition, valid-context or custody premise is added.

Production delegates through one line to the shared adapter. Both compilers
instantiate the same ten batch macros over the same declarations; aliases route
to the existing shared sort/search functions. Source wiring, adapters, the public
forwarder and all model source files are bound to the signed source commit.
The cached previous key preserves the prior header's register-friendly loop.

`after == before` on error is equality in the actual-type Verus model, not a
claim about physical addresses, allocator state or unwind. Production's counted
read hook is empty in non-test builds; test counters are deliberately outside
the formal state and are checked by tests. Release unit benchmarks retain that
instrumentation. The public forwarding method is source-inspected and tested,
not itself compiled into this proof root.

The pinned Verus/vstd/Z3 distribution, existing standard library contracts,
Rust compiler, host and review of the macro adapters remain trusted. No new
assumptions, external bodies, axioms or trusted contracts are introduced.
Historical enrollment projection, reader/issued-producer custody and producer
status preservation still need explicit correspondence/factorization lemmas.
Physical storage, allocation failure and normal/unwind semantics remain open.

## Qualification

- Two whole-root positives bracket 28 scoped executable mutations. Positives
  must be exactly 90 verified and zero errors at default verifier limits.
  Negative results must match source-bound diagnostics exactly, including
  typed spans and macro expansions; arbitrary compiler/solver failure is rejected.
- Controls cover entry coordinates, header shape/output/order/empty/free guards,
  reverse-stack indexing, duplicate recognition, replay bounds/results, selected
  vacancy, retained scanning, initialized commit fields/slots/truncation, and
  omitted sorting/clearing/refill/commit or reordered batch error checks.
- Eleven proof inputs, three macro projections (10/13/8 bodies), include
  envelopes and declarations are reconstructed from the signed source commit.
  Source input brackets and the 190-file pinned verifier closure are checked.
- CPU qualification covers the full model suite, formatting, all-target Clippy
  with warnings denied, and two explicit complete-enrollment release benchmarks.
  The 199,936-case malformed-arena oracle is retained. New tests cover selected
  slots above declared capacity, dirty output at the final index, exact counted
  reads on late rejection, and dirty unrelated state across success/rollback.
- Every process receipt must report its expected normal exit and absent owned
  process group. The auditor requires proof completion before the serialized
  CPU campaign and a completed release build before timing.

## Performance Scope

[RESULTS.md](RESULTS.md) retains every case, including overhead. One control uses
the frozen iterator batch body with standard sorting. A second uses that same
body with the current adaptive sort/searches, isolating orchestration and its
compiler choices. Both candidates invoke the actual production method. The
second comparison adds half-occupied success and raw first/last replay and
capacity rejection paths; those rejection fixtures are not saturated-journal
throughput. Each fixture validates results and state before measurement.

Benchmarks use logical CPU 31 on a shared host without exclusivity, sibling-core
isolation or frequency control. Seven alternating rounds and a fixed shuffle
seed do not establish universal speedup or significance for near-unity ratios.
Owned build/solver work is completed before timing. No native GPU or HIP/HSA
measurement is made here; the no-regression/performance-parity gate remains open.

## Reproduction

Use the source named in `SOURCE`, a fresh owned scratch directory, the pinned
Verus installation and a process affinity mask that includes CPU 31:

```sh
python3 docs/evidence/dev-journal-enrollment-composition-2026-09-21/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-journal-enrollment-composition-2026-09-21/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
python3 docs/evidence/dev-journal-enrollment-composition-2026-09-21/audit.py \
  --repo . --packet docs/evidence/dev-journal-enrollment-composition-2026-09-21 \
  --selftest
```

The offline auditor reconstructs the source roster from Git objects, checks exact
artifact/command/diagnostic/workload rosters, replays the results and rejects
rehashed substitutions and developer probes. It can run against an independent
bare/shared Git database after owned scratch removal, without Cargo, a solver,
GPU hardware or the original worktree. Verify commit signatures separately
against the owner's trusted SSH signer configuration.

Construction, historical custody projection, Begin, reader admission/release,
remaining wrapper correspondence and storage/unwind verification remain Gate 1
work. Native pending-consumer admission remains closed.
