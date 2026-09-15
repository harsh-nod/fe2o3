# R118 Mutation Framework Checkpoint

2026-09-14. Reviewed source remains
`df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb`
with 5,692 identities above R117 `a07ec44309e214f2a8ef0e687e610c8e60a36224`.
GNU/musl each pass 2,780/0/5 across 48 libtest targets and one CSV benchmark.
Only runtime changes from 715 to 733 tests; all other per-target rosters match
committed R117. Both prerequisites were independently reviewed.

## Executed And Immutable

- `r118-mutations-c1-v1.js`: 40 executions, 36 distinct sources.
- `r118-mutations-c2-v1.js`: 19 production executions.
- `r118-mutations-c3-v1.js`: 16 production, one two-file combined-defense,
  two helper-calibration executions.
- `r118-mutation-plan-v1.js`: aggregate 78 executions, 74 complete maps,
  eighteen selected tests, four explicitly shared C1 source pairs.
- `r118-mutation-core-v1.js`: scoped patch derivation, complete-map overlay,
  application-attempt tracking, manifest-bound recovery and aggregate restoration.
- `r118-mutation-core-tests-v1.js`: 48 passing tests, only in-memory writes;
  actual source/plan preflight is read-only.
- `r118-run-v1.js`: unchanged recorded process runner.

Seven hashes are pinned in `r118-mutation-core-inputs-v1.json`, SHA-256
`9afd082517e9ffd1669d78d61ab660fdf320554fb1b3b39377d53f0ab60af600`.
Do not overwrite these executed helpers; use new versioned files if corrections
are needed and retain the original records and pins.

The latest closed record is `r118-mutation-core-contracts-v1.json`, SHA-256
`6af8e092050a6f4ba31a0f05d8bbc2ba3728ca6bc412653f77d649cfd56d725e`.
Its predecessor is `r118-reviewed-musl-all.json`, SHA-256
`91ff92dbc70b970a6438789c7faa076149794cb133543751b93595226149040c`.
Core record/log/maps and helper hashes passed independent review. No job remains
live from these runs. No SSH, native GPU or solver execution was added.

## Integration Still Required

The full qualification plan, historical validator, freeze, runner contracts,
campaign runner, collector and archive are not implemented for R118 yet.
`r118-source-gate.py` and `r118-auxiliary-gates.py` are unexecuted namespace-only
copies of the corresponding R117 helpers. They require a later frozen map.
No real Rust mutation has been applied, compiled or executed.

Use the frozen core consistently in execution, expected-map derivation,
grouping, collection and recovery. Patches are arrays of distinct paths, each
with exact edit pairs and an optional validated method scope. Derive every
patch before writing any source. Put all application writes inside try/finally.
Pass the real attempt-prefix array through failure handling, and set
`runnerAttempted` before invoking any child runner. A missing terminal record
after invocation means unknown closure, not a never-spawned child.

Keep physical recovery separate from acceptance. Normal qualification requires
a checked exact failing test and oracle, complete planned mutation map,
`restored`, `quiescent`, `all_mutant_before_restore`, no external edit, no error,
and a freshly equal full source map. A fully restored file set followed by an
I/O exception is not an accepted chain entry. Authenticated recovery must retain
the original runner-attempt state and independently prove process closure.
Hash checks observe endpoints, not an atomic exclusion of concurrent writers.

The three pre-execution review bugs were corrected before helper testing:
unattempted files cannot be reclaimed merely because their bytes equal a
planned mutant; present falsy scopes reject instead of becoming whole-file
edits; failed or malformed process scans leave quiescence unproven. Tests also
compose partial second-file writes, mixed-state recovery retries and complete
restoration followed by an I/O error.

Preserve three original candidate chains (8/5/6 runs), the eight preliminary
integrated records and their distinct maps, both full prerequisites, and the
helper-contract record. The three reviewed format/runtime/Clippy prerequisites
are references to existing preliminary records, not duplicate artifacts.
C1 retains its 381 sparse index descriptors; do not fabricate source hashes.
See `r118-isolated-history-handoff.md` and the original negative handoffs.
