# KFD Host Module-Image Residency Development

Developed on `codex/r65-runtime-drain-versions`, based on
`f6519a449a5ecb3d080d1423560ff0b835452197`. This is MEM-4A development evidence,
not sealed native qualification, new formal refinement, A1/A2 acceptance or
HIP/HSA parity.

## Implementation

`KfdRuntimeBackendV1::configure_host_image_budget_v1` installs an optional,
immutable backend-local byte and allocation-record ceiling before resource
history. Invalid configuration leaves the backend unchanged. Other native
budgets may be selected in either order. `host_image_usage_v1` is inert and
remains available before native startup and after terminal failure; `None`
means unconfigured, not zero residency.

Module loading reserves `ExecutableHostImageBytes` and one record before the
owned copy, hash or parser. Configured storage must have exactly the reserved
Rust `Vec` byte capacity. Allocation/parse errors and pre-commit unwind refund
provisional admission after disposing temporary ownership. Global allocator
abort is not a recoverable allocation-failure guarantee.

`ResidentModuleImageV1` and `ResidentKernelImageV1` carry one shared image owner
through module records, resolved kernels and prepared launches. There is no raw
owning-envelope accessor. Kernel aliases are destroyed before their shared
owner, and the final owner explicitly destroys the loader envelope before
refunding the retained credit. Duplicate loads copy and charge independently;
kernel resolution and prepared-launch clones share the original image debit.
An unloaded module may remain charged while an internal prepared/image alias
still owns its bytes. Failed unload never refunds that retained image.

Unconfigured backends remain unaccounted, but still use the new owner wrapper;
no unchanged-allocation-cost or performance claim is made. Invalid images now
reject before the module hash and optional profiling hash. Successful module
identities and load events retain their existing semantics.

## Coverage Boundaries

The focused tests cover independent byte/record exhaustion before allocation
and parsing, exact capacity, invalid images, injected allocation errors,
allocator/parser panic, duplicate loads, repeated resolve/unload cycles,
shared pointer identity and varied final-owner drop order. Actual backend
preparation keeps its image credit across unload and backend destruction.
Reload rejects while an alias survives and succeeds after its final disposal.

Configuration tests cover invalid record limits, zero-byte admission denial,
immutable selection, released resource history, independent device-budget
selection, unchanged capabilities/IDs, wrong devices and unknown modules.
The caller-unwind test panics after a successful load; it is not an injected
internal post-insertion failure. Exhausted handle IDs exercise an actual
post-parse, pre-insertion refund.

Pending-dispatch coverage uses logical admission with a CPU native-dirtiness
marker that defers publication. It checks `Busy` unload, cancellation,
submission release and final unload without creating native custody. Recycled
detach coverage is a metadata-only missing-lane rejection (`Unsupported`),
not an ambiguous native disposal. A separate retained-persistent-control
metadata fixture reaches real terminal error handling and verifies preserved
module/kernel owners, retained metadata and observable credit. Only these
CPU-only fixture markers are disarmed for cleanup; production terminal
native custody is not claimed to be recoverable.

The budget excludes parser/metadata/kernel-cache storage, allocator/Arc/account
overhead, Worker transport and frame copies, generated source/materialized
images, profile captures, authority-callback copies, native executable/control
backing and other backends. Repeated kernel resolution can still grow excluded
metadata. This is not total executable, process, device or multi-Context memory
accounting. MEM-4B, MEM-DOM and MEM-5 remain open.

## Validation

Commands use the owned target
`/home/harsh/.codex-tmp/fe2o3-image-residency-target-20260926-XDKnC7cS`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_INCREMENTAL=0`. Cargo commands use `--locked --offline`; library tests
use two threads.

The runtime test executable is `debug/deps/fe2o3_runtime-dcc6ccea00c58f82`.

| Raw log | Command scope | Result |
| --- | --- | --- |
| `host-image-qualified.log` | Final runtime executable, `host_image_ --test-threads=2`, 120-second bound | 14 passed |
| `runtime.log` | `cargo test -p fe2o3-runtime --all-features --lib -- --test-threads=2`, 1,200-second bound | 1,462 passed; 3 failed; 28 ignored |
| `clippy.log` | `cargo clippy -p fe2o3-runtime --all-features --all-targets -- -D warnings` | Passed |
| `default-check.log` | `cargo check -p fe2o3-runtime --no-default-features` | Passed |
| `doctests.log` | `cargo test -p fe2o3-runtime --all-features --doc` | 46 passed |
| `fmt-qualified.log` | `cargo fmt -p fe2o3-runtime -- --check` | Passed |

The full runtime failures are
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal`, and
`pre_native_telemetry_failure_is_returned_and_poisoned`, all reporting
`InspectSocket(EPERM)`. They match the preceding development baseline; this is
not a fully passing CPU suite. The focused tests are included in the full run,
not additional independent coverage. The KFD library test suite and hardware
tests were not executed for this runtime-only change.

The initial focused run (`raw/host-image-initial.log`) passed ten tests and
failed two test assumptions: submission could progress immediately rather
than remaining pending, and missing recycled-lane state returns `Unsupported`
rather than terminal failure. The expanded run (`raw/host-image-final.log`)
passed thirteen tests and caught one incorrect test expectation (`UnknownHandle`
instead of `WrongDevice`). These failed logs are retained, not overwritten.

`raw/socket-probe.json` independently reproduces EPERM for all six socket
inspection prerequisites. `raw/mi300x-access.log` fails at hostname resolution
before reaching MI300X. No remote resources were created.

## Cleanup

All validation processes reached terminal status before removal of the exact
owned target above. It held 560,280 KiB of allocated storage, recorded in
`raw/cleanup-before.log`. After `rm -r --` succeeded, independent `ls -ld`
reported ENOENT in `raw/cleanup-absence.log`, and `test ! -e` succeeded.
No unrelated build directories or the user's owner-inspection evidence were
modified or removed.

## Remaining Gates

Native module/dispatch/cache pressure and ambiguous unload campaigns, generated
image and native materialization accounting, aggregate bootstrap/terminal
headroom, formal refinement, signed independent replay and matched HIP/HSA
measurements remain open. CPU ownership tests do not establish native or
performance parity, nor any milestone acceptance.
