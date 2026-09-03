# Authority-free virtual runtime V1

Status: experimental, bounded semantic CPU execution. This contract implements
the first persistent host-lifecycle slice tracked by
[#216](https://github.com/harsh-nod/fe2o3/issues/216).

## Boundary

`fe2o3-virtual-runtime` accepts only an `AdmittedSimulationModuleV1`. It composes
the same deterministic KIR V7 interpreter used by `fe2o3-kir-sim` with
`fe2o3-runtime-model` transitions for a model-only device and VM, allocations,
mappings, loaded KIR, queues, prepared dispatches, publication, completion, and
quiesced failure.

The runtime:

- binds every handle to an exact caller-supplied runtime identity and monotonic
  ordinal; the CLI identity additionally commits to the target, bundle presence
  and identity, command mode, and all runtime/simulation limits;
- retains exact KIR identity, target profile, simulation limits, and virtual
  runtime limits;
- exposes allocation-relative byte copies and initialization state, never host
  pointers or synthetic virtual addresses;
- accepts typed scalar and allocation-view arguments for any kernel admitted by
  the existing simulator subset;
- executes ready dispatches in deterministic submission order with explicit
  earlier-completion dependencies;
- rejects malformed or mixed-type allocation views before modeled dispatch
  preparation and copies each shared backing exactly once only after successful
  semantic execution;
- blocks host reads, snapshots, and writes while a prepared or ambiguous
  dispatch retains an allocation;
- rejects early release through the unchanged canonical runtime-model state;
- cancels only prepared work through the canonical pre-publication abort
  transition, releases its retained resources exactly once, and propagates the
  terminal failure to dependent dispatches;
- propagates failed dependencies without executing dependent kernels;
- invalidates every potentially written byte when completion becomes ambiguous,
  requires explicit queue quiescence before settling it, and never promotes
  those unknown bytes back to initialized state;
- distinguishes an injected unknown outcome from an expired host wait while
  treating both as published ambiguity; timeout does not claim cancellation or
  stopped execution; and
- refuses to quiesce a queue while it has prepared work, so teardown cannot
  strand a dispatch that the lifecycle model can no longer publish; and
- atomically replaces the entire runtime generation only after the old model
  admits cancellation, quiescence, ambiguous settlement, and complete resource
  teardown; the mandatory fresh identity makes every old handle foreign.

Semantic execution failure aborts the virtual dispatch before modeled
publication. This is possible because no physical device has observed the
request. `mark_completion_ambiguous` is the separate test boundary for a
modeled publication whose completion is unknown.

Every outcome is a **simulated observation**. This crate creates no compiler,
proof, artifact, load, launch, KFD, hardware, equivalence, performance, or
universal-correctness authority. Virtual ordering and CPU wall-clock duration
are not GPU scheduling or performance predictions.

## Headless operation

The companion integration command retains the existing hardened simulator
admission boundary:

```text
FE2O3_HIP_SYS_DISABLE=1 FE2O3_HSA_RUNTIME_DISABLE=1 \
  cargo run --locked -q -p fe2o3-virtual-runtime-cli --bin fe2o3-virtual-runtime -- \
  --kir-v7 crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir \
  --request crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json \
  --repeat 2
```

Use `--bundle PATH` instead of `--kir-v7 PATH` for a V1 simulation bundle.
Bundle admission selects its exact admitted `gfx942:xnack-` or `gfx950:xnack-`
semantic profile. Loose KIR defaults to `amdgpu64-target-neutral`; an exact
profile can be selected with `--target` and mismatched exact-target KIR fails
closed.

`--repeat` is limited to 256. Dispatch N depends on the observed completion of
dispatch N-1 and reuses the same persistent virtual allocations. The command
caps aggregate snapshots at 16 MiB and the encoded response at 48 MiB before
hex construction. Runtime configuration also caps arguments per dispatch and
aggregate retained dispatch-request storage. Success uses
`fe2o3-virtual-runtime-result-v1`; admission,
misuse, semantic, and response-bound failures use
`fe2o3-virtual-runtime-error-v1`. The lifecycle block identifies the runtime,
module, queue, allocations, dependency edges, completions, and terminal cleanup
states without exporting model-internal addresses.
`--fault early-release` exposes a bounded lifecycle misuse operation; it must
exit with a typed `resource_in_use` JSON error while the prepared dispatch
retains the allocation.

The command never compiles source. Ordinary Rust must first pass through the
sole `fe2o3-export-sim` compiler transaction described by
[Source-to-simulator bundle V1](simulation-bundle-v1.md). A hardware command
never falls back to this virtual runtime.

## Normal host interface

`fe2o3-sim-runtime` adapts exact V3 bundles to the ordinary
`RuntimeContextV1<SimRuntimeBackendV1>` API. Selection is explicit: construct a
`gfx942:xnack-` or `gfx950:xnack-` simulator backend, then use normal typed
allocation, copy, module, kernel, stream, launch, event, poll/wait, and release
operations. It neither probes a GPU nor silently falls back from hardware.

Module admission revalidates the complete V3 bundle and requires exact target,
semantic-MIR version, target-layout identity, root ABI identity, KIR identity,
kernel entry, local/type/ownership correspondence, and KIR value identity.
Address-free runtime bindings patch only the pointer slots authenticated by the
storage map. The current packing rules admit scalar and thin global pointers
with a direct ABI and global slices with the exact pointer/length pair ABI.
Adjusted, cast, indirect, aggregate/opaque, ambiguous, reordered, expanded,
and non-global forms fail typed admission. The adapter does not materialize an
ABI that V3 and KIR do not represent.

One dedicated CPU worker retains the virtual runtime state behind a fixed
64-command channel. Synchronous lifecycle commands wait for their response;
asynchronous submission uses nonblocking backpressure and rejects before
custody when the queue is full. Drop closes its sender before joining, so even
a full or disconnected queue terminates after its finite accepted work, while
each dispatch is bounded by the configured simulator limits. A worker exit
makes pending observation terminal and preserves resource custody.
The reported evidence is exactly `simulated=true`, `hardware=false`, and
`performance_prediction=false`. Multiple devices, peer copy, host collectives,
dynamic shared memory, and performance prediction are not advertised.

The adapter depends on `fe2o3-runtime` because that crate owns the public
backend SPI. Its policy-audited final ELF links no HIP, HSA, DRM, or GPU runtime
library, contains no GPU device path, and invokes no KFD path. Rust's worker
thread imports libc `dlsym` for pthread compatibility; the dedicated policy
continues to reject `dlopen`, GPU library names, and all other loader entry
points. This is not a claim that the Cargo closure omits the pure-Rust KFD
packages already owned by `fe2o3-runtime`.

## Issue 216 matrix

This matrix distinguishes implemented evidence from the issue's complete exit
criteria. It is not a closure claim.

| Contract | Current evidence | Status needed for exit |
| --- | --- | --- |
| C0 semantics/baseline | `fe2o3-kir-sim-capabilities` derives a stable, CI-checked owner-or-typed-rejection matrix from the exhaustive admitted operation/terminator surface and exact scalar support predicates for every simulator-facing profile; its declared, authority-free compact JSON is fixed at 4,698,338 bytes | Future KIR surface changes must extend the exhaustive classifier and matrix before the generic lane can pass |
| C1 scalar interpreter | Verified KIR admission, structured CFG/calls/scalars, canonical/seeded/replay schedules, bounded errors, generated wrapping-`i32` expressions, and a fixed V2 corpus covering every admitted fixed-width integer type, both target `index` widths, `bool`, exact finite additions for `f16`/`bf16`/`f32`/`f64`, multi-block CFG/block arguments/integer switch/internal calls, global memory, shared-view aliasing, and typed adversarial failures | Float rounding-edge/nonfinite matrices, casts, broader operation combinations, concurrency families, and universal coverage remain incomplete and are explicit typed exclusions |
| C2 typed memory/Rust | Allocation provenance, initialization, bounds, alignment, buffer views, plus V3 bundles carrying exact current-production semantic MIR and compiler-owned source-local/type/ownership to KIR storage correspondence. The typed-layout query reports rustc struct/tuple/array/enum layouts, direct/niche discriminants, payload fields, padding, initialization ranges, exact shared-backing overlap ranges, and request-local provenance for the admitted ordinary-Rust corpus | KIR V7 cannot represent by-value aggregate values. Aggregate construction/execution and value materialization remain typed unsupported or `opaque_flattened`; broad generated corpus and all scalar/memory families remain incomplete |
| C3 wave/workgroup | Wave32/Wave64 masks and selected collectives, LDS, barriers, integer atomics/fences, race/HB exploration | General divergence/reconvergence, matrix/MFMA, dynamic/general LDS, and remaining intrinsics stay typed unsupported |
| C4 virtual runtime | This crate and CLI cover bounded allocation/copy/queue/dependency/dispatch/completion plus pre-publication cancellation, typed timeout ambiguity, early-release, quiesced recovery, and atomic full-generation reset. `fe2o3-sim-runtime` additionally runs exact admitted V3 bundles through the normal typed `RuntimeContextV1` lifecycle, including event dependencies and output copyback, without GPU probing or fallback | #182-defined multi-device plans and KIR-unrepresentable aggregate materialization remain outside this single-device adapter |
| C5 debugger/agent | Bounded JSONL simulator debugger, semantic scopes, replay/reverse inspection, race/source evidence, direct V3 bundle admission, and a one-shot agent-readable typed layout/region query with hostile correspondence substitution rejection | Aggregate runtime value reconstruction is deliberately absent until KIR represents it; end-to-end seeded reduction and all requested query families are not complete |
| C6 differential | The bounded V1/V2 harnesses bind exact case, KIR, expected-output, observed-output, and rejection sequences; emit reduced machine-readable diagnostics; require exact seed/case/KIR identity for replay; and expose an agent-readable capability/exclusion query | No exact simulator-versus-physical KFD matrix covers every supported semantic family; the V2 report is CPU model agreement only |

The initial acceptance cases currently have these honest dispositions:

| Case | Disposition |
| --- | --- |
| 1 no device | Source export and supported KIR simulation work without GPU libraries; general public hardware execution remains intentionally unavailable |
| 2 per-lane memory fault | Implemented for the admitted simulator/debugger subset |
| 3 divergent barrier | Implemented with bounded participant/phase diagnosis |
| 4 wave collective | Partial: admitted ballot/any/all/shuffle and mask failures exist; the issue's broader reduction/scan requirement is not complete |
| 5 race exploration | Implemented as bounded seeded schedule evidence, not race-freedom proof |
| 6 virtual runtime misuse | Implemented here for early release and ambiguous completion with canonical model transitions; released identity-bound handles fail as stale |
| 7 differential replay | Open: no general exact physical-KFD differential pipeline |
| 8 unsupported operation | Implemented as typed rejection; unsupported operations are never approximated |

Therefore issue #216 cannot yet truthfully close on this milestone alone. The
remaining C1-C3/C5 semantic breadth, C6 physical
differential qualification, and #182-dependent multi-device work remain
independent gates.

## Qualification

The focused no-GPU commands are:

```text
cargo test --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli
cargo test --locked -p fe2o3-sim-runtime
cargo test --locked -p fe2o3-kir-sim --test capability_matrix
cargo run --quiet --locked -p fe2o3-kir-sim --bin fe2o3-kir-sim-capabilities
cargo test --locked -p fe2o3-sim-differential
cargo test --locked -p fe2o3-kernel-ir simulation_bundle_v3
cargo test --locked -p rustc-codegen-fe2o3 --test production_ranked_bounds_driver_v1 ordinary_rust_exports_and_queries_exact_v3_typed_layouts_and_regions -- --ignored --exact
cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- --seed-start 0 --cases 256
cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- semantic-capabilities-v2
cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- semantic-run-v2 --seed 0
cargo clippy --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli -p fe2o3-sim-runtime -p fe2o3-sim-differential -p fe2o3-debug-cli -p fe2o3-kernel-ir --all-targets -- -D warnings
cargo doc --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli -p fe2o3-sim-runtime -p fe2o3-sim-differential -p fe2o3-debug-cli -p fe2o3-kernel-ir --no-deps
bash scripts/ci-local.sh workspace-policy
bash scripts/ci-local.sh runtime-policy
```

The packages are also members of the generic CPU test list. Runtime policy
audits the syscall-free virtual-runtime roots and scalar differential command
against an exact package/source allowlist. It separately audits the normal
simulator adapter through the runtime pure-Rust policy and its example ELF.
Those policies reject HIP, HSA, GPU shared libraries, process spawning, and
runtime library loading; the worker policy permits only libc `dlsym` required
by `std::thread`, while still rejecting `dlopen`. The syscall-free policy
additionally rejects DRM/KFD packages and device paths. This
is a dependency and linked-binary policy check, not a general proof that
arbitrary future source cannot issue a syscall; source and review controls
remain part of the trust boundary.
