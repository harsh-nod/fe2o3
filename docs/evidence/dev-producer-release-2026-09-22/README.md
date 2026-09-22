# Actual Producer-Read Release Qualification

This developer packet binds the runtime's shared release body to its raw execution
contract and independently executed historical lifecycle. It is not a native GPU
qualification or a claim of full HIP/HSA parity. Gate 1 remains open.

The packet records two whole-root positives around 24 scoped negative controls,
the pinned Verus distribution, complete source brackets, exact terminal process
receipts, CPU tests and an instrumented frozen-versus-shared release benchmark.
Probes, syntax errors, resource exhaustion and timed-out jobs are not evidence.
The auditor replays exact source projections, diagnostics and benchmark rows from
the signed source commit named in `SOURCE`, without requiring the original scratch.

Release accepts Pending, Unknown, Success and NoEffect. Header success activates
a sufficient count-storage coverage condition; no combined-budget, incarnation
epoch, arena partition or producer issuer-validity premise is imposed. Uniqueness
comes from lookup identity and strict reference ordering. Both executions preserve
the whole stable owner and producer incarnation. Physical capacity is explicitly
observed in the proof and remains a separate refinement boundary.

The historical projection contains 23 unchanged declarations. The frozen Rust
baseline is the public release method at `db5dd95e182ca7c90a00e3e98d234710c90c6264`,
renamed only for testing. Query helpers are unchanged, not replaced by older query
baselines. The original ordering helper is now test-only. Reviewed public/private
adapters, including the delayed capacity expression, have independent source pins.

Four proof fixture/witness functions cover live subset release and stale replay,
late errors, physical shortage, all raw statuses, malformed prefixes and epoch zero.
They are synthetic reachability examples, not Rust constructor/Begin proofs. CPU
fixtures cover actual lifecycle construction, mixed states, distinct incarnations,
retained producer/stable reads, full state/storage identity and exact access counts.
Early lookup errors with missing counts extend beyond the conservative formal
domain and are tested only as concrete Rust cases.

The 80-case benchmark emits 1,120 ordered rows with 11 fields across seven alternating
rounds. O(k) restoration, allocation, assertions and counter reset are outside timing.
Every variant's post-state is checked before reset. Non-exclusive CPU timing and
test instrumentation limit interpretation; measured regressions are not discarded.

Replay from a checkout or Git object database:

```sh
python3 docs/evidence/dev-producer-release-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-producer-release-2026-09-22 --selftest
```

To record a fresh campaign, first commit the reviewed source, then use an owned
scratch directory. Run proof qualification before CPU qualification, serially:

```sh
python3 docs/evidence/dev-producer-release-2026-09-22/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-producer-release-2026-09-22/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
```

Generate `SOURCE`, `RESULTS.md` and `SHA256SUMS` from the recorded campaign, audit,
remove only owned scratch after every child process exits, and audit again. Do not
edit captured transcripts or treat a probe campaign as a recorded qualification.

Open boundaries include outer stable wrappers, construction/reachability, physical
storage/nonallocation, allocator/unwind and producer-quiescence authenticity.
Native pending-consumer admission remains closed. This packet changes no native
runtime, touches no remote GPU, and establishes no GPU performance advantage.
