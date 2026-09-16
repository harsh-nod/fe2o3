# R126 Runtime Materialization Custody: Development Receipt

R125 remains accepted at the CPU/test boundary. This packet fixes the legacy
runtime ownership-retention gaps and qualifies bounded native AUXILIARY budget
rejection. It does not accept R126, close A1/A2 or #182, prove executable/formal
correspondence, or establish HIP/HSA parity or a performance advantage.

## Source And Scope

Base: `067d692707bb1476464bcdbb6721f06de923e71d`.
`source.patch` is the complete four-file runtime source delta. SHA-256:
`58b80ed8524de3c2637c7c92c0e8512dd3fa7cca01d5eade0c7cddcce3e5a3c3`.
Documentation is separate. `SHA256SUMS` binds this receipt, source patches,
toolchain identities, executable hashes and raw logs. No binaries are committed.

- NEW/AUXILIARY and REBOUND materialization use a pre-reserved custody driver.
  It preserves the original specification vector, callback captures and every
  returned native owner on subsequent failure. Lower primitives retain owners
  they cannot return; the runtime does not duplicate their current-item authority.
- NEW installs its memory session before invoking the materializer. On success
  it transfers the session and complete data directly to the rooted native
  constructor; error and panic leave the session installed.
- Resident overwrite retains both original vectors before writes. Failure
  preserves the successful prefix, partially written current item and untouched
  suffix, without retry, rollback or content-hash promotion.
- Capture and metadata destruction occur inside the owning drivers' unwind
  guards before native output transfer. Already destroyed captures/metadata are
  not claimed to survive their own destructor panic. Exact original errors and
  panic payloads propagate after retention.
- The session-only helper protects the session, not arbitrary callback-created
  outputs. Its production callback delegates to the owning materializer.

The new generic module forbids unsafe code. Native owner retention follows the
existing fail-closed terminal policy; it is not recoverable resource release.

## CPU Validation

GNU and musl final runtime library suites each pass **764 tests**, with four
hardware-only tests ignored locally. Elapsed times are 23.99s and 26.08s. These
are test durations, not runtime performance measurements. The 14 new generic
tests cover zero/one/many success, first/middle/last errors and panics, lower/outer
owner partitions, exact panic identity, capture/metadata destructor panics,
deterministic capacity overflow, session handoff, unchanged vector storage,
roster mismatch and partial borrowed writes. CPU probes are not native authority.

The production materializer reserves one output vector on nonempty success and
none for empty input. The overwrite driver adds no allocations to the measured
success cases. Callback/native allocations are outside these counting fixtures.
Host initialization still borrows source bytes; DeviceLocal retains its existing
owned-copy/content-descriptor path. Source-wiring tests check both routes.

Commands:

```sh
cargo test --locked --offline -p fe2o3-runtime --all-features --lib --no-run
cargo test --locked --offline -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
# Copy each resulting libtest executable to a private, stable filename, then:
<frozen-runtime-binary> --test-threads=4
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-kfd -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

All final commands exited 0. The unchanged unsafe-source gate passes five tests
with its explicit inventory-refresh maintenance test ignored. Frozen binaries
protect tests that spawn `current_exe()` from concurrent build replacement.
KFD source did not change; this packet does not claim a fresh full KFD suite,
full-workspace suite, compiled-negative/calibration or authenticated Verus run.
The preceding receipt retains the earlier 1,314-test KFD GNU/musl qualification.

## Native Budget Rejection

Stripped musl SHA-256:
`843db14d89dc864bbc4f68cb5e454f9172bcdcf2a7d506ab578f89e339a9cc9d`.
The uploaded hash matches. All six native runs use this binary on MI300X GPU 1,
unique ID `ab83d2ffef0d3cdf`, one exact test per isolated process.

The new test is
`kfd_backend::retained_release_tests::native_runtime_auxiliary_budget_failure_retains_initialized_prefix`.
For each prefix `0`, `1`, `2`:

```sh
ssh mi300x prlimit --core=0 -- \
  env FE2O3_TEST_NATIVE_UNIQUE_ID=ab83d2ffef0d3cdf \
  FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=<prefix> \
  timeout --signal=TERM --kill-after=10s 120s \
  /tmp/fe2o3-r126-materialization.Ce4Wwm/runtime-native \
  --exact <test-name> --ignored --nocapture --test-threads=1
