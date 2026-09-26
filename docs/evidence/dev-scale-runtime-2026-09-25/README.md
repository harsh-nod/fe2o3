# Runtime Scale Integration Development

Development evidence for the runtime Qualification1024 opt-in and accounted
allocation custody, based on `5ce4a0548eeacf76549405378331efddd48e0639`.
This is not A1/A2 acceptance, native qualification, formal refinement, or a
HIP/HSA parity/performance result.

## Scope

The runtime's feature-gated exact-vecadd constructor allocates both 1024-slot
pipeline tables before opening KFD. One private host-table account/profile is
forwarded through bootstrap and materialized queue startup. Per-allocation
custody preallocates 1024 owners, admits no growth, refunds provisional failures,
and keeps the complete debit until disposal after the final retirement.
Generated preparation/shell/readiness, DeviceLocal allocation and persistent
preparation reject unsupported scaled use before their mutable work. Default
constructors, 64-slot pipelines and 256-owner custody remain unchanged in scope.

Only requested slot/roster payloads are charged. Allocator overhead, account
arenas, fixed metadata, maps, kernargs, launch snapshots, GPU backing and total
process residency are excluded. Native replacement peaks need additional credit;
later native credit failures can still follow existing terminal-custody policy.
The two-lane CPU test stores 2048 simulated owners; none is a GPU receipt.

## Results

Final full runtime command (exit 101):

```sh
env CARGO_TARGET_DIR=/home/harsh/.codex-tmp/fe2o3-scale-runtime-target-20260925-Pm47qn8h \
    CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
    timeout 900 cargo test -p fe2o3-runtime --all-features --lib -- --test-threads=2
```

`raw/runtime-all-features-final.log`: **1435 passed, 3 failed, 27 ignored**.
All ten new tests pass, including first-owner full preallocation, 1025th rejection,
partial retirement retaining the full debit, second-table byte/record failure,
partial new/existing-roster rollback, moves, mixed-kind retirement, valid generated
shell rejection, early unsupported paths and startup source forwarding. The three
existing runtime capacity tests and the unchanged default custody-bound test also
pass. `raw/runtime-all-features.log` retains the earlier full run with the same
counts, before strengthened assertions and the reused allocation lookup.
An initial console-only focused run passed eight tests before the forwarding test
was added. No compile or test failure was suppressed by changing runtime policy.

Both full runs fail these unchanged tests at
`authorized_execution.rs:1317` with `InspectSocket(EPERM)`:

- `cooperative_debug_telemetry_emits_only_bounded_logical_records`
- `failed_session_end_is_explicit_and_terminal`
- `pre_native_telemetry_failure_is_returned_and_poisoned`

The independently rerun
`docs/evidence/dev-mixed-observers-2026-09-25/socket_probe.py` reports `EPERM` for
all six socket inspections/options in `raw/socket-probe.json`. Production socket
admission is unchanged. This packet does **not** pass full CPU qualification.

Additional final checks, all exit 0 with the same Cargo environment:

| Command | Result | Log |
| --- | --- | --- |
| `cargo clippy -p fe2o3-runtime --all-features --all-targets -- -D warnings` | Pass | `raw/clippy-final.log` |
| `cargo check -p fe2o3-runtime --no-default-features` | Pass | `raw/default-check.log` |
| `cargo test -p fe2o3-runtime --all-features --doc` | 46 passed (4 + 42) | `raw/doctests.log` |
| `cargo fmt -p fe2o3-runtime -- --check` | Pass | `raw/fmt.log` |

Cargo checks had a 900-second outer limit; none reached it. The earlier passing
Clippy run is retained separately in `raw/clippy.log`.

Two read-only agents reviewed startup/authority propagation and custody/storage
ownership. Review identified and corrected method visibility and preservation of
default amortized deque allocation before compilation; test review added first-
owner and partial-retirement debit assertions. Primary owned edits and validation.

## Reproduction And Limits

All Cargo validation uses the environment above; toolchain versions are retained
in `raw/rustc-version.log` and `raw/cargo-version.log`. Validation is GNU/Linux CPU
only. No musl, scaled executable refinement, native capacity, physical-overlap,
or matched HIP/HSA performance result was produced here.

The owned local build target was
`/home/harsh/.codex-tmp/fe2o3-scale-runtime-target-20260925-Pm47qn8h`.
After all build/test sessions terminated, its 544500 KiB were removed using
`rm -r -- <exact-owned-path>`; an independent `test ! -e <exact-owned-path>`
confirmed removal. Cargo caches and unrelated user evidence were left alone.
No shared-host artifacts were created: SSH failed before connecting with
`Could not resolve hostname sharkmi300x-1: Temporary failure in name resolution`.
