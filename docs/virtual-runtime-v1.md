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
- propagates failed dependencies without executing dependent kernels; and
- invalidates every potentially written byte when completion becomes ambiguous,
  requires explicit queue quiescence before settling it, and never promotes
  those unknown bytes back to initialized state; and
- refuses to quiesce a queue while it has prepared work, so teardown cannot
  strand a dispatch that the lifecycle model can no longer publish.

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

## Issue 216 matrix

This matrix distinguishes implemented evidence from the issue's complete exit
criteria. It is not a closure claim.

| Contract | Current evidence | Status needed for exit |
| --- | --- | --- |
| C0 semantics/baseline | `fe2o3-kir-sim-capabilities` derives a stable, CI-checked owner-or-typed-rejection matrix from the exhaustive admitted operation/terminator surface and exact scalar support predicates for every simulator-facing profile; its declared, authority-free compact JSON is fixed at 4,698,338 bytes | Future KIR surface changes must extend the exhaustive classifier and matrix before the generic lane can pass |
| C1 scalar interpreter | Verified KIR admission, structured CFG/calls/scalars, canonical/seeded/replay schedules, bounded errors, plus generated wrapping-`i32` cases translated into KIR and compared lane-by-lane with an independent evaluator; failures are reduced deterministically within their typed failure class and encoded responses fail closed above 1 MiB | Differential coverage of the remaining scalar widths, floating-point modes, structured control flow, calls, and memory families remains incomplete |
| C2 typed memory/Rust | Allocation provenance, initialization, bounds, alignment, buffer views, source/KIR diagnostics for the admitted subset | Aggregate/enum/layout coverage and complete conformance-corpus qualification remain incomplete |
| C3 wave/workgroup | Wave32/Wave64 masks and selected collectives, LDS, barriers, integer atomics/fences, race/HB exploration | General divergence/reconvergence, matrix/MFMA, dynamic/general LDS, and remaining intrinsics stay typed unsupported |
| C4 virtual runtime | This crate and CLI cover bounded allocation/copy/queue/dependency/dispatch/completion plus early-release and ambiguous-completion recovery | Normal generated host-interface integration, cancellation/timeout/reset, and #182-defined multi-device plans remain open |
| C5 debugger/agent | Bounded JSONL simulator debugger, semantic scopes, replay/reverse inspection, race and source evidence | End-to-end seeded reduction and all requested query families are not complete |
| C6 differential | The bounded generated wrapping-`i32` harness binds exact case, KIR, and output sequences and emits reduced machine-readable reproducers; hardware tests remain separate evidence | No exact simulator-versus-physical KFD matrix covers every supported semantic family |

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
remaining C1-C3/C5 semantic breadth, generated-host integration, C6 physical
differential qualification, and #182-dependent multi-device work remain
independent gates.

## Qualification

The focused no-GPU commands are:

```text
cargo test --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli
cargo test --locked -p fe2o3-kir-sim --test capability_matrix
cargo run --quiet --locked -p fe2o3-kir-sim --bin fe2o3-kir-sim-capabilities
cargo test --locked -p fe2o3-sim-differential
cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- --seed-start 0 --cases 256
cargo clippy --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli -p fe2o3-sim-differential --all-targets -- -D warnings
cargo doc --locked -p fe2o3-virtual-runtime -p fe2o3-virtual-runtime-cli -p fe2o3-sim-differential --no-deps
bash scripts/ci-local.sh workspace-policy
bash scripts/ci-local.sh runtime-policy
```

The packages are also members of the generic CPU test list. Runtime policy
audits both virtual-runtime roots and the scalar differential command against
an exact package/source allowlist, rejects HIP, HSA, DRM, and dynamic-loader
authority, and scans the final ELF for device paths as defense in depth. This
is a dependency and linked-binary policy check, not a general proof that
arbitrary future source cannot issue a syscall; source and review controls
remain part of the trust boundary.
