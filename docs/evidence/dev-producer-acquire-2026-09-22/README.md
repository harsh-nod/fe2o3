# Actual Producer-Read Acquisition

Development evidence for actual-type producer admission and combined read capacity.
This packet does not authorize native pending consumers or establish HIP/HSA parity.

Production and proof share retained counts, combined budget, capacity validators,
header, ordered admission and sequential commit into the public mutable slice.
Paired actual/historical acquisition executes independently under represented
producer custody and preserves exact results, complete owner state and output.
A separate paired harness executes producer capacity validation.

The raw actual domain is path-sensitive: safe budget arithmetic is required only
after the header prefix passes, and count storage covers allocation storage only
after the full header passes. Roster length fits u64. No desired-success, selected
uniqueness or issuer-valid producer-ID premise is introduced. This count-storage
bound is sufficient, not minimal for every reached path. Duplicate free destinations
retain sequential overwrite semantics; their raw witness does not claim custody.

Synthetic witnesses cover live acquisition with retained custody, late rejection,
occupied-output replay, raw aliases, producer ID zero, malformed prefix errors and
unequal arena arithmetic. The fixture mirrors a logical constructor and synthetic
Pending metadata, not actual fallible constructor or Begin reachability. Concrete
CPU early allocation/device errors with missing counts exceed the formal domain's
conservative count-storage scope.

`check.py` brackets thirty-three controls with two whole-root runs, authenticates
the pinned Verus closure, and records exact diagnostics and terminal receipts.
It stages a 64-file include closure and ten executable macro policy projections.
`cargo-checks.py` records formatting, full model tests, Clippy, release compilation
and instrumented frozen-baseline measurements. `performance.py` authenticates
six frozen producer methods, the stable count and older independent capacity leaf,
unchanged dependencies, reviewed adapters and 43 historical declarations.
Its parser checks 1,008 ordered rows across 72 cases. `audit.py` reconstructs
inputs from Git objects and validates exact receipts, serialization and tamper controls.

The generation adapter uses two unchanged immutable Deref implementations in Rust
and an explicit journal field path in proof. This is source-authenticated, not a new
proof of trait dispatch. Producer release, outer stable wrappers, physical storage,
allocator/unwind, construction/reachability and native authority remain open.

CPU fixtures retain producer and stable reads, comparing exact results, snapshots,
output, access counts and storage identities. Timing includes test-only counters,
per-call timer and dispatch. The same owner's O(k) restoration, counter resets,
assertions and storage checks are outside timing. These non-exclusive CPU
observations are not uninstrumented production latency, GPU performance or parity.

Replay from a checkout containing the signed source commit in `SOURCE`:

```sh
python3 docs/evidence/dev-producer-acquire-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-producer-acquire-2026-09-22 --selftest
```

For fresh qualification, run `check.py --repo ... --verus ... --output ...`, then
`cargo-checks.py --repo ... --output ... --target ...` in new owned directories.
Probe mode is development-only and is rejected by the auditor.
`RESULTS.md` retains every benchmark case, including regressions.
