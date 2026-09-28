# Generated-Only Composed Startup

Status: implemented, CPU-qualified and checked-device admission-qualified.
This does not qualify protected Worker execution, formal native refinement,
aggregate process memory, or HIP/HSA behavioral/performance parity.

Source: signed `7bf41ba2d61683d3b7d7d428beab7b64cb573422`.

## Implementation

The generated-only backend now has open and checked-device constructors accepting
the existing composed root and typed device/session budgets. Both use the existing
Context-request/N1/N2 admission machinery and retain `WorkerV3GeneratedOnly`.
They neither require a caller-supplied generic launch authority nor fall back to
an unrooted backend after failure.

The additive host entrypoint is
`run_inherited_worker_v3_current_thread_with_composed_backing_root_v1`.
It shares authentication and execution with the existing entrypoint. The private
backend factory runs after runtime-evidence validation, bounds checking, one
argument-constructor call, and the second deadline check. Existing configuration,
Context-construction retention, typed completion, Stop, drain and shutdown behavior
are unchanged. The root is borrowed during initialization; admitted owners retain
their original accounting root. Result-storage accounting remains separate.

This closes startup access to the already implemented composed generated-adoption
path, not the missing production verifier/refinement providers and owned proof
artifacts. The public signature still requires the refining verifier adapter.

## Qualification

Toolchain: `nightly-2026-04-03`, GNU target, unoptimized test profile with debug
assertions and overflow checks. Cargo used `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2`,
`--locked --offline`. No new Verus campaign was run.

| Check | Result |
| --- | --- |
| Runtime all-feature library, exact stripped ELF | 1818 passed, 0 failed, 30 hardware ignores |
| Host no-default-feature library | 186 passed, 0 failed |
| Host and Runtime no-default-feature doctests | 30 and 53 passed |
| Both crates, all-feature/all-target strict Clippy | Passed, `-D warnings` |
| Both crates formatting and Git whitespace | Passed |
| MI300X checked-device admission test | 1 passed, 0 failed |
| Strict endpoint observations | 3 passed; all 4 recorded groups reaped |
| Offline record/endpoint replay | Passed |

The CPU suite includes generated-only generic/atomic/collective denial, composed
policy refusal, generated-shell whole-roster pressure and retirement, and owned
shutdown controls. The new host source-wiring test checks call-site ordering;
it is not behavioral authentication or successful protected execution evidence.
Doctests compile the borrowed-root APIs and reject a protected-only verifier.

The initial focused build exposed a test-only comparison between lower and
Context-specific usage types. Its diagnostic is retained. The corrected test
compares all fields, including the Context device identity. Final qualification
used the corrected source.

## Checked-Device Scope

The exact ignored test is
`kfd_backend::native_budget::generated_composed_tests::native_generated_composed_admission_pressure_gate_and_refund`.
On MI300X GPU1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, it checks:

- Exhausted session domains reject with Capacity and preserve root usage.
- The canonical device budget cannot be replaced after session disposal.
- Successful construction retains Required request accounting and the restricted
  generated-only gate, while preserving copy/pool capabilities.
- Witness-free allocation is refused; exhausted request credit rejects through
  the actual Context before backend allocation or native startup.
- Releasing pressure permits a complete request-roster reservation again.
- Context shutdown preserves Required policy; unused owners refund to the
  bootstrap baseline and their session domains can be admitted again.

Root, device and request ceilings coincide in this test, so it does not identify
which individual ancestor rejects the byte pressure. It does not register generated
shells, allocate native backing, create a VM/queue, dispatch a kernel or obtain a
protected generated completion. Reopening here is possible because no VM was
acquired; the process-lifetime VM re-admission gap remains open.

The native run uses the same debug-stripped ELF as the full CPU run: 93,010,640
bytes, SHA256 `5b234ccb735c25e6a6512d2a4125fba755747b28d77120987d2392cf54b5174f`.
The original Cargo ELF SHA256 is
`d620cd2724be951964f53cc3dc32826d4905cdf26c382e71af034d6bb4701d4f`.
This is a local incremental-cache build with selected-source hash checks and a
signed commit, not a hermetic build or an authenticated compiler refinement proof.

Pinned observer/recording helpers capture fresh preflight, immediate postflight
and delayed postflight, including strict raw SMI/PID/sysfs checks. All nine sysfs
samples report 312,930,304 VRAM bytes and idle selected engines; no selected PID
is present. These are endpoints, not exclusive reservation or continuous isolation.
The raw script and independent offline check preserve those limits.

Collection SHA256 matches before cleanup. Only the owned directory was removed,
and a separate SSH command confirms absence:
`/home/harsh/fe2o3-generated-composed-20260928.D3O6QPOY`.
No remote build, reset, foreign-process action or shared-cache removal occurred.

## Reproduction

From the repository root with the Cargo environment above:

```sh
cargo test -p fe2o3-runtime --all-features --lib --locked --offline -- --test-threads=2
cargo test -p fe2o3-host --no-default-features --lib --locked --offline -- --test-threads=1
cargo test -p fe2o3-host -p fe2o3-runtime --no-default-features --doc --locked --offline
cargo clippy -p fe2o3-host -p fe2o3-runtime --all-features --all-targets --locked --offline -- -D warnings
cargo fmt -p fe2o3-host -p fe2o3-runtime --check
```

`raw.tar.gz` retains logs, selected-source hashes, the executed ELF, pinned
helpers, the one-case runner, native records and cleanup observations. After
extraction, `python3 -I -B verify_native.py` checks record consistency and strict
endpoints offline. This is not a semantic proof of the native implementation.

A1/A2 and broader accepted checkpoints are unchanged. Production Worker providers,
Context/journal proof composition, device-gated dependency execution, scale/reuse,
aggregate accounting and matched HIP/HSA performance remain open.
