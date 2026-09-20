# Lower XGMI Currentness Diagnostic CPU Qualification

This packet qualifies the opt-in lower-level currentness timing API. It does
not qualify runtime capture, benchmark CLI integration, native execution,
performance parity, or formal refinement. No MI300X files or processes are
created by this CPU protocol.

`python3 -I -B cpu.py --record-target <owned-target>` records 15 sequential
commands: selected source map, Rust/Cargo versions, GNU and musl all-feature
and GNU feature-off test rosters and executions, unsafe-source inventory
roster/execution, verifier calibration tests, scoped formatting, warnings-denied
KFD/runtime library Clippy, and the closing source map.

Each configuration runs the same 198 exactly pinned tests: topology,
currentness, diagnostic timers, pair-session guards, device admission, and
SDMA owner/batch tests. Twelve new tests cover enabled/disabled differential
behavior, complete-host discovery, errors, boxed panics, latch sequencing,
clock inversion, missing intervals, arithmetic overflow, and nested timing
containment. Public diagnostic signatures are compile-checked only with the
feature enabled. Existing SDMA tests do not constitute native execution of
the new public batch APIs.

The helper reuses the thread-owned local build cache
`/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target`.
All builds explicitly set two jobs, disable incremental compilation, and use
optimization level 1 with debug assertions and overflow checks enabled.
GNU/musl/feature-off tests have explicit target triples. There are no selected
test skips; the unsafe inventory retains its one exact maintenance-only ignore.

Authenticated existing helpers provide ordinary-file source selection,
process-group supervision, ordered command receipts, stream hashes, strict
JSON parsing, and exact roster/result validation. Before/after source maps
must be byte-identical. Tool hashes include this protocol, the verifier and
its tests, and the design document. The packet records source files and
hashes, commands, toolchain observations, test rosters, and false native,
performance-acceptance, and formal-refinement flags.

Every command must complete successfully and leave no live owned process
group. Exact archive inventory and `SHA256SUMS` reject omitted, extra,
symlinked, or changed files. Replay with
`python3 -I -B docs/evidence/dev-xgmi-currentness-diagnostic-cpu-2026-09-20/cpu.py --live`
from the source root checks the sealed packet against current source bytes;
omit `--live` for historical replay.

The packet verifies test and instrumentation evidence. It does not prove an
atomic Linux/driver snapshot, machine-code refinement, or performance gains.
