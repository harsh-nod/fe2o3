# Multi-GPU Async Lifecycle CPU Checks

## Source and Scope

Source base: `a400b2c796eb056efef24b18ff5faa2c5ba421bd`, signed and pushed to
both repositories before this increment. The raw packet retains the exact
source/guard patch, source hashes, commands, clean environments, toolchain,
test rosters, outputs and exit codes. Documentation is outside the source patch.

The persistent transfer now has a private, statically dispatched `TransferIo`
interface. Concrete native wrappers keep their existing public signatures and
forward to one shared transfer sequence. Synchronous and asynchronous callers
share the same mapping, publication and restoration steps. The SDMA owner's
submission method reuses the existing `SdmaSingleMemoryV1` interface, whose
native implementation forwards to the same memory facts and doorbell store.
These changes make the production algorithms composable in CPU tests; they
do not widen native route eligibility or change ownership policy.

## Composed KFD Coverage

The test fixture uses distinct model-only device, VM, session, PCI and render
identities, disjoint virtual-address ranges, genuine PUBLIC memory leases and
the existing memory engine. Logical copy size is 2,048 bytes; the original
physical allocations are independently 4,096 and 8,192 bytes. Existing fixture
defaults remain unchanged.

Seven composed test functions contain 85 loop cases: two async success cases
with accounting disabled/enabled, one synchronous success case, 32 native
mapping-prefix error/unwind cases, eight closing-map currentness cases, eight
publication cases, two polling cases and 32 model-retake boundary cases.
These are loop cases, not 85 independently listed test functions.

The tests execute production persistent detach/restore, actual paired
foundation loans and reclamation, the shared transfer sequence, memory-engine
map/unmap transitions, SDMA submission, and mapped-fence polling or waiting.
They compare the entire emitted packet to independently captured source and
destination addresses and the logical length, inspect control and actual local
doorbell bytes, and only then inject completion. Assertions track both original
authorities, retained accounting, generations, identities and restoration timing.

The memory and queue fixtures inject native-leaf outcomes and CPU-visible
completion. They do not execute GPU payload copying, concrete Linux route
admission/currentness, or the entire public two-session endpoint-preflight
wrapper. The scoped operational-currentness tests must not be represented as
qualification of full Linux route currentness or GPU memory visibility.

## Runtime Lifecycle Coverage

Six new tests compose the actual public Context, cooperative-copy ledger and
current-thread-owned engine with the existing scripted native-route transport:

- Expired public drain before and after publication retains custody; resumed
  success preserves exact owners and bytes and requires explicit event release
  before submission release.
- Opposite-direction copies use distinct source/destination allocations, retain
  the original drain future across deadline expiry, deliver callbacks once,
  retain both results and complete explicit owned shutdown.
- A disjoint successful pair and a cancelled pair produce a quiescent group
  with both successful and failed results, not an all-success report.
- Retirement uncertainty after disjoint success stops the engine and retains
  the exact submission/allocation inventory until process exit.
- Stop without drain, stop with pending drain and tick-budget exhaustion retain
  published roots rather than pretending shutdown released them.
- A consumer behind a pending ordinary native copy is rejected without losing
  custody. This records an unsupported pipeline, not dependency-driven parity.

Successful cleanup exercises scripted zero-upload, demotion and recycling
without clearing native flags to bypass release. Scripted transport still does
not increment the native completion counter. These are CPU scheduler and
ownership tests, not hardware overlap, payload-transfer or shutdown evidence.

## Native Witness

The two-GPU `gfx942-runtime-compute-xgmi-smoke` now checks expired public-drain
rejection, then drives its unflushed native copy through `RuntimeContextV1::drain`.
Existing pre-flush observers, exact four launches, destination sentinel,
unchanged source, 13 full-buffer readbacks, native completion count and explicit
logical/native cleanup remain required. PASS adds `expired_drain=rejected` and
`copy_progress=explicit-drain`. It remains a correctness witness, not a benchmark.

The fresh read-only SSH attempt at `2026-10-02T01:47:50Z` exits 255 because
`sharkmi300x-1` does not resolve. No remote command executes, no pair is admitted,
no workload runs and no scratch is created. Exact records are under `admission/`.

## Retained Tooling Attempts

All earlier checks and source preimages remain in the packet:

- `attempt-01/checks`: strict Clippy rejects five diagnostics from two test API
  mismatches. The new Context fixture now unwraps the checked submission lookup
  and calls `record_event` with its submission only. No production code changes.
