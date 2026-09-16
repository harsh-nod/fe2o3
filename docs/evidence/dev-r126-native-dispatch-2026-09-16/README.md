# R126 Native Dispatch: Development Receipt

R125 remains the accepted local CPU/test checkpoint. This packet qualifies two
bounded native success workflows and fixes repeated shutdown and missing SDMA
host-read observations. It does not accept R126, close A1/A2 or #182, prove full
formal/native correspondence, or establish HIP/HSA parity or performance gains.

## Source And Changes

Base: `6648160d45f78dc1f6daa785e272a420555f1a6a`.
`final-source.patch` is the complete four-file runtime source delta with zero
context; apply to that base using `git apply --unidiff-zero`. SHA-256:
`056fd7a73c889c7c323260f86610533fee0ff4c1004cdc6749126f92fdf11d6d`.
`SHA256SUMS` binds the patches, toolchain records and raw logs. Executable hashes
are in `final-executables.log`; no binaries are checked in. Frozen libtest copies
prevent concurrent builds from replacing executables used by `current_exe()`.

Production changes are limited to:

- An idempotent retired-queue shutdown guard after existing terminal/busy checks,
  before native teardown. Repeated shutdown previously panicked when the SDMA
  enabled flag survived successful queue retirement. CPU regressions cover the
  retired state and multi-device retry after a child rejects for live resources.
- Exactly one HostRead observation after a complete successful SDMA download,
  including transient staging recycle. ContentIdentity uses actual returned
  bytes; RangeOnly remains unhashed. Errors never emit a successful read event.

Test-only hooks observe configured primary account usage before release and
after settlement but before completed-root Drop. Repeated native shutdown is
checked with re-armed sentinel hooks, not by comparing old observations. A
test-only immutable recorder accessor inspects negative fixtures without
finalizing a capture or bypassing the public terminal/lifecycle checks.

## CPU Validation

Both builds used `cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime
--all-features --lib --no-run`; musl added
`--target x86_64-unknown-linux-musl`. Frozen runtime executables ran under
`prlimit --core=0 -- <runtime-binary> --test-threads=16`.

| Check | GNU | Musl |
| --- | --- | --- |
| Full runtime library | 750 passed, 2 ignored | 750 passed, 2 ignored |
| Test duration | 18.12s | 21.47s |

All final builds/runs exited 0. The three new read-observation tests cover
host/device storage, both content modes, full/subrange/empty reads, deliberately
stale shadow bytes and hashes, out-of-bounds rejection, failure before destination
mutation, failure during staging recycle after mutation, and a second-chunk
failure after a visible prefix. Successful chunking emits one full-range event;
failed operations emit none. Terminal fixtures retain their expected owners and
still reject public profile finalization. Scripted failures are not hardware
fault evidence.

Strict Clippy passed for both crates with `--all-features --all-targets --
-D warnings`. `cargo fmt --all -- --check` passed. The unchanged unsafe-source
policy passed five tests with its inventory-refresh maintenance test ignored:
`cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy`.
KFD library tests, full-workspace regressions, compiled negatives and checker
calibrations were not rerun in this packet. Test durations are not performance
measurements.

## Native Success Workflows

The stripped musl binary SHA-256 is
`cbf3c2c012739a98886729deeb6466447de54af8e6488f1cb4bfd308fc81677a`.
The uploaded file's independently computed hash matches. Both probes ran
sequentially on shared `mi300x`, selecting only GPU unique ID
`ab83d2ffef0d3cdf` (GPU 1). Pre/post checks show its VRAM percentage and utilization
at zero; GPU 0's pre-existing 44% VRAM allocation was untouched. These observations
do not establish exclusive GPU reservation. No reset, service stop or fault
injection was used.

Each test used:

```sh
ssh mi300x prlimit --core=0 -- \
  env FE2O3_TEST_NATIVE_UNIQUE_ID=ab83d2ffef0d3cdf \
  timeout --signal=TERM --kill-after=10s 120s \
  /tmp/fe2o3-r126-profile.YoDorP/runtime-native \
  --exact <test-name> --ignored --nocapture --test-threads=1
```

