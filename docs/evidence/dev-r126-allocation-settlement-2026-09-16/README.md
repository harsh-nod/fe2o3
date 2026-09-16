# R126 Allocation-Specific Settlement: Development Receipt

This packet extends `c82abce624f5bdfc7198d9a01d76936e81375117`.
R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.

`source-files.sha256` identifies all eight changed Rust files. `source.patch`
contains the complete source/test delta, SHA-256
`4eaddeb2217ff229c43dea1118cce56e85cfe18d3a2d707586ce792276d7d06d`.

## Implementation

The additive `RuntimeBackendV1::allocate_with_outcome_v1` defaults to the
existing allocation method without strengthening any failure. The explicit
`SettledNoOwner` outcome asserts that this requested allocation attempt left no
owner or pending allocation root, settled model/currentness, consumed no prior
logical allocation owner, and left the backend live. Queue infrastructure and
internal pool/model bookkeeping may remain or advance.

Context consumes only this attempt's unattached retained token through the
existing `release_after_disposal` transition, then returns its original error as
`BackendQuiescent`. It does not format, clone or replace the diagnostic. Generic
quiescence still quarantines credits, including failures with no public handle.
Backend panic before the explicit outcome preserves the original panic payload
and configured Context quarantine. Credit invariant failure seals Context.

Direct KFD emits the outcome only for cold typed backing-capacity rejection
from `allocate_sdma_owner_v1`, whose lower `RetryableCapacity` disposition
requires successful model retake and settled empty custody. Queue creation and
subsequent initialization, promotion and hidden-cleanup failures cannot emit it.
Warm capacity remains pre-mutation rejection. No message classification, native
allocation algorithm or unsafe code is introduced.

The multi-device router forwards settlement without inserting a route, retaining
the existing consumed-ID ordering. Direct legacy allocation APIs erase the new
outcome back to `Quiescent`. Worker V1/V4/V5 retain their wire formats and legacy
allocation dispatch, so no settlement authority crosses those protocols.

## Test Scope

Nine new tests cover:

- Exact boxed diagnostic identity, no Display/Debug formatting, one drop,
  repeated refunds, successful retry/release, neighboring owners and devices.
- Original boxed panic identity and exact configured quarantine/terminal retry.
- Two-child direct routing, child/route ID consumption, device isolation and
  legacy erasure for both HostVisible and DeviceLocal allocations.
- A later zero-initialization failure with successful hidden-owner cleanup that
  remains generic `Quiescent` and therefore cannot refund Context credits.
- Canonical V1/V4/V5 dispatcher erasure and real Python child-process proxy
  accounting, exact request order, retained quarantine through failed shutdown,
  continued stream operation and Linux child reaping on transport Drop.

The existing warm/cold Context matrix now requires exact refund and successful
retry with and without configured admission. The unchanged default mock still
creates a hidden owner before generic Quiescent/Terminal failure; its regression
now explicitly asserts that owner's bytes, rather than accepting a vacuous
empty-index test. Invalid handles and other existing failure regressions remain.

Scripted cold admission is not native queue-creation evidence. The two-child
fixture proves runtime/router/account forwarding of an injected typed
disposition; constructed lower tests separately exercise settlement authority.
Fixture disarming is not native cleanup. Worker tests follow the existing
Python-availability skip policy; the campaign explicitly requires Python first.

## Final CPU Results

| Target | All-Feature Runtime Library Suite | Failed | Ignored | Libtest Duration |
| --- | ---: | ---: | ---: | ---: |
| GNU | 840 passed | 0 | 8 | 26.74 s |
| musl | 840 passed | 0 | 8 | 26.89 s |

Both targets also pass all 15 selected constructed lower allocation tests.
These exercise typed capacity, denied settlement witnesses, validation/loan
rejection, model retake, native-operation fault injection and retained owner
partitions. They are CPU fault fixtures, not hardware execution or a full KFD
suite. All eight ignored runtime tests require isolated native hardware.
Test durations are not benchmarks.

Strict all-feature/all-target KFD/runtime Clippy, formatting and runtime
no-default-feature compilation pass. The resource-accounting library passes
23 tests, including unattached retained-token disposal and exact conservation.
The unsafe-source policy passes five tests with its explicit maintenance test
ignored. GNU/musl rosters are identical (848 names). Both source checks pass;
Cargo JSON identities and all four test-executable hashes are retained.
The separate all-target HSA/simulator API-compatibility check also passes with
the HSA qualification adapter enabled and native HSA disabled.

`preliminary/` preserves the initial module-path compile failure, two earlier
focused runs (41 and 45 passed, each with one hardware test ignored), and the
first campaign's strict-Clippy rejection. The failed campaign includes its own
source manifest and patch. The preliminary focused logs do not authenticate
executable/source identities and are not substituted for final-source results.

## Qualification Boundaries

No GPU job, upload or remote scratch was created. A read-only MI300X inspection
found GPU 0 holding about 91.55 GB for another process despite zero utilization,
and GPUs 1-7 occupied by active jobs. It found no additional disposable temporary
directory confidently attributable to this runtime campaign; other compiler
campaigns and their worktree caches were left untouched.

Native cold-admission/failure qualification, pending-compute probes, formal
implementation correspondence, broader allocation-failure recovery, negotiated
Worker settlement support, aggregate memory and matched HIP/HSA benchmarks
remain open. This receipt does not advance milestone acceptance.

## Reproduction

`bash verify-allocation-settlement.sh CHECKOUT OUTPUT_DIRECTORY` requires an
existing output directory, locked offline dependencies, GNU/musl Rust targets,
Python, `jq` and GNU `prlimit`. It checks source identities before/after,
records command/timestamp/status files, identifies test executables from Cargo
JSON and records binary hashes. This is a CPU development campaign, not the
closed full-workspace qualification runner, a full KFD suite or a benchmark.
No executable binaries are archived. `SHA256SUMS` covers the other archive files.

`bash verify-downstream.sh CHECKOUT OUTPUT_DIRECTORY` separately checks the
HSA and simulator adapters on all targets. It explicitly enables
`qualification-legacy-hsa-runtime`, so it checks the HSA adapter rather than
the default inert marker. This does not build or execute the native HSA backend.
