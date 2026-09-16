# R126 Directional SDMA Promotion: Development Receipt

This implements retained directional promotion and typed runtime error handoff
above `5bfc1ee232e374378d343a316e7c39895e7bdf8c`. R125 remains the accepted
CPU/test checkpoint. R126, A1/A2, issue #182, formal implementation correspondence
and HIP/HSA parity remain incomplete.

## Source And Custody

`source.patch` contains the complete KFD/runtime source and test delta. SHA-256:
`afcc0980d57cab1fa76f7853114c570be92859f45524ea922ee38ad76b7cf0d8`.
Documentation is separate. The source stayed unchanged during the final complete
regressions and native probes; binary identities are in `binary-sha256.txt`.

The lower queue installs the exact input buffer before the live-model loan,
validates its genuine mapped backing by reference, and retakes before extracting
the owner. Healthy rejection returns Retryable custody; terminal returned errors
carry ProcessTeardown custody; panic leaves the original owner in the poisoned
queue. Retake errors outrank ordinary validation errors and the first panic
survives secondary retake/poison panics. Promotion preserves pool generation and
the outstanding debit, including the existing post-validation zero-debit check.
The root guards allocation, pool checkout/trim, initial bind, retained/legacy
teardown and Drop. It remains session-global across compute-lane selection.

Runtime driver selection happens before input leaves its root. Lower panic seals
the runtime while the queue retains the input. Native errors cross the adapter
without string conversion; returned owners are rooted before the entire final
diagnostic is formatted. A diagnostic panic retains custody and seals the backend.
Successful retryable formatting still enters the existing consuming recycler.
That recycler, demotion and synchronous submit/wait/retire remain separate gaps.
The existing persistent-owner `Rc` and boxed ledger allocations are unchanged;
allocator-failure qualification and allocation-free promotion are not claimed.

## CPU Evidence

Eight new constructed-parent tests use actual freshly allocated mapped leases,
the original directional queues and real model loan/reclaim. They cover:

- Exact identity, logical/physical extents, pair, generation and outstanding debit,
  including 17/4097-byte inputs and a larger backing with a shorter logical range.
- Healthy loan/validation rejection followed by successful reuse of the same
  returned buffer, demotion, backing release and complete parent shutdown.
- Borrowed preflight rejection and genuine foreign mapped-authority rejection.
  The two-parent test restores each original fixture dispatcher before cleanup.
- Validation success/error/panic crossed with retake success/error/panic,
  post-reclaim failures and actual model revision regression, with secondary
  poison panics. Native records/accounts, primary resources/signals and
  directional observations stay unchanged; loan generations follow exact oracles.
- Terminal reentry and process-isolated public guard/Drop behavior.

Seven new scripted runtime tests plus the existing promotion regression cover
retryable and terminal errors, driver selection and lower promotion panics,
typed native diagnostics with injected formatter unwind, exact original panic
identity, unchanged neighboring owners and shadow allocations, no premature
publication/accounting/profile commit, inert terminal retries, configured and
unconfigured public Context behavior, and two isolated terminal Drop modes.
Configured Context retains eight requested bytes, one record and one quarantined
record across panic/retry. The unconfigured facade seals on its next valid backend
call; the backend is already terminal immediately after unwind.

Final complete library regressions pass on the unchanged source:

| Toolchain | Crate | Passed | Failed | Ignored | Duration |
| --- | --- | ---: | ---: | ---: | ---: |
| GNU | KFD | 1,334 | 0 | 0 | 1,060.79 s |
| GNU | Runtime | 780 | 0 | 5 | 27.00 s |
| musl | KFD | 1,334 | 0 | 0 | 1,612.44 s |
| musl | Runtime | 780 | 0 | 5 | 33.94 s |

The two clean focused GNU invocations each pass eight tests. Strict
all-feature/all-target Clippy, no-default-feature runtime compilation and
formatting pass. Unsafe-source policy passes five tests with its explicit
inventory-maintenance test ignored. This is not a full-workspace,
compiled-negative, formal-proof or full R126 qualification campaign.

Commands:

