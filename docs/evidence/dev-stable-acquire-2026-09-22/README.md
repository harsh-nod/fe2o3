# Actual Stable-Reader Acquisition

Development evidence for shared actual-type stable-read validation and acquisition.
This packet does not authorize native pending consumers or establish HIP/HSA parity.

The production mutable-slice API, private owner and scan declarations, public macro
invocation and helper adapters are source-bound. The actual and historical
executors run independently. Paired custody preservation requires represented
reader-invariant pre-state; the raw actual contract instead preserves malformed
duplicate free-slot overwrites without claiming custody for that state.

Concrete witnesses execute synchronous acquisition, retain an existing lease
during an overlapping batch, reject a late invalid member, reject occupied output,
and execute raw alias overwrites. Their initial storage is synthetic, not a proof
of actual fallible construction. The generation getter is inspected and bound;
physical vector storage, allocator/unwind behavior and other wrappers remain open.

`check.py` brackets nineteen scoped negative controls with two complete proof-root
runs, authenticates the pinned Verus closure, and records exact solver diagnostics
and terminal process receipts. `cargo-checks.py` records formatting, complete model
tests, Clippy, release compilation and an instrumented frozen-baseline benchmark.
`performance.py` authenticates the original baseline and reviewed adapters, and
checks the complete ordered benchmark roster. `audit.py` reconstructs all inputs
from the source Git object, replays receipt validation and checks artifact hashes.

The benchmark uses the same owner for both variants, with precomputed O(k) reset
outside timing. It compares complete result/state/output, retained custody, lookup
counts and storage identities before timing. Measurements include test-only lookup
instrumentation, timer and dispatch overhead on a non-exclusive CPU. They are not
GPU performance or an uninstrumented production speedup claim.

Replay from a checkout containing the signed source commit in `SOURCE`:

```sh
python3 docs/evidence/dev-stable-acquire-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-stable-acquire-2026-09-22 --selftest
```

For a fresh qualification, run `check.py --repo ... --verus ... --output ...`, then
`cargo-checks.py --repo ... --output ... --target ...` into new owned directories.
The probe mode is development-only and is rejected by the offline auditor.
Results and all observations, including regressions, are retained in `RESULTS.md`.
