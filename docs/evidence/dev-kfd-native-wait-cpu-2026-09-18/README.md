# Persistent-Window Wait Diagnostic: CPU Qualification

Status: CPU qualification complete; no native result is contained in this archive.

Base revision: `c6310ac6369a95e2e836444f19b347f26054193a`.
The final source roster and executable hashes bind the candidate independently of
this base revision. The enclosing signed commit publishes that candidate.

## Scope

The opt-in `hardware-diagnostic` feature adds two closed persistent-window wait
policies: a 1 ms or 25 us maximum requested sleep, both retaining the existing
50 us active-spin floor and observation-before-deadline rule. Ordinary waits
retain their policy and do not collect the new counters or CPU observations.

The shared queue transition preserves ticket identity, currentness, timeout
custody, and terminal-error handling. Runtime observations are appended to a
preallocated bounded recorder only after successful retirement/restoration,
including a completed window whose logical copy still needs an explicit flush.
Unprofiled native SDMA settlements invalidate a configured capture. Extraction
requires both logical and native teardown and consumes the complete capture once.

The directional-window benchmark admits `diagnostic-native-sleep1ms` and
`diagnostic-native-sleep25us` only with this feature and exactly 256 MiB. Both use
the full remaining outer deadline for each ordinary `RuntimeContext::wait` call.
Before writing the separate `fe2o3.kfd-directional-native-wait-diagnostic.v1`
schema, it requires two waits/flushes and exact 63+2 packet windows in each
direction, checks timing and counter decompositions, and validates the returned
buffer after every warmup and sample. Output follows explicit teardown.

Counters describe full scans, packet observations, pauses, and requested sleep,
not actual scheduler sleep or physical DMA timing. Thread CPU/context-switch
measurements explicitly distinguish available, unavailable, and invalid data.
Instrumentation adds overhead. These observations convey no completion authority.

## Verification Boundaries

Final GNU and musl runtime suites each passed 1,067 tests with 17 hardware tests
ignored. Their complete test/status rosters match. The diagnostic example passed
17 tests on each target; the default-feature example passed 14 tests. Final KFD
SDMA/wait filters passed 151 and 26 tests, respectively, covering 160 distinct
tests after accounting for overlap. The six existing R39 model tests and five
unsafe-source tests passed (one inventory-maintenance test ignored). Clippy with
all targets/features and denied warnings, plus workspace formatting, passed.

The final verifier consumed nine complete harnesses, checked 5,532 source files
and eight executables, and rejected all 11 corrupted-harness mutations. Earlier
development passes are preserved but not added to these final counts.

The scripted runtime tests cover timeout custody/index restoration, retirement
failure, missing/unprofiled observations, preallocated recorder capacity,
lifecycle/error precedence, and explicit-flush continuation. The small scripted
windows do not stand in for native 63+2 execution. Benchmark fixtures cover both
policies, exact identities, malformed captures, CPU statuses, and output failures.

The existing R39 model tests exercise the existing abstract policy only. No new
solver execution or Rust/native refinement proof is claimed. The frozen base
SDMA manifest is unchanged; the feature has a separate diagnostic sidecar.

No native throughput, copy-engine equivalence, HIP/HSA parity, or milestone
completion follows from this CPU archive. Native matching and performance
qualification remain required. No remote workload or temporary directory was
created for this CPU qualification.

## Preserved Attempts

- `gnu-example` failed to compile because a test used a private module path;
  the test now uses the public crate-root export.
- `gnu-kfd` and `gnu-kfd-serial` stopped without complete harness/exit receipts.
  The interruption notes retain the observed session outcomes; neither is a pass.
  The focused final KFD filters must not be described as a complete KFD suite.
- `gnu-runtime` and `mixed-settlement-debug` failed at a new test's missing
  explicit flush before its second copy wait. Correcting the fixture preserved
  the existing runtime progress contract; the diagnostic implementation was not
  changed to make waiting publish work.
- `clippy` and `clippy-fixed` caught nested conditionals and item order. The final
  lint run retains these fixes and does not suppress the warnings.

Earlier source captures are exploratory snapshots, not the final pre-build
boundary. The final chain starts at `source-qualified-before-v2`, then builds and
tests the candidate, hashes the executables, repeats the full GNU runtime suite,
rehashes them, and captures `source-after`. Other executable hashes are post-run
identity observations; only the final GNU runtime run is bracketed by both hash
captures. `verify.py` checks complete harnesses, exact commands, ordered receipts,
full source/binary rosters, unchanged identities, and corrupted-harness rejection.

Commands and failed/incomplete observations are retained without overwrite.
The `qualify-*.sh` scripts preserve the development sequence; the final invocation
uses `qualify-sealed.sh`. `seal.sh` only succeeds after the final verifier passes.
