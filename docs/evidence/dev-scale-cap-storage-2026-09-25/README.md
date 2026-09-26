# Accounted Capacity Development Checks

Development on `codex/r65-runtime-drain-versions`, based on `5eeb9c300`.
This is not a sealed qualification packet, a new formal result, a native result,
or A1/A2 acceptance. See [implementation and remaining work](../../runtime-scale-capacity-v1.md).

## Scope

The new tests exercise fixed host-table admission/destruction, independent
debits, initializer/destructor failure, 1024 exact epoch identities, 1025
rejection, exhaustion, separate observation encoding, replacement and pristine
continuation. Constructed CPU native fixtures exercise fresh/replacement,
bootstrap/initial and auxiliary preparation; persistent binding and wrong-ledger
pristine rebind reject before preparation. The initial-binding shortage cell
checks the existing terminal custody policy after DATA initialization.

The runtime tests cover high-index retirement, contiguous logical commitment,
stale restoration into a vacant reused slot, lane swaps and exact table refunds.
They do not use a public scaled runtime constructor; that constructor is absent.

## Commands And Results

Cargo commands use `--locked --offline`, two build jobs, test/dev debug info 0,
`CARGO_INCREMENTAL=0`, and `RUSTFLAGS=-Clink-arg=-Wl,--threads=1` on the GNU host.
The owned target is `/home/harsh/.codex-tmp/fe2o3-scale-cap-target-20260925-jdWs4TSf`.
Tests use two threads unless explicitly noted. `timeout` bounds execution;
`bash -o pipefail` preserves exit status while `tee` retains output.

| Log | Command scope | Result |
| --- | --- | --- |
| `scaled-final.log` | `cargo test -p fe2o3-kfd --all-features --lib scaled_` | 14 passed |
| `dispatch-binding.log` | Same KFD executable, `queue::dispatch_binding::` | 147 passed |
| `rebind.log` | Same executable, `queue::live::rebind_tests::` | 51 passed |
| `layout.log` | Same executable, `persistent_owner_and_queue_layouts`, one thread | 1 passed |
| `bootstrap-routing.log` | Same executable, `backing_constructor_forwarding_precedes_prepare_and_keeps_process_vm_envelope`, one thread | 1 passed |
| `runtime-accounting.log` | `cargo test -p fe2o3-runtime -p fe2o3-resource-accounting --all-features --lib` | Accounting: 29 passed; runtime: 1425 passed, 3 failed, 27 ignored |
| `runtime-capacity.log` | Runtime executable, `kfd_backend::compute_state::capacity_tests` | 3 passed |
| `fmt.log` | `cargo fmt -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-resource-accounting -- --check` | Passed |
| `clippy.log` | `cargo clippy` for the three packages, `--all-features --all-targets -- -D warnings` | Passed |
| `default-check.log` | `cargo check` for the three packages, `--no-default-features` | Passed |
| `doctests.log` | `cargo test` for the three packages, `--all-features --doc` | KFD: 27 passed; accounting: 3 passed; runtime: 46 passed |
| `kfd-non-construction.log` | KFD executable, `--skip queue::live::construction_primary::integration_tests` | 1282 passed, 1 failed, 292 filtered out |
| `socket-probe.json` | Existing observer packet's `socket_probe.py` | All six socket prerequisite calls return EPERM |
| `kfd-attempt1.log` | `cargo test -p fe2o3-kfd --all-features --lib`, 900-second bound | Timed out, exit 124; 421 completed passing test rows, no whole-suite result |

Test groups overlap and must not be added into a distinct-test total.
Direct executable filters use the executables reported by Cargo in the logs.
These logs are development observations, not signed pre/post source or ELF
identity attestations. The runtime build preceded a test-only constructor's
`cfg(test)` annotation; the native full-suite attempt preceded the final
test-only custody assertions. Final targeted checks compile the later source.

The three runtime failures are `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`. Each reports
`InspectSocket(EPERM)` at `authorized_execution.rs:1317`, matching the earlier
[observer packet](../dev-mixed-observers-2026-09-25/README.md). No socket admission
check or test was weakened. Full runtime qualification did not pass.

The separately scoped KFD run excludes the 292 primary/auxiliary construction
integration cases, including the particularly slow fault matrices. Its sole
failure is `credential_bound_channel_accepts_typed_failure_before_publication`
at `target_debug_telemetry_v2.rs:1173`, reporting `SocketAdmission`. That endpoint
calls `validate_connected_seqpacket`; the direct prerequisite probe reproduces
EPERM on socket-domain/type, peer-name/credentials and pass-credentials operations.
This subset is not a whole-library pass and does not replace the omitted cases.

## Earlier Attempts And Limits

Earlier development console output was not retained as files: the 29 accounting
tests, 145 then 147 native binding tests, and three runtime storage tests passed.
An unrestricted-parallelism KFD run was deliberately interrupted with status 130
after reporting a queue inline-size regression. The size ceiling now allows
one additional KiB of fixed configuration/credit metadata and separately bounds
the table owner to 64 bytes; no 1024-slot storage is inline in the queue. A later
scaled run passed 11 and failed 1 because the new initial-binding fixture lacked
its local runtime gate. The fixture was corrected; subsequent 14-test results
are retained. These early console-only attempts are not claimed as archived.

`kfd-attempt1.log` retains the bounded whole-library KFD attempt, which stopped
at the 900-second deadline while progressing through the auxiliary insertion
fault matrices. It is incomplete, not a pass and not proof of a hang. No musl
run or optimized full-suite retry was performed for this slice.

SSH failed before connection: `Could not resolve hostname sharkmi300x-1:
Temporary failure in name resolution`. No remote files, jobs or reservations
were created. No GPU execution, overlap, native retained-depth or HIP/HSA
performance improvement is claimed.

Further CPU joins remain: simultaneous constructed primary/auxiliary budget
exhaustion and fully scaled successful public detached/pristine rebinding with
aggregate refunds. Existing default-profile join tests and scaled table tests
are not substitutes for these cases. Formal allocator/queue refinement,
aggregate backing/control/slot admission and the complete SCALE-2 native
campaign remain open.

## Cleanup

All validation sessions were terminal before cleanup. The exact owned build
directory above occupied 807,312 KiB according to `du -sk`. It was removed with
`rm -r -- <exact-owned-path>`; an independent `test ! -e <exact-owned-path>`
returned success. The initial force-style removal command was rejected by the
execution environment before running. Cargo caches and unrelated worktrees,
including `dev-owner-inspection-2026-09-23`, were left untouched. Compiled test
executables were not archived; only development source and result logs remain.