```

Six public 4 MiB HostVisible allocations are prepared before any dispatch. The
first exact-artifact vecadd is launched/flushed but not polled or waited, keeping
the primary logically occupied. The independent second stream therefore takes
the real AUXILIARY materializer, not an idle-primary rebind.

| Returned AUX Owners | Budget | Pre-Retake Charged Bytes | Records |
| --- | --- | --- | --- |
| 0 | 37 MiB | 38,281,216 | 12 |
| 1 | 41 MiB | 42,475,520 | 13 |
| 2 | 45 MiB | 46,669,824 | 14 |

All three pass. The next 4 MiB allocation rejects before reservation/native
allocation with the exact backing-credit `Capacity` error. The borrowed account
observation has zero reserved/quarantined records, all listed records retained,
an unchanged budget and no accounting poison. This observation precedes lower
terminal parent transport; unavailable accounting afterward is not a refund.

The observer borrows actual `Gfx942FixedDispatchDataV1` owners only after their
custody is placed in `ManuallyDrop`, so observer unwind cannot release them.
Assertions cover prefix count, three retained specifications, actual public
layouts, first execution ID/stream/allocation roster, second-stream pending
identity, no installed AUX lane, and exactly unchanged queue/publication records.
The context and backend become terminal; another public flush changes neither
observations nor profiler length. Public layouts and logical IDs are not native
storage identities. No KFD token is cloned or fabricated.

The whole terminal context is retained in `ManuallyDrop` until process exit,
including on assertion failure. These cases do not run ordinary shutdown or
claim terminal refunds, ioctl-failure recovery, physical overlap or first-kernel
output validation. They are genuine native initialized-prefix/budget-rejection
workflows, not injected GPU faults.

## Native Success Regression

These existing exact tests also pass, with prefix
`kfd_backend::retained_release_tests::`:

- `native_runtime_allocation_shutdown_selects_retained_directional_release`:
  public allocation/pool return and packetless teardown.
- `native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary`:
  primary ordinal 0, all three input/output readbacks, 22 observed profile events.
- `native_runtime_two_stream_dispatch_uses_primary_and_auxiliary_then_refunds`:
  preallocated primary 0/AUX 1, six full-buffer readbacks, 41 profile events.

Both vecadd outputs retain SHA-256
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
All success probes observe final primary account 532,480 bytes/3 records fall to
zero, with no poison or reserved/retained/quarantined charges. Repeated shutdown
is inert and backend Drop completes. These success claims do not extend to the
terminal budget-rejection cases.

## Development History And Review

The first full GNU run passed 763 tests and failed one source-inspection test:
its extraction stopped at the newly added `cfg(test)` observer before reaching
the initializer. The final test uses explicit function/module boundaries, keeps
the host-borrow/device-owned-copy assertions and adds initial-wrapper wiring
checks. `source-first-regression.patch` binds that failed full run; the corresponding
identity log retains its original temporary filename. The subsequent final GNU
and musl runs pass independently. The early 13-test focused log is exploratory,
not a separately frozen qualification artifact. No production behavior was
changed to make the source-inspection regression pass.

Two read-only reviewers found no remaining blocker in this packet. They corrected
the proposed native routing (waiting first would select REBOUND), established the
budget arithmetic and prompted exact execution/pending/profile assertions. The
primary performed all edits, builds, tests and SSH work. Review is not proof.

## Cleanup And Remaining Work

All six native processes completed. The anchored private-executable query returned
no matches (exit 1). Only the uploaded `runtime-native` file was removed; `rmdir`
then removed its private directory and the absence check passed. GPU 1 remained
at zero reported utilization/VRAM percentage afterward; GPU 0's existing 44%
VRAM allocation was unchanged. No reset, service stop, host-wide cleanup or other
user's files were involved. This is not an exclusive GPU reservation.

Pending-compute allocation still rejects. Before widening admission, root fresh
lower allocation outputs through `with_live_queue_memory_model` retake failure,
then separate existing-SDMA-owner admission from actual owner creation and
qualify HostVisible/DeviceLocal initialization independently. Preserve existing
copy/currentness and persistent-compute alias restrictions.

Integrated NEW/REBOUND/resident-overwrite native failures, other queue profiles,
pool-trim failures, generated DATA-ADOPT/ISSUE/COMPLETE, active Stop/drain/graphs,
native depth, aggregate budgets, formal correspondence and matched HIP/HSA
performance remain open. R126/A1/A2/#182 and the overall parity goal remain open.
