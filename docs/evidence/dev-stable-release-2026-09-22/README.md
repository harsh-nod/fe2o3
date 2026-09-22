# Actual Stable-Reader Release

Development evidence for shared actual-type stable lease lookup and release.
This packet does not authorize native pending consumers or establish HIP/HSA parity.

The public slice API, private owner/scan declarations and helper adapters are
source-bound. Production observes real `Vec::capacity()` only after the original
early-return header prefix. Proof execution uses the same bodies and an explicit
capacity observation; the actual and historical executors run independently.
The paired theorem requires represented reader-invariant pre-state, not successful
admission or desired post-state. The raw actual contract requires matching reader
and allocation storage lengths and derives selected uniqueness from preflight.

Concrete acquisitions establish three overlapping synchronous leases. Witnesses
release a subset while preserving unrelated custody, reject insufficient capacity,
duplicates, replay and stale references, and reacquire with a fresh incarnation.
Separate raw witnesses preserve identity on late undercount and preserve a malformed
existing free prefix on success without claiming a valid reader invariant there.
Initial storage is synthetic, not a proof of actual fallible constructor reachability.

`check.py` brackets twenty scoped negative controls with two complete proof-root
runs, authenticates the pinned Verus closure, and records exact solver diagnostics
and terminal process receipts. `cargo-checks.py` records formatting, complete model
tests, Clippy, release compilation and an instrumented frozen-baseline benchmark.
`performance.py` authenticates original lookup/release, unchanged validation and
reviewed adapters, and checks all 728 ordered benchmark rows across 52 cases.
`audit.py` reconstructs inputs from the source Git object and checks receipts,
diagnostics, serialization, artifact hashes and tamper controls.

CPU tests distinguish equal contents with different physical capacities, isolate
logical and physical headroom, and count observation placement. They do not prove
physical capacity/address, push nonallocation, allocator/unwind behavior or
quiescence authenticity. Truncated-storage fixtures establish particular early
errors, not universal raw safety outside the formal storage-shape domain.

The benchmark uses the same owner for both variants, with precomputed O(k) reset
outside timing. It compares complete state/results, retained custody, lookup counts
and storage identities before timing. Measurements include test-only lookup
instrumentation, timer and dispatch overhead on a non-exclusive CPU. They are not
GPU performance or an uninstrumented production speedup claim.

Replay from a checkout containing the signed source commit in `SOURCE`:

```sh
python3 docs/evidence/dev-stable-release-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-stable-release-2026-09-22 --selftest
```

For a fresh qualification, run `check.py --repo ... --verus ... --output ...`, then
`cargo-checks.py --repo ... --output ... --target ...` into new owned directories.
Probe mode is development-only and is rejected by the offline auditor.
Results and all observations, including regressions, are retained in `RESULTS.md`.