Exact names, both prefixed by `kfd_backend::retained_release_tests::`:

- `native_runtime_allocation_shutdown_selects_retained_directional_release`:
  passed in 1.95s, 751 filtered. One public 4 KiB allocation returns to the SDMA
  pool; no compute packets. Retained teardown observes 532,480 bytes/3 records
  before release and zero after settlement, with a 16 MiB/32-record budget.
- `native_runtime_typed_dispatch_shutdown_refunds_and_profiles_retained_primary`:
  passed in 5.49s, 751 filtered. Existing exact authenticated gfx942 vecadd
  admission, three 4 MiB HostVisible allocations, one typed launch, and full-byte
  checks of A/B preservation and expected C. Output SHA-256:
  `79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
  Live configured account: 26,222,592 bytes/10 records. Before retained-primary
  release: 532,480 bytes/3 records; after settlement: zero, under the unchanged
  64 MiB/128-record budget. Reserved/retained/quarantined counts all reach zero
  without poison. Completed-root Drop and backend Drop finish.

The typed probe explicitly checks actual auxiliary ordinal 1 and one auxiliary
owner. Its `primary_ordinal=0` output is a fixed label, not an additional observed
assertion. Its trace has 22 observed events, zero dropped
events, all three read identities/ranges and one matching
queue-created/published/completed/destroyed chain. The profiler's complete-history
flag is supplemented with this exact expected roster. Device clocks, copy-engine
events, counters and other unavailable profiler evidence remain unavailable.

**This proves auxiliary execution followed by retained bootstrap-primary teardown,
not execution on the retained primary.** A distinct fresh-primary binding path,
with rooted partial preparation and genuine model transitions, remains necessary.
No permissive authority, fabricated generation or continuation is introduced.

## Reproductions And Review

All diagnostic patches below apply independently to the same base, not in order:

- `regression-before-fix.patch` and `retired-shutdown-before-fix.log` preserve
  the original repeat-shutdown panic (exit 101). The initial test expected
  rejection; code review established that idempotent success is the correct
  multi-device retry contract. Final source/tests implement that contract.
- `pre-profile-source.patch`, `pre-profile-native-executable.log` and
  `pre-profile-native-dispatch.log` preserve the earlier successful vecadd probe
  that exposed the profiling omission: only C's read was recorded after three
  actual reads. Its 20-event trace is diagnostic, not final profiling evidence.
- `host-read-before-fix.patch` and `host-read-before-fix.log` preserve the CPU
  regression failing with an empty read roster (exit 101) before the production
  event fix.
- `profile-first-source.patch` and `profile-first-debug.log` preserve an interim
  test-harness error: finalizing a deliberately incomplete terminal fixture
  correctly returned `IncompleteLifecycleMarkedComplete`, then panic-on-Drop
  aborted the run (exit 134). The final tests inspect immutable raw events instead;
  no production validation was relaxed. Only the final logs above are passing
  regression evidence.

Read-only reviews checked shutdown guard placement and multi-device retry
semantics, actual bootstrap/auxiliary routing, account observation boundaries,
and successful versus partial/failed read-event emission. Execution found the
interim harness error missed in static review; the reviewer subsequently checked
the test-only accessor and unchanged public rejection. Reviews are not formal
proof or hardware fault qualification.

## Cleanup And Remaining Work

Both final remote test processes exited 0. The anchored process check
`pgrep -af '^/tmp/fe2o3-r126-profile[.]YoDorP/runtime-native( |$)'` returned 1,
indicating no match. Only our uploaded executable was removed, then its private
directory was removed; `test ! -e /tmp/fe2o3-r126-profile.YoDorP` passed. The earlier
diagnostic probe's `/tmp/fe2o3-r126-dispatch.dBKWxE` directory was also removed.
No remote scratch or test jobs remain from either run.

Remaining: actual primary-executed binding/dispatch, applicable native error and
panic retention, integrated pool-trim failure/account observations, additional
ordinary/XGMI/window owner joins and queue profiles, full qualification gates,
formal correspondence, aggregate-memory behavior and matched HIP/HSA benchmarks.