- `attempt-02/checks`: strict Clippy rejects a nonexistent device-facts address
  getter in a new test. The fixture now uses the existing checked logical
  subrange API; no raw-address accessor or native admission bypass was added.
- `attempt-03/checks`: strict Clippy rejects two indexed assertion loops. They
  now enumerate the physical-extent array with unchanged assertions.
- `attempt-04/checks`: all six stages exit zero, including strict combined
  all-feature library/test/example Clippy in 38.73 seconds, the no-default
  runtime check, two example tests, runnable witness build, formatting and
  whitespace checks. The no-default check retains the existing feature-specific
  `Route::Native` dead-code warning.

The runnable witness is built but has not executed on GPUs. The final unit
rebuild follows the tooling packet, so its recorded intermediate unit ELF
identities must not be substituted for the final test before/after hashes.

## Final Unit Results

The combined all-feature unit rebuild exits zero in 10 minutes 20 seconds.
`attempt-01/build/elf.sha256` binds the final executables. Each direct test
packet verifies those hashes before and after execution. All commands use the
recorded clean environment, locked offline dependencies, the pinned nightly,
optimized tests with debug and overflow assertions, and one test thread.

| Final-source check | Result |
| --- | --- |
| KFD `compute_xgmi` | 47 passed |
| KFD `xgmi` | 115 passed |
| KFD `shared_memory::tests` | 329 passed |
| KFD `sdma::tests` | 114 passed |
| KFD `model_pair_loan` | 7 passed |
| KFD `initialized` | 63 passed |
| KFD `public_sdma` | 6 passed |
| KFD constructed-allocation regression | 1 passed |
| Runtime `compute_xgmi` | 23 passed, including all six new lifecycle tests |
| Runtime `multi_group_drain_tests` | 7 passed |
| Full runtime | 1,952 passed, 3 failed, 32 ignored; zero filtered |

The KFD filters overlap and are not additive unique counts or a full-suite
result. All KFD lists and runs exit zero without ignored tests. Runtime lists
and focused runs exit zero. The full runtime exits 101 in 89.25 seconds; the
three failures are exactly the existing authorized-execution telemetry tests
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each reports
`InspectSocket` with `EPERM` at the unchanged `authorized_execution.rs:1317`.
This is not a full runtime pass. All six new lifecycle tests also pass in that
unfiltered run. No tests were removed or ignored to obtain these results.

Final unit SHA-256 identities:

```text
KFD     0fd360bfa015330c54c57742ed64f53288cf1cd361fa28fbbe097c09f64ae848
runtime 16fbf25ba33e1f92cfd6f109012fa20355bd9eb115fdf6de32e3fdaa8ea07b14
```

## Source Controls

The exact existing 32-command source-control workflow exits zero. Every command
exits zero and its owned process group is absent afterward. Before/after audits
confirm the same source inventory. The reviewed metadata proposal changes only
19 SHA literals and seven source-roster counts across 12 guard files; runtime
source cardinality changes from 335 to 336, or 340 to 341 with the five schemas.
All 76 executable proof files, predicates, contracts, expected solver results
and mutant counts remain unchanged. The cadence scan/currentness function
slices remain byte-identical. Independent proposal auditing checks all 1,014
recorded source hashes.

The packet retains the initial independent-audit failure caused by a missing
`jq` in its clean PATH, the corrected audit, and a rejected numeric patch-header
application that changed no files. Root normalized only hunk-header syntax;
the post-application audit confirms exact equality with the proposal. These
source-identity checks do not execute a solver or verify the new native route.

## Remaining Gates

Hardware execution of the complete-output two-GPU pipeline, full concrete
Linux/public-adapter fault composition, outstanding native group qualification,
pending ordinary native-copy-to-compute admission, persistent peer mappings,
multi-packet copies, broader workload partitioning and matched scaling remain
open. A3 is not complete. No new machine-code refinement, formal verification,
HIP/HSA parity or performance improvement is established by CPU tests or source
identity controls.

## Raw Packet

`SHA256SUMS` identifies `raw.tar.gz`. It preserves initial and corrected attempts
separately, source-control inputs and receipts, scripts and exact source hashes.
Executables and Cargo caches are not bundled. Replay requires the recorded
toolchain and locked dependencies; source is recoverable from the base plus
`source.patch`. `final-attempts.txt` identifies accepted final-source runs and
`final-elf.sha256` binds the final unit executables and runnable witness.