```sh
prlimit --core=0:0 -- <gnu-kfd-test> sdma_promotion --test-threads=2
prlimit --core=0:0 -- <gnu-runtime-test> sdma_promotion --test-threads=2
prlimit --core=0:0 -- cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib -- --test-threads=4
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
prlimit --core=0:0 -- <musl-kfd-test> --test-threads=4
prlimit --core=0:0 -- <musl-runtime-test> --test-threads=4
cargo clippy --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

Exploratory attempts found test-only Debug/import/API-name/hex-format compilation
errors, incorrect child-test naming and two-parent fixture dispatcher selection,
and a missing-driver fixture Drop-disarm mistake. The full diagnostic log retains
the latter abort. `promotion-focused-final.log` contains a lower eight-pass run
followed by the runtime abort, not a passing combined invocation. The final
focused commands use the same binaries as the complete GNU regressions and are
recorded separately in `focused-gnu-kfd.log` and `focused-gnu-runtime.log`.
Early `promotion-focused*` and `clippy.log` files are rejected history, not
passing qualification; some earlier exploratory failures were not
archived and are not represented as a complete attempt inventory. Review also
found a matrix that did not require Err on validation failure; the final test
requires the exact expected success/error/panic outcome. Two independent bounded
source reviews found no remaining blocker in the promotion boundary. They do not
qualify recycling, native injected failures or the complete runtime.

## Native Evidence

The stripped musl runtime binary SHA-256 is
`96fd0fa5f00d5b6bc2538a5a0f674513d346b5fbf920936a6425ba5d18e4e247`;
the uploaded hash matches. All seven probes passed in separate sequential
processes on MI300X GPU 1, unique ID `ab83d2ffef0d3cdf`.

The new `native_runtime_device_promotion_roundtrip_and_retained_shutdown` probe
uses public backend allocation/write/release with genuine Native promoted owners.
It checks 17 logical bytes on 4096 physical bytes and 4097 on 8192, public initial
zero readback and patterned native SDMA roundtrips. The patterned read uses the
internal download helper and requires its handled result to be true; it does not
claim public patterned-read profile coverage. Readback SHA-256 values are:

- 17 bytes: `cca448791d4bcee8fe07acb2b42c1ea727893454bf2d47cfbd43b644d9c6fd76`.
- 4097 bytes: `7d9f8ee9ea61461d10b2b71c408cb7153390786855259a9ded5872aa87b514e3`.

Device-account usage is exactly 12,288 bytes/two records before release and zero
after trim, with no reserved/retained/quarantined remainder. Retained primary
shutdown observes the host account refund from 532,480 bytes/three records to
zero. The eight-event public runtime history validates; the internal download is
not a separately observed public event. These are successful native operations,
not native promotion fault injection or physical overlap evidence.

The six unchanged regressions cover HostVisible allocation/shutdown, primary and
two-stream vecadd with three/six readbacks, and AUX host-budget rejection after
zero/one/two initialized owners. Vecadd output remains
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
Dispatch profiles contain 22/41 events. AUX failures retain
38,281,216/42,475,520/46,669,824 bytes and 12/13/14 records until process exit;
these are not successful native cleanup observations.

Every remote invocation used `prlimit --core=0:0`, an exact test name,
`--ignored --nocapture --test-threads=1`, the explicit device ID and
`timeout --signal=TERM --kill-after=10s 120s`. AUX probes additionally set
`FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=0`, `1` or `2`.

## Cleanup And Remaining Work

Only the stripped executable was uploaded to
`/tmp/fe2o3-r126-promotion.1ALXQU`. After all seven commands exited zero, the
anchored process query found no matching executable (status 1). The exact file
was removed, the empty directory removed with `rmdir`, and absence verified.
GPU 1 reports zero utilization/VRAM percentage afterward; GPU 0's existing 44%
VRAM use is unchanged. No reset, service stop or unrelated deletion occurred.
The local stripped copy was hash-verified and removed; no binaries are archived.

Next work is consuming recycle and synchronous-copy custody, typed healthy
backing-credit rejection, and genuine borrowed directional-owner admission before
enabling allocation with pending compute. Generated DATA-ADOPT/ISSUE/COMPLETE,
readback/typed replies, Stop/drain/graphs, production Context/resource proofs,
other profiles and broader native qualification remain. No HIP/HSA performance
comparison or speedup is established by this packet.
