# R126 Initial Primary Binding: Development Receipt

R125 remains accepted at the local CPU/test boundary. This packet implements
initial ordinary dispatch binding on an existing bootstrap primary and qualifies
bounded native success workflows. It does not accept R126, close A1/A2 or #182,
prove executable/formal correspondence, or establish HIP/HSA parity or speedup.

## Source

Base: `d5cb4d3b375c5ce27b682541357335aa33389df4`.
`source-published.patch` is the complete six-file source delta with zero context;
apply with `git apply --unidiff-zero`. SHA-256:
`32d3f36afcfc3cb9cdbeaf455b5eeebad58ab99516519b721897fafb777148eb`.
Documentation is not part of that source patch. `SHA256SUMS` binds the archived
patches, raw logs, toolchain and executable-hash records; no binaries are checked in.
`source-first-native.patch` identifies the initial complete GNU run with the
oversized-input assertion failure. `source-review.patch` identifies the later
prefix/model assertions and preliminary native run. `final-source.patch` is the
preallocated two-stream/type-alias source before the import-only formatting fix;
the authoritative final source is `source-published.patch`.

The new KFD module forbids unsafe code. It admits only a fresh ordinary primary
with its genuine model foundation and original platform owners. It rejects
detached generations (including `Some(0)`), insertion cursors, persistent
attachments, outstanding completion/dependency state and terminal state. It does
not fabricate a recycled or pristine-abort predecessor. Live SDMA data is allowed.

The initializer returns one move-only owner at a time into a pre-reserved root.
Preparation/native owners and the original parent survive admitted failure/panic.
Initializer captures stay retained until their guarded destruction before
validate/install; the destructor-panic case does not retain destroyed captures. Model
retake errors take precedence over ordinary operation errors; the original
operation panic takes precedence over secondary retake failures. Runtime lane
assignment follows successful binding, not just entry. Other materialization and
rebind routes retain their existing contracts and gaps listed below.

## CPU Evidence

GNU and musl builds use `cargo test --locked --offline -p fe2o3-kfd
-p fe2o3-runtime --all-features --lib --no-run`; musl adds
`--target x86_64-unknown-linux-musl`. Frozen executables prevent later builds from
replacing libtest binaries that spawn `current_exe()`.

| Check | GNU | Musl |
| --- | --- | --- |
| Runtime library, four test threads | 750 passed, 3 ignored | 750 passed, 3 ignored |
| Full KFD library, sixteen test threads | 1,314 passed | 1,314 passed |

All four final library runs exited 0. GNU/musl runtime durations are
25.63s/26.09s; KFD durations are 1,020.52s/1,239.94s. These CPU test durations
include fixture/model fault matrices and are not runtime performance measurements.

The six initial-binding tests exercise 97 cases across constructed-primary
success with/without SDMA, first/middle/last initialized prefixes, 31 preparation
stages with errors and panics, model/validation boundaries, retake precedence,
real loan-generation exhaustion/reclaim regression, initializer-destructor
panic, five nonfresh ledgers and an oversized count. Assertions cover exact
errors/panic payloads, original primary/platform/signal identities, authenticated
model placement and loan generation, exact host/device charges, complete native
owner partitions, inert terminal retries and absence of queue creation/publication
or destruction during binding. Current-item native initialization failures remain
covered by lower primitive suites, not a new integrated injected matrix here.

The final import-formatting change affects only the KFD test module. The rebuilt
GNU/musl runtime binaries are byte-identical to the frozen binaries used for their
complete CPU and native runs (`runtime-identity.log`). Native tests remain ignored
locally and are explicitly invoked below. No full-workspace, compiled-negative,
checker-calibration or authenticated Verus campaign is claimed by this receipt.
Strict Clippy passes for both crates with `--all-features --all-targets --
-D warnings`, formatting passes, and the unchanged unsafe-source policy passes
five tests with its inventory-refresh maintenance test ignored.
The default community configuration also compiles with `cargo check --locked
--offline -p fe2o3-kfd -p fe2o3-runtime --no-default-features`.

## Native Success

Stripped musl binary SHA-256:
`620e448da2983d1a131e52d8fbe82a516bfbd7bb43cac50fb9a49849da386a8e`.
The independently computed uploaded hash matches. Each test ran in a separate
process on shared MI300X, selecting only GPU 1, unique ID `ab83d2ffef0d3cdf`:

