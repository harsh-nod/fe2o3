# R126 Runtime Host Write Retention: Development Receipt

This closes the borrowed host-write prerequisite above fresh SDMA allocation
custody. R125 remains the accepted CPU/test checkpoint. R126, A1/A2, issue #182,
formal implementation correspondence and HIP/HSA parity remain incomplete.

## Source And Ownership

Base: `eab31046246532e89d8998628f9c0f4c09ecb446`.
`source.patch` contains the complete runtime source/test delta; SHA-256:
`707d20695956f36a2da13b59d8d1965caf47f79d7b66a50b469f53f2451fb525`.
Documentation is separate. `SHA256SUMS` binds this receipt, patch, toolchain,
binary identities and raw logs. No executable copies are archived.

Fresh HostVisible initialization and upload staging keep the buffer outside
`catch_unwind` while invoking the borrowed write and its error conversion.
Failure installs that exact buffer in the existing terminal SDMA slot before
poisoning or constructing diagnostics. Failed staging is retained directly,
not passed through consuming recycle. Indexed ordinary/authenticated host writes
borrow the existing record instead of extracting its owner. All four branches
seal the backend on error or panic. The non-formatting poison operation is shared
with existing terminal errors. Original panic identity survives a secondary
poison panic, including one whose payload destructor would panic.

Successful initialization returns the same buffer before the existing allocation
record/accounting commit. Successful indexed writes keep the existing later
shadow/digest/profiling commit. No owner clone, new heap custody container or
additional byte scan is introduced on success. This is a source-level observation,
not an allocation-count benchmark, memory bound or speedup measurement.

## CPU Validation

The final focused GNU run passes nine test functions. Scripted matrices cover:

- Fresh initialization and upload staging failing before copying, after three
  bytes and after all eight bytes, for both returned errors and panics.
- Fresh host storage filled with `0xa5` before zero initialization, making partial
  initialization observable rather than comparing zeros with zeros.
- Exact new/neighbor owner IDs, live-owner counts, no unexpected drops and an
  unconsumed recycle sentinel after failure. Fresh initialization consumes a
  backend ID but commits no allocation record, staging charge or creation event.
- Indexed partial and full authenticated writes, starting from an authenticated
  `4` image, retaining the exact changed `9` prefix and untouched suffix while
  preserving the original published shadow, digest and full-image identity.
- Failure in chunk two after one completed DeviceLocal upload; failure in chunk
  two of both indexed ordinary and authenticated host writes; invalidated lower
  host certificate and no successful whole-write event.
- Inert allocate/write/read/release/shutdown/profile retries after terminalization.
- Public Context errors and caught panics, successful initialization/upload and
  explicit release, and two process-isolated terminal Drop aborts with core dumps
  disabled. The panic-precedence helper checks exact payload address and one
  invocation of the failing poison callback.

The full frozen runtime GNU/musl suites each pass 773 tests, with four opt-in
hardware tests ignored locally, in 24.80/27.12 seconds. These are runtime-crate
regressions, not a fresh full-workspace or full-KFD campaign. An earlier unarchived
eight-test exploratory run passed before the final indexed chunk-two test was
added; only the nine-test final source supplies this receipt's focused evidence.
Strict Clippy, no-default-feature compilation and formatting pass. Unsafe-source
policy passes five tests, with its explicit inventory-maintenance test ignored.

Commands:

```sh
cargo test --locked --offline -p fe2o3-runtime --all-features --lib host_write_ -- --test-threads=2
cargo test --locked --offline -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
# Freeze each compiled runtime executable, then run:
<frozen-runtime-gnu> --test-threads=4
<frozen-runtime-musl> --test-threads=4
cargo clippy --locked --offline -p fe2o3-runtime --all-features --all-targets -- -D warnings
cargo check --locked --offline -p fe2o3-runtime --no-default-features
cargo fmt --all -- --check
cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
```

Two bounded read-only reviews found no blocking source defect. The review of
test coverage requested authenticated indexed chunk-two failure; the final
matrix includes both ordinary/authenticated error and panic cases. These reviews
are not native failure qualification or a full R126 acceptance audit.

## Native Regression And Cleanup

The stripped musl executable SHA-256 is
`81002b14c50d556d634cb6c5c5517640d50c8caff9f5439b364d22c9b3f3db17`;
the uploaded hash matches. Six unchanged opt-in probes pass in sequential isolated
processes on MI300X GPU 1, unique ID `ab83d2ffef0d3cdf`:

- Public HostVisible allocation/pool return and packetless retained teardown.
- Exact-artifact vecadd through primary ordinal 0 with three full readbacks.
- Two-stream vecadd through primary 0/AUX 1 with six full readbacks.
- AUX host-budget failure after 0/1/2 initialized owners, retaining resources
  until process exit while repeat flush is inert.

Vecadd output SHA-256 remains
`79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3`.
The successful dispatch probes report complete 22/41-event runtime histories and
final host-account refund from 532,480 bytes/3 records to zero. Budget-failure
prefixes retain 38,281,216/42,475,520/46,669,824 bytes and 12/13/14 records; those
are process-exit-only retention observations, not successful native cleanup.

Each probe used `prlimit --core=0:0`, an exact test name,
`--ignored --nocapture --test-threads=1`, the explicit device unique ID and
`timeout --signal=TERM --kill-after=10s 120s`. Budget probes additionally used
`FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=0`, `1` or `2`.
The executable was the only file uploaded to
`/tmp/fe2o3-r126-host-write.CF36Wf`. All six commands exited zero. An anchored
process query returned status 1 with no matches. The exact file was removed,
the empty directory removed with `rmdir`, and absence checked. GPU 1 reports
zero utilization/VRAM percentage afterward; GPU 0's existing 44% VRAM use is
unchanged. No resets, service stops or unrelated file deletion occurred.
After all runs completed, the three frozen local executables were hash-verified
and removed; source patch, hashes and logs remain retained.

These native probes exercise successful HostVisible initialization and writes,
not injected partial writes or native DeviceLocal upload failures. No physical
overlap or matched performance conclusion follows from their profile timings.

## Remaining Work

Consuming promotion still needs its input rooted through live-model validation,
retake and diagnostic conversion. Synchronous DeviceLocal copy/retire/recycle
needs equivalent ownership retention, including failures before publication.
Healthy lower backing-credit rejection still needs typed runtime classification,
with whole-call no-mutation semantics qualified separately from queue creation.
Then borrow and validate existing directional owners before enabling allocation
with pending compute; queue creation must remain idle-only.

An unconfigured Context does not immediately set its terminal marker after the
caller catches a backend panic. The backend is already sealed, and the next valid
backend call returns terminal without native effects and seals the Context. That
existing facade policy is explicitly tested, not broadened by this change.
Scripted partial-write evidence does not establish native fault behavior, formal
refinement, aggregate-memory bounds, graph integration or HIP/HSA parity.
