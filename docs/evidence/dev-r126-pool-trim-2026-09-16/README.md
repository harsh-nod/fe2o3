# R126 Retained Pool Trim: Development Receipt

This packet fixes pool-trim failure custody before primary teardown. It does not
accept R126, close A1/A2 or #182, prove native/formal correspondence, benchmark
copy execution, or establish HIP/HSA parity. R125 remains accepted at the local
CPU/test boundary.

## Source And Executables

Base: `1daa443fe267890ebaf1db96543eefbcb3e51eec`.
`source.patch` records the complete KFD/runtime source delta, including new
modules and tests, with zero context; apply using `git apply --unidiff-zero`.
`SHA256SUMS` binds the patch, toolchain and raw logs. No binary is checked in.

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --lib --no-run
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib --no-run
```

Private frozen copies avoid overwriting libtest executables while their tests
spawn `current_exe()`. The source was unchanged during the passing runs.

| Executable | SHA-256 |
| --- | --- |
| GNU KFD | `46aec3d57eef99d4f9cca6325b0f1bca80b50f36380035f03d54bc2f7aeafa6d` |
| GNU runtime | `94b3a4621e58aa11419e7659699a9147566ce75f670ab4971b3551d6c9c6c72a` |
| Musl KFD | `40a3ce35924899c2363fbfbc434b1d738c255613c8fbe6ce3dc54415f5e0a0a6` |
| Musl runtime | `81dc0c3d737ffe5dbbb3450c3e4742008aba9e392d87cc8630b95a54463b4820` |
| Stripped native runtime | `f3e9b76cd340bc3db89d4735a5da31717700903b0a5fbae835950282cb99a53f` |

The native executable was made with `strip --strip-debug -o runtime-native runtime-musl`.
Its remote SHA-256 matched before execution.

## Local Checks

Both GNU and musl passed:

- `integration_tests::release_cases::`: 27 constructed-parent tests, including
  six new pool-trim tests. GNU took 124.44s, musl 118.14s.
- `shared_memory::tests`: 274 tests.
- `sdma_cleanup`: six tests, including exact original certificate-box retention.
- Runtime library: 745 passed, one opt-in hardware test ignored locally.
- Concrete unfinished-trim Drop, activity-latch and host/device pool-wiring checks.

The GNU `pool_trim` filter separately passed seven tests. Strict Clippy with
`--all-features --all-targets -- -D warnings` passed for both crates. The
unsafe-source policy passed five tests; its explicit inventory-refresh test
remained ignored, and no unsafe inventory change was needed. Formatting passed.

Constructed-parent tests use genuine allocated/mapped fixture tokens, original
completed-constructor foundations and the production ordering/cleanup drivers.
They assert native arguments, model/account prefixes, unchanged backing bytes,
original metadata, one-shot retry behavior and cleanup/retake failure precedence.
The free roster is constructed directly with valid recycled generations, not
through public recycle admission. CPU ioctls and currentness failures are
scripted. The runtime callback test uses a mock without a native queue and
proves terminal settlement/payload identity, not native failed-trim accounting.

These are focused KFD tests and full runtime library regressions, not fresh full
KFD/workspace qualification or authenticated formal proofs. Earlier development
compile errors were corrected before these passing builds; they are not passes.
Reported test durations are not runtime performance measurements.

## Native Probe

SSH host `mi300x`, selected GPU unique ID `0xab83d2ffef0d3cdf` (GPU 1). The precheck
showed zero utilization and zero-percent allocated VRAM for that device. GPU 0
had existing allocations and was not selected. This was not an exclusive GPU
reservation. No service stop, GPU reset, native fault injection or remote build
was performed.

With core dumps disabled:

```sh
prlimit --core=0 -- env FE2O3_TEST_NATIVE_UNIQUE_ID=ab83d2ffef0d3cdf \
  timeout --signal=TERM --kill-after=10s 60s \
  /tmp/fe2o3-r126-pool-trim.TYyyyO/runtime-native \
  --exact kfd_backend::retained_release_tests::native_runtime_allocation_shutdown_selects_retained_directional_release \
  --ignored --nocapture --test-threads=1
```

The test and SSH command exited zero: one passed in 1.92s, 745 filtered out.
The public open/stream/allocation/release workflow produced exactly one cached
4096-byte buffer with zero checked-out buffers before shutdown. The test observed
the real retained-primary selector after trim, without forcing it; verified
shutdown, completed-root/backend Drop, reuse rejection and valid profiling
without false compute-lane destruction events; and submitted zero packets.
This proves nonempty-pool successful native teardown, not error/panic retention,
copy execution, concurrency or throughput.

## Review And Cleanup

Two read-only reviewers checked production ownership ordering and lower
metadata/test oracles. A configuration-latch regression and insufficiently
specific test assertions were corrected before final validation. Their followup
reviews found no new blocker within those scopes. This is not independent full
R126 qualification.

The sole remote artifact was `runtime-native` in the private directory shown
above. After the SSH/test process exited zero, an anchored `pgrep` found no
remaining test process (exit 1). Exact-file `rm` and directory `rmdir` both
returned zero, followed by `test ! -e` returning zero. No remote scratch or test
job remains. No other user's files or processes were modified.