```sh
ssh mi300x prlimit --core=0 -- \
  env FE2O3_TEST_NATIVE_UNIQUE_ID=ab83d2ffef0d3cdf \
  timeout --signal=TERM --kill-after=10s 120s \
  /tmp/fe2o3-r126-primary.tOhOZ1/runtime-native-qualified \
  --exact <test-name> --ignored --nocapture --test-threads=1
```

All names have prefix `kfd_backend::retained_release_tests::`:

- `native_runtime_allocation_shutdown_selects_retained_directional_release`:
  public 4 KiB allocation/pool return and packetless retained teardown pass.
- `native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary`:
  exact admitted vecadd executes on primary ordinal 0 with zero auxiliary lanes.
  Three 4 MiB inputs/output are compared byte-for-byte; the exact 22-event trace
  matches allocation/read identities and one stream/queue lifecycle. Live backing
  is 25,698,304 bytes/9 records under the 64 MiB/128-record budget.
- `native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds`:
  two streams with preallocated distinct buffers execute on primary 0 and auxiliary
  1. Both are launched/flushed before explicit waits. Six full-buffer comparisons
  and 41 events match two distinct streams, queues and dispatch lifecycles. Live
  backing is 51,388,416 bytes/16 records under the same budget.

All three observe 532,480 bytes/3 records before retained-primary cleanup and zero
after settlement, with unchanged budgets, zero reserved/retained/quarantined
records and no poison. Repeated shutdown is inert; completed-root and backend
Drop finish. Each vecadd output SHA-256 is
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
Profile coverage reports zero dropped events; expected rosters are checked
independently. Host durations and profile host timings are not matched performance
measurements, and these tests do not establish simultaneous physical execution.
Queue events describe the runtime's assigned compute-lane lifecycle. The SDMA
bootstrap primary existed before its first compute-lane assignment; this public
profile is not a trace of every native queue-creation ioctl.

## Retained Failures And Review

- The earlier complete GNU KFD run has 1,313 passes and one failed oversized-input
  assertion. It expected no trace but the borrowed preflight records
  `release-validate-owners`. The final test permits exactly this read-only check
  and still requires unchanged memory observations and no initialization.
- The preliminary musl runtime run under competing large KFD matrices has 749
  passes and one `blocked_request_write_obeys_absolute_deadline_and_reaps_the_worker`
  scheduler-tolerance failure. The final four-thread full runtime run passes all
  750 tests without changing that worker test or production deadline code. This
  rerun is separate evidence, not promotion of the failed run.
- The first native two-stream experiment allocates the second stream's buffers
  after launching the first. It rejects with Busy: `cannot change native SDMA
  ownership while compute is pending`; panic during teardown ends the isolated
  process. The passing test preallocates both streams. Pending-compute allocation
  remains an explicit limitation, not a fixed behavior or successful fault cleanup.
- The first strict Clippy run rejects the nested preparation return type. The
  final module uses a named alias. The first formatting check then rejects only
  the long test import; the final two-line import is formatted.
- Two preliminary full KFD runs were deliberately terminated after the alias and
  test changes, before starting final-source regressions. They are not passes.

Read-only reviews found no blocking source defect in the new primary path. They
prompted exact model/account/error assertions and stream-attribution checks. The
primary performed edits, compilation, tests and native runs. Reviews are not
proofs and do not extend the test/native evidence boundaries.

## Cleanup

All native processes completed. An anchored query for this private directory's
two executable names returned no matches. Only those two uploaded files were
removed, then `rmdir` removed the empty directory; the absence check passed.
Pre/post ROCm observations show GPU 1 at zero utilization/VRAM percentage; GPU 0's
pre-existing 44% VRAM allocation was unchanged. No reset, service stop, host-wide
cleanup or native fault injection was used. These observations do not establish
an exclusive GPU reservation. See `cleanup.md` and the pre/post logs.

## Remaining Gaps

NEW/AUXILIARY materialization and REBOUND materialization still keep completed
owners in a callback-local vector that can be dropped on a later error/panic.
NEW retains the memory session on ordinary error, not panic. Same-shape resident
overwrite takes a roster that can also be lost on failure after partial writes.
These require rooted runtime custody, preserved error/panic semantics and exact
integrated failure tests. The new per-item primary initializer does not repair
those separate paths.

Native failure retention, additional queue profiles, pending-compute allocation,
general generated DATA-ADOPT/ISSUE/COMPLETE, Stop/drain/graphs, native depth,
aggregate budgets, formal correspondence and matched HIP/HSA performance remain
open. Neither one/two successful launches nor the CPU fault matrix substitutes
for those requirements.
