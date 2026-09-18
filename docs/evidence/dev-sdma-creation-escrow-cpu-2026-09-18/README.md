# SDMA Creation Escrow: CPU Development

Frozen CPU qualification passed for the exact source cohort below:

- GNU and musl: 227 selected KFD tests each, zero failures and zero ignored.
- GNU runtime: four selected retained-release smoke tests passed.
- Unsafe-source policy: five passed; one explicit baseline-refresh maintenance
  test remained ignored.
- Strict KFD/runtime Clippy, no-default-feature checks, formatting and diff checks
  passed. Before/after source inventories are identical.
- Seven archive calibration tests passed.

The complete receipts and source inventory are bound by `SHA256SUMS` and must be
checked with `verify.py`; the interrupted attempt is not a passing result.

The source cohort starts at `b1c8219ab38d10656f68f71d83e17297c38dd45e`
plus the exact per-file identities in `raw/source-before.log`.
The selector covers 5,559 Cargo, toolchain, source and release-contract inputs;
evidence and build products are excluded. Before/after inventories must match.

Ordinary generic, targeted, directional, striped, LogicalMux and combined
creation now borrow a preallocated escrow owned outside the facade's unwind
boundary. Terminal custody preserves intended profiles, counts/cursors, separate
primary/secondary prefixes, and an optional prepared native attempt. The ioctl
mutates arguments rooted in that attempt. A mapped doorbell is rooted before
closing currentness. Successful extraction occurs after creation-arm disarm.
The facade roots terminal custody before final poisoning and resumes the
original creator panic even when retake or final poisoning also panics.

The bounded host-resource roster is inline (maximum 16), avoiding a temporary
heap roster allocation. Some profile-specific public Contract diagnostic strings
are now shared; no exact-string compatibility is claimed. Typed dispositions
and the terminal memory boundary are unchanged. No new unsafe code is introduced.

New constructed tests cover every legal profile/count and every confirmed prefix,
with the retake/error/panic and final-poison cross-product at complete rosters and
combined primary/secondary boundaries. Nine native-driver fault cases run at each
profile's final attempted owner and the combined first-secondary boundary. They
compare exact mutable arguments, engine, allocation identities, record storage,
and doorbells. Successful native-driver transitions fill and extract every profile,
including original storage identities. A separate triple-panic case retains an
attempt with its mapped doorbell. All terminal forms remain excluded from
retained-release authority, and retry is inert. Tests use real local fixture
mappings/model/accounting and scripted native calls, not GPU calls.

Development exposed a fixture doorbell encoding error before the intended
mapping/currentness fault stages. That encoding and profile/group oracles were
corrected, and success/error-settlement coverage added after independent review.
The superseded developer test process was stopped after its failure; it is not
qualification evidence. Developer compilation also caught migrated enum patterns,
fixture argument construction and an oversized test observation. The frozen
qualification reran the corrected source.

The first frozen, unoptimized attempt is preserved in `interrupted/`: its first
six commands passed, then the GNU test process was intentionally stopped with
SIGTERM (Cargo exit 101). Its partial harness is not a qualification pass. The
accepted rerun uses test optimization level 1 with debug assertions and overflow
checks explicitly enabled. Source identities and the selected 227-test roster
remain unchanged. The verifier checks both attempts' exact commands, chronological
ordering, interruption status and unchanged source.

This is CPU development evidence, not R126 acceptance, full HIP/HSA parity,
real KFD/GPU fault injection, native constructor qualification or performance evidence.
Public XGMI creation unwind custody remains outside this packet. Memory operations
before complete queue preparation remain opaque; arbitrary consuming-memory
unwind refinement is not established. Formal Rust/native correspondence and
submitted-work qualification remain open. Historical native archives qualify only
their historical source. No SSH work or remote cleanup was needed for this packet.

`verify.py` checks exact named rosters/outcomes, commands, statuses, chronology,
source inventory equality and sealed membership. It uses only SHA-pinned prior
receipt/harness helpers and never re-executes qualification commands. Seven
calibration tests cover malformed rosters/outcomes, seal handling, archive membership,
symlinks, commands, statuses and timestamps.

```sh
python3 -B docs/evidence/dev-sdma-creation-escrow-cpu-2026-09-18/verify.py
python3 -B docs/evidence/dev-sdma-creation-escrow-cpu-2026-09-18/verify.py --live
```

`--live` reruns only the pinned source selector and requires the current source
cohort to match. `qualify.sh` and `record.sh` refuse receipt overwrite or appending
to a sealed archive.
