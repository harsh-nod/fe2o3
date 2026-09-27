# Deferred Initialized-Storage Runtime Adapter

Development checkpoint, not A1/A2, #182, full HIP/HSA parity, native execution,
formal refinement or performance acceptance.

## Source And Evidence

Implementation: `04a3b5cc5df28f909810efcfea8f0b9335d9c990`, SSH-signed and verified.
The primary agent implemented and tested; two read-only agents reviewed native
classification, runtime custody, tests and the next integration/hardware boundaries.

Frozen raw directory:
`/home/harsh/.codex-tmp/fe2o3-runtime-storage-adapter-20260927-KSppBZIk`.
Archive: [receipts.tar.xz](receipts.tar.xz), SHA-256
`eab92742db1f108b9a70dc47c7e10ab512faa1848064bf12efb58920b98daae7`.
Archive comparison passed. The packet retains source hashes, test-binary hashes,
compiler versions, signed source identity, complete patch, commands and logs.

## Implemented Behavior

- Native conversion separates typed clean ineligibility from scope, custody,
  currentness and integrity failures. Missing engine, terminal state and an
  unfinished root cannot grant fallback. Clean refusal is a borrowed metadata
  result before live-currentness validation, not a certificate that another
  operation is safe; that operation must independently validate its admission.
- Runtime conversion occurs only after peer gate, dependencies, FIFO, native
  reconciliation and lane/conflict checks. Admitted single-binding and exact
  distinct full-range R/R/W launches can use actual initialized native storage
  without a content digest or prior-dispatch fiction.
- Exact original owners remain indexed or terminally rooted across refusal,
  error and unwind. Single and whole-roster restoration use preallocated shells;
  cancellation preserves storage origin and completion restores replay origin.
- Successful conversion clears stale digest/full-host-write metadata. Dirty CPU
  shadows do not replace native input bytes. Only typed clean refusal permits
  the existing independently validated synchronized Read fallback; writable
  R/R/W refusal settles unpublished rather than materializing an output.
- Ordered early publication cannot bypass conversion. Incompatible retained
  control is released before replacement publication. Normal normalization and
  release recognize storage-origin custody.
- R57's obsolete fresh-output negative now uses an invalid mixed-memory roster.
  The exact R57 authority and both successful phases are unchanged.

## Qualification

| Lane | Result |
| --- | --- |
| Final runtime all-feature library | 1631 passed, 3 existing failures, 28 ignored |
| Full KFD library | 1676 passed, 1 existing failure |
| Focused native initialized-storage filter | 16 passed |
| New runtime test groups | All 10 passed in focused and full runs |
| KFD/runtime doctests | 42/52 passed |
| Strict KFD/runtime all-feature/all-target Clippy | Passed |
| Minimal runtime and no-default-feature test compilation | Passed |
| R57 example build/test | Compiled; zero tests, no hardware execution |
| Production dependency metadata audit | 43-package closure passed |
| Formatting, whitespace, committed source continuity | Passed |

Runtime failures are the three `authorized_execution::tests` cases
`cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal` and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each reporting
`InspectSocket(PermissionDenied)` at `authorized_execution.rs:1317`.
The KFD failure is
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`,
reporting `SocketAdmission` at `target_debug_telemetry_v2.rs:1173`.
Both source files are unchanged; these failures match the previous checkpoint.
They remain failures, not skipped or passing qualification.

The initial full runtime attempt aborted on an obsolete synchronous-rejection
assertion; an isolated diagnostic and the corrected rerun are retained. Initial
runtime Clippy rejected the inline custody error size; its narrow allowance
matches the existing promotion adapter and avoids allocation during recovery.
After the complete frozen campaign, two test files gained restoration-state and
zero-authority assertions, and the R57 example negative changed. The exact
three-file delta is recorded; native source remained unchanged. The final runtime
suite and strict lint were rerun. See `source-revision.md` and `final-results.md`.

## Limits And Next Work

Runtime tests use a scripted driver; native conversion tests use registered
owners and CPU-injected completion. Neither executes this adapter on a GPU.
The foundation-ownership guard has source review but no new direct public-session
fixture. No new Verus campaign or ELF audit ran. Prior scalar initialization
proofs do not prove this concrete adapter. Zero materialization counters are not
hardware latency or throughput measurements.

MI300X DNS failed before connection; no remote artifacts or processes were created.
Pending directed-peer router admission still needs exact active-DMA ownership,
captured ancestry progression, lifecycle integration and durable mixed-depth
history. Native XGMI compute composition remains separate. Storage-origin first
inputs need a new bounded hardware qualification profile: existing R26/R57
profiles require initial hashes and R60 uses HostVisible buffers. Matched HIP/HSA
runs must distinguish first conversion from steady replay. The archive contains
concrete peer-lifecycle and hardware handoffs. Broader Worker, device-language,
collective, distributed, resource and release acceptance gates remain open.
