# R126 Directional Release: Development Native Receipt

This is packetless lifecycle evidence, not a full R126 qualification archive,
formal refinement, copy-execution test, performance result or HIP/HSA parity.

## Source And Build

Base: `0bf0faf065f8c599e41b5cf25ffeca486d517ccf`.
`source.patch` records the KFD/runtime source delta on that base. It uses zero
context; apply with `git apply --unidiff-zero`. `SHA256SUMS` binds the patch,
toolchain, build/freshness logs, GPU precheck, raw native output and local
development checks. No binary is checked in.
`unsafe-inventory.patch` separately records the reviewed test-only inventory
delta used by the policy check; apply it normally alongside the source patch
when reproducing that check. It does not affect the native executable.

```sh
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime \
  --all-features --lib --no-run
cargo test --locked --offline -p fe2o3-kfd -p fe2o3-runtime \
  --all-features --target x86_64-unknown-linux-musl --lib --no-run
strip --strip-debug -o runtime-test \
  target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-fc1dc596f3761ce3
```

The uploaded static-PIE executable was 51,283,672 bytes, SHA-256
`dad9379b14cd7f91b5997847f49c40782c038fa522e862a9a40b5ae7b8ca7723`.
Its remote SHA-256 matched before execution.

The final-source build freshness recheck (`freshness.log`) completed without
recompilation. The unstripped runtime executable's SHA-256 was
`aec00715b5cafe23d1f6fec6ece5a77ff351fcc8a37b73b2b4e7fe9d3bf18b58`.

## Execution

SSH host: `mi300x`. Explicit device unique ID: `0x6ced1647a296545c`.
The precheck reported zero GPU utilization; this was not an exclusive GPU
reservation. No remote build, GPU reset, service stop or fault injection was used.

In a fresh SSH process, with core dumps disabled:

```sh
env FE2O3_TEST_NATIVE_UNIQUE_ID=6ced1647a296545c \
  timeout --signal=TERM --kill-after=10s 60s ./runtime-test \
  --exact kfd_backend::retained_release_tests::native_runtime_allocation_shutdown_selects_retained_directional_release \
  --ignored --nocapture --test-threads=1
```

The test and SSH process exited normally with status zero. One test passed,
744 were filtered out, and the test reported 2.20 seconds. This duration is not a
benchmark. The test used public backend open, stream, HostVisible allocation,
release, stream destruction and owned-shutdown APIs. A test-only observer recorded
the actual post-pool-trim retained selector; it did not force the branch or seed
a queue. The completed primary/directional root was dropped by real runtime
shutdown. Subsequent stream/allocation reuse was rejected; the final profile
validated without false logical-compute queue events. The backend was explicitly
dropped before the success marker. Zero packets were submitted.

This does not qualify pending copies, native error/panic retention, assigned
compute-lane profile events, other SDMA profiles, dispatch-bearing queues or
pool-trim failure custody. Those remain separate work.

## Local Development Checks

The completed constructed-parent filter passes all 21 tests on GNU and musl.
Musl shared-memory and Linux-platform filters pass 274 and 29 tests; the full
runtime library passes 744 on each target, with this opt-in hardware test ignored
locally.
Strict all-feature/all-target Clippy passes for KFD and runtime. The unsafe-source
policy passes five tests with its inventory-refresh test ignored; the inventory
adds only the reviewed test-local anonymous doorbell mapping, not production
unsafe code. These are development checks, not full-workspace qualification.

The earlier broad GNU run in `gnu-relinked-failed.log` is failed history:
1,291 passed and five failed on an intermediate 1,296-test source cohort.
The executable was replaced by a concurrent build after the run started.
All five failed at `Command::new(std::env::current_exe())` process creation with
`ENOENT`, not a teardown assertion. Inspection confirmed the running executable
had zero links and `/proc/PID/exe` ended with ` (deleted)`. Final-source Linux
and USERPTR reruns pass 29 and 13 tests. A clean full-suite run must use frozen
executable bytes; focused reruns do not turn the earlier run into a pass.

The fresh GNU build check completed without recompilation. Private executable
copies used for the full runs have these SHA-256 identities, matching their
build outputs before execution:

| Harness | SHA-256 |
| --- | --- |
| GNU KFD | `01398e45f8482436c2f3a3f068785c5b382a37dcb1a15732c937aac85ee38151` |
| GNU runtime | `1d600bc1c9f6601d8ba5ab0b4c44d995bb0a0cf989350d8b115e21bc233d2355` |
| Musl KFD | `32563911f098df7b71cdc1c246f5aa0db762d4f027db24bbc1d0f2d635d1d5ca` |

The GNU runtime copy passed with `--test-threads=16` in 28.29 seconds. Harness
durations are test execution observations, not runtime performance comparisons.

The full frozen GNU KFD copy subsequently passed all 1,297 tests with no failures,
ignored or filtered tests in 1,791.97 seconds (`--test-threads=16`). Its process
exited zero. This includes successful executions of all five subprocess tests
from the failed intermediate run. The failed log remains separate history.

The full frozen musl KFD copy also passed all 1,297 tests with no failures,
ignored or filtered tests in 2,255.99 seconds (`--test-threads=16`), exiting zero.
All three frozen executable hashes were unchanged after their completed runs.
An anchored process check found no remaining process under the private harness
directory. The two full KFD logs and two full runtime logs cover the final source
delta, not the intermediate source used by the relinked failed run.

Raw logs retain libtest's trailing whitespace and final blank lines. Rustfmt and
the staged diff whitespace check pass for source, documentation and patches;
the latter deliberately excludes these unmodified `*.log` artifacts.

Two bounded read-only reviews found no new blocking correctness issue in this
extension. Source review also confirms that the native test's selector assertion
cannot pass on a silent legacy fallback. This is not independent acceptance of a
full R126 qualification archive.

## Cleanup

The only remote artifact was `runtime-test` under the private `mktemp` directory
`/tmp/fe2o3-r126-directional-release-20260916.tPwFGUj3`. After confirmed process
exit, an anchored process check found no matching test or timeout process
(`pgrep` status 1). Removing the exact binary and then its directory both
succeeded. A final `test ! -e` returned zero. No probe job or staging directory
remains on the shared machine.
