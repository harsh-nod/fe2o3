# Runtime XGMI Currentness Capture CPU Qualification

This packet qualifies runtime recorder and benchmark CLI integration for the
opt-in full-currentness timing API. It does not qualify native execution,
performance parity, or formal refinement. It creates no MI300X resources.

`python3 -I -B cpu.py --record-target <owned-target>` records 22 sequential
commands: before-source map, Rust/Cargo versions, GNU/musl all-feature and GNU
feature-off runtime and example rosters/executions, unsafe-source inventory
roster/execution, five verifier calibration tests, scoped formatting,
warnings-denied all-feature and feature-off Clippy, and closing source map.

Each diagnostic-enabled configuration runs 58 selected runtime tests and 13
example tests. Feature-off runs the exact remaining 33 runtime and 12 example
tests. Eleven new runtime tests cover both bounded storage modes, admission,
nonmutating failure, every nested missing interval, overflow, containment,
wrong-mode extraction, teardown precedence, and typed adapter equivalence,
custody, absolute deadlines, original panic identity, and unavailable detail.
Three new example tests cover feature gating, exclusive flags, mode labels,
and exact formatting with distinct opening/closing sentinels. Existing old
mode tests remain selected. No selected test is ignored; the unsafe inventory
retains its one exact maintenance-only ignore.

The recorder reuses the thread-owned local target
`/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target`. Builds explicitly
use two jobs, no incremental compilation, optimization level 1, and enabled
debug assertions and overflow checks. Test triples are explicit.

Authenticated existing helpers supply source selection, process-group
supervision, ordered receipts, stream hashes, strict JSON, and exact test
roster/result checks. Source maps before/after must be byte-identical. Tool
hashes bind this protocol, verifier/tests, and the successor design document.
Every stage must finish successfully, emit no compiler warning, and leave no
live owned process group. Exact archive closure and `SHA256SUMS` reject
omitted, extra, symlinked, or changed files.

Replay from the source root:
`python3 -I -B docs/evidence/dev-xgmi-currentness-capture-cpu-2026-09-20/cpu.py --live`.
Omit `--live` for historical replay. This packet does not modify previous
sealed evidence and keeps native execution, performance acceptance, and
formal refinement explicitly false. Native measurement remains pending.
