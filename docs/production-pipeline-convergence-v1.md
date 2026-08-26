# Production compiler convergence V1

This document defines the implementation shape for
[#175](https://github.com/harsh-nod/fe2o3/issues/175). It narrows the compiler
work under [#134](https://github.com/harsh-nod/fe2o3/issues/134) to one
production transaction. Existing scalar, GEMM, attention, collective, and MoE
implementations remain qualification oracles while their evidence migrates.
They are not additional production architectures.

## One transaction

The completed convergence target sends every kernel-containing final crate
through one rustc-owned transaction:

```text
authenticated rustc kernel closure
    -> canonical semantic MIR
    -> owner-authenticated executable MIR graph
    -> canonical target-neutral Kernel IR
    -> owner-authenticated executable Kernel/GPU graph
    -> typed AMDGPU legalization
    -> canonical typed LLVM handoff
    -> pinned upstream LLVM and in-process LLD worker
    -> independently inspected AMDHSA artifact
    -> generated typed host interface
```

`cargo fe2o3 build` and `cargo fe2o3 run` realize that transaction with one
fixed orchestration plan. Cargo first performs a device `build` for the fixed
AMDGPU target under the protected compiler closure. Only after the exact
generated-artifact generation commits does a fresh Cargo process build or run
the same package/feature/profile selection for the pinned host target using
ordinary rustc. Run payload arguments are forwarded only to that host process.
The caller cannot pass `--target`, and the host phase receives no fe2o3 backend,
wrapper, broker, device, build-manifest, qualification, or simulation controls.
This is phase separation inside one production build, not two compiler routes.

Compiler provenance is one cross-cutting input to this transaction, not a
second compiler route. The canonical `CompilerClosureV2` commits to six
role-specific SHA-256 pins:

1. Cargo executable;
2. static Cargo binding trampoline;
3. full `cargo-fe2o3` binding wrapper;
4. rustc executable;
5. complete rustc runtime tree; and
6. selected rustc codegen backend.

The closure also commits to the canonical Cargo-to-trampoline-to-wrapper
transition protocol, currently
`CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1`, and derives one aggregate
identity from the domain, protocol version, and ordered pins. The aggregate is
validated, not an independently trusted seventh pin.

`RustcInvocationDescriptorV3` is exactly one complete
`RustcInvocationDescriptorV2` process description, including cwd, final argv,
and complete sorted child environment, plus the complete canonical
`CompilerClosureV2` preimage. Construction cross-checks the duplicated rustc
and backend digests.

### Compiler provenance wiring

| Boundary | Current state | Remaining production wiring |
|---|---|---|
| Protected release and Cargo broker | The release contract validates `CompilerClosureV2`; the broker transfers a sealed raw closure capability to the binding wrapper. | Preserve the admitted closure through runtime authorization. |
| Exact rustc invocation | The wrapper constructs and seals V3 for production, installs its immutable image at fd 199, retains parent custody, and the backend revalidates argv, cwd, environment, target, role pins, and closure before V3 publication. Qualification V2 captures receive no fd 199 capability and are not retained as production custody. | Extend archived end-to-end evidence across the final application process boundary. |
| Compiler module handoff | Production has one mandatory protected-custody path and one V3 publication/consumption transaction. Before monomorphization, a device transaction must retain one admission containing both the authenticated gfx942 target and exact managed build attempt; preflight roots and post-monomorphization device work must agree exactly. The attempt and protected rustc invocation then move as one publication custody value, so no optional or late direct-publication branch remains. The ordinary publication branch and runtime schema selector are deleted. | Keep V1/V2 consumers confined to explicit qualification code until their oracles retire. |
| Worker publication restart | `ManagedProductionBuild` has only `Fresh`, `Recovered`, and `Ready` states. It performs strict V3 preflight, one-shot consumption, direct LLVM/LLD execution, independent inspection, durable publication, and load-readiness recovery. | Join generated host interfaces and runtime authorization to the recovered production artifact. |
| Application handoff | Production admits only the canonical Worker V3 load envelope. `cargo fe2o3 run` requires the authorized locked compiler closure, injects the required-envelope marker directly, rejects intermediate Cargo runners, and has no compiled no-envelope fallback. The neutral `fe2o3-runtime-protocol` crate owns the load-envelope custody transition, application handoff wire, and sealed static-application identity used by Cargo and the host. Cargo pins the application and V3 identity, binds the envelope, artifact directory, and ACK descriptors into a fresh occurrence, validates the challenge-bound ACK, and retains the current-publication lease through application exit. Feature-free `fe2o3-host` exposes one `load_inherited_worker_v3_application_v1` transaction that performs exact descriptor recovery, reviewed compiler/Verus authentication, environment authorization, exact HSA load, and compiler-generated typed dispatch without returning intermediate authority. The raw HIP `launch!`, module/function, parameter-pack, launch-configuration, and launch-function route is qualification-only across `fe2o3-host` and `fe2o3-core`. Shared descriptor failures use the neutral `ApplicationDescriptorHandoffErrorV1`; the old Worker V2 error name is a qualification-gated alias. | Supply production verifier implementations and generated dispatch applications whose authenticated proof/effect evidence closes the kernel-specific obligations. |
| Qualification isolation | Backend workload oracles and extraction drivers require `qualification-oracles-test-only`. The feature-free backend compiles a dedicated workload-neutral `production_worker_handoff`; the legacy Worker V2 producer, S09 identity model, scalar MIR V2 model, and semantic type V2 adapter are absent from its module graph. Cargo V1/V2 work state, restart modules, workload parsers, and V2 intake require the same package-local feature, but Cargo application transfer is V3-only in every build: V2 decode, lease recovery, child environment, challenge, and ACK branches are deleted, and stale V2 names are recognized only for rejection before spawn. The feature-free normal dependency graphs of both `cargo-fe2o3` and `fe2o3-host` exclude `fe2o3-worker-v2-bundle`; they depend on `fe2o3-runtime-protocol` for production records. Host V2 application recovery, bundle admission, prerequisite authentication, HSA loading, launch metadata, the legacy generic HIP module/function launch stack, workload-specific generated adapters, the exact FlashAttention, LDS GEMM, wave64 collectives, workgroup synchronization, MoE, protected row-softmax, and raw gfx942 OCML receipt lifecycles, and the embedded vecadd artifact API require the package-local qualification feature. General typed macro expansion, including exact vecadd and Scalar GEMM, emits workload-neutral Worker V3 host capabilities unless an oracle fixture explicitly requests `qualification_worker_v2`; empty Rust slices are handled by the generic capability instead of selecting a GEMM-only wrapper. Feature-free Cargo prepares a concrete `PreparedProductionBuildConfig` through `prepare_production_managed_attempt` and receives one mandatory `ManagedProductionBuild` from the shared V3 recovery helper. Completion consumes that transaction exactly once; it has no optional empty-work state or missing-custody branch. `PreparedManagedWork`, optional production or qualification work, `ManagedQualificationWork`, Worker V1/V2 recovery decisions, and empty managed attempts compile only in the qualification harness. Source-debug executable and process measurement is likewise absent from feature-free compilation. Host-only dependency units use rustc's built-in LLVM backend and receive no fe2o3 route selector, managed compiler arguments, backend descriptor, or artifact custody. | Delete the now-unreferenced host Worker V2 application consumer, complete the Worker V3 application verifier/dispatch join, and then delete each remaining qualification oracle. |

The feature-free Cargo path also resolves its build-configuration API directly
to `PreparedProductionBuildConfig`, with no feature-dependent compatibility
type alias and no no-op qualification conversion methods. It parses only
`FE2O3_PRODUCTION_BUILD_CONFIG_V1`; the profile enum, Worker V2 schema parser,
envelope controls, source-debug controls, and workload fields are not compiled.
Qualification-enabled tests retain their multi-profile parser, but production
manifest cases call the same dedicated production parser used by the release
binary. The release path always uses the production expected-identity namespace
and ordinary compiler-capability profile; route-dependent identity and S09
selection logic is compiled only into the qualification harness. The
feature-free binding wrapper admits every managed kernel root through protected
rustc and requires compiler-closure custody directly; it does not compile the
qualification-oracle predicates or qualification command preparation path.
The feature-free Cargo driver also binds the fixed gfx942 target profile and
production semantic-generation identity directly. Its backend preparation
context has no production-route boolean and compiles no simulation selection;
those controls exist only in the qualification harness.
Once a compile is selected as a production kernel root, the binding wrapper
requires the concrete production manifest before it can begin an artifact
attempt. Production preparation and completion contain no Worker V1/V2,
in-rustc oracle, simulation, row-softmax, or empty-attempt dispatch. The
qualification harness calls the same V3 recovery/preparation helper when it
tests production behavior, so recovery semantics still have one owner.
Production capability intake also releases the broker's one-shot invocation
authority immediately after authenticating the transfer. The release
`CompilerCapabilities` shape has no retained invocation-authority field or
child-inheritance API; those compile only for the row-softmax qualification
oracle. The S09 broker profile and pinned-Cargo transfer image are likewise
absent from feature-free compilation. Shared closure, backend, Cargo-image,
and artifact validation remains implementation-neutral and runs before either
constructor receives custody.

The feature-free rustc backend likewise does not compile
`QualificationSelection`, `SelectedQualificationOracle`, or
`RustcInvocationPolicy`.
It captures a selector-free production environment preflight, enters protected
V3 rustc admission directly, and requires the production device transaction to
complete directly for every discovered kernel. The qualification-feature build
has an optional non-publishing oracle token and an invocation-policy enum for
differential testing, but no compiler-route enum or release implementation
choice.

The `cargo fe2o3 simulate` command is also oracle-only and is absent from
feature-free command dispatch and help. Production `build` and `run` cannot
select it implicitly.

The host-consumer and shared hostile application fixtures accept only V3
inputs. The old V2 consumer binary, input adapter, Cargo feature, and hostile
fixture protocol implementation are deleted. All application-boundary
adversarial coverage now runs against the strict V3 path in generic-core CI;
the V2 vertical retains only publication, restart, and explicit rejection
oracles.

Version suffixes remain on serialized records, identity domains, receipts, and
external protocol types. Private production methods and states are unversioned
because there is only one implementation. A new production schema must be an
explicit migration of the same transaction, never a selectable pipeline.

The implementation uses one move-only typestate owner, conceptually
`ProductionCompilation<'tcx, Stage>`. A transition consumes the previous
stage and returns the next. The owner retains:

- the active compiler session and #140-authenticated graph handles;
- the canonical record and identity at each completed semantic boundary;
- bounded before/after transformation receipts;
- source, ABI, layout, target, proof-obligation, and diagnostic provenance;
- the exact Worker request, response, finalized bytes, and inspection owner;
- no publication, load, launch, or runtime authority.

The owner may retain several graph handles in one session, but a semantic fact
has one authoritative representation at a stage. Side data is permitted only
when the graph cannot yet represent the fact. The graph and side data are
compared at every boundary, and the side-data field has a named removal issue.

## Entry convergence

The explicit extraction driver and compatibility codegen backend must both call
one importer:

```text
import_rustc_kernel_closure_v1(tcx, collected_roots, limits)
    -> OwnerControlledSemanticMirV1
```

The importer is workload-neutral. It discovers roots from authenticated
`#[kernel]` metadata and rustc identities, traverses the complete reachable
monomorphized device closure, and records typed rustc-independent semantics.
It never branches on an export name, source substring, workload identity, or
exact MIR transcript.

The imported representation must preserve the facts needed by later lowering:

- source spans and expansion/call-site origins;
- item, instance, generic, and const-generic identities;
- layouts, FnAbi modes, calling convention, unwind behavior, and relocations;
- locals, types, places, projections, operands, constants, assertions, drops,
  volatility, atomics, direct calls, tail calls, and control-flow edge meaning;
- pointer provenance, address-space requirements, and source-level capabilities;
- deterministic call chains for unsupported reachable behavior.

Ordinary scalar admission and General GEMM refinement consume this same owner.
They do not run separate importers. The current #174 work is accepted only when
generic capture is independent of ordinary-scalar authentication and scalar
lowering is a separate consuming adapter.

### Rustc and device target custody

The current compatibility backend analyzes the final crate in a host rustc
session while `cargo-fe2o3` separately configures the device compiler for
`gfx942`. These are two different target facts. Host-session layout and FnAbi
must never be relabeled as AMDGPU layout or FnAbi merely because device lowering
was selected.

Production collection therefore retains both the exact rustc layout context
and the fixed `gfx942:xnack-` device profile in one move-only token. The
semantic importer must consume that pair and fail closed on an unsupported
bridge. The intended convergence is for the explicit extraction driver and the
compatibility backend to enter the same importer under an AMDGPU rustc target
session; the compatibility backend may become a thin coordinator for that
session. Existing host-to-gfx942 conservative layout projections remain
qualification inputs and cannot mint production semantic identity. Until the
AMDGPU-session handoff exists, production stops before semantic-MIR admission.

## Canonical and executable IR

`fe2o3-mir-model` and `fe2o3-kernel-ir::Module` remain the canonical semantic
identity boundaries. Pliron operations are transient executable state.

The general Kernel IR module already represents functions, roles, signatures,
blocks, SSA values, control flow, memory effects, address spaces, barriers,
atomics, wave operations, matrix operations, capabilities, and inline assembly.
Profile records may validate or construct regression fixtures, but production
MIR lowering must emit the general module rather than select a profile-specific
replacement.

Conversion between canonical records and executable graph state is checked in
both directions. Identity never includes text rendering, traversal accident,
arena slot, pointer, process ID, or filesystem path. Frozen wire formats remain
byte compatible.

## Transformations

All mutable transformations execute through the sealed #140 service. A pass
receives an owner-authenticated operation handle and a bounded configuration;
it cannot receive or return a raw Pliron pointer.

### Session dependency boundary

The sealed service requires a dependency inversion around the dialect crates.
It must not be implemented as a public callback that receives `&mut Context`:
safe callback code could retain a contextless upstream `Ptr<T>` and recreate
the cross-session confusion that #140 is intended to remove.

The production dependency direction is:

```text
Pliron owner/registration core
    <- fe2o3 dialect definitions and typed constructors
    <- closed production Pliron session and transform adapters
    <- ProductionCompilation typestate transaction
```

The lower owner core contains context identity, bounded dialect-registration
actions, opaque handle mechanics, and fixed diagnostics. Dialect crates depend
only on that core and pinned Pliron APIs. The closed production-session layer
depends on the owner core plus the admitted dialect crates, owns the raw
`Context`, and directly invokes their typed constructors and transformations.
Its raw-context implementation is compiler-internal TCB code; it exposes no
callback, trait implementation point, context, pointer, value, block, type, or
attribute handle to callers.

Construction consumes a bounded canonical MIR or Kernel IR recipe and returns
an opaque root handle only after recursive verification and canonical
cross-checking. Transformation selection is a closed fe2o3-owned operation,
not an arbitrary caller-provided Pliron `Pass`. A transition consumes the
input-stage capability, reserves its complete work and growth budget, mutates
only the authenticated tree, recursively verifies the result, and returns a
new stage capability plus a canonical receipt. Any failure after allocation or
mutation begins poisons and terminally consumes the production session.

The current textual import and detached lowering services remain test and
migration bridges. They cannot be called by `ProductionCompilation`, and
removing their final production callers is part of #140/#178 rather than a
second compiler route.

Each pass performs this transaction:

1. Validate the complete input graph and canonical binding.
2. Reserve bounded work, diagnostics, graph growth, and nesting.
3. Apply one independently specified transformation deterministically.
4. Validate the complete output graph and declared analysis preservation.
5. Emit a receipt binding pass, input, output, resource use, and diagnostics.
6. Poison the session after a failure that may have partially mutated state.

The initial pass order is deliberately conservative:

1. unreachable control-flow removal and branch simplification;
2. eligible local-storage promotion to SSA;
3. constant propagation and folding;
4. dead value, operation, argument, helper, and symbol elimination;
5. equivalent pure-computation reuse;
6. bounded aggregate decomposition and helper integration;
7. loop normalization and explicitly bounded unrolling;
8. address-space refinement and memory-effect analysis;
9. uniformity, divergence, barrier, and synchronization validation;
10. ABI preparation and target-independent call lowering.

No optimization is required for semantic correctness. A pass may reject or
leave code unchanged, but it may not select an old compiler route.

## Proof and verification

Verification does not select compiler implementation. The generic MIR-to-KIR
receipt binds an operation correspondence between one retained MIR owner and
one canonical Kernel IR module. A workload-specific Verus proof may discharge
obligations for that module, but it cannot replace MIR or Kernel IR or provide
artifact authority.

The #106 General GEMM proof is therefore the first substantial producer of a
generic MIR-to-KIR correspondence receipt. The #174 consumer retains that
receipt with the same MIR owner. Later kernels use the same receipt type and
relation vocabulary with different proved obligations.

Source proof, compiler transformation validation, LLVM/ISA correspondence,
machine inspection, hardware observation, and runtime authority remain
separate evidence classes.

## AMDGPU and finalization

One AMDGPU lowering owner centralizes exact target identity, features, wave
policy, address spaces, device libraries, code-object policy, resource bounds,
kernel metadata, calling conventions, and module flags. Textual LLVM is a
bounded Worker transport and inspection form, not a semantic identity boundary.

The production finalizer returns a generic move-only inspected-artifact owner.
It retains and freshly revalidates the exact compiler graph/handoff, Worker,
finalized bytes, descriptor, ELF, metadata, target, and ISA observations. The
current General GEMM owner chain under #173 is a qualification oracle for this
generic boundary. Its three late-machine axes must not become a second
GEMM-only authority path.

Generated host interfaces derive from the canonical Kernel IR ABI and are
checked against the inspected descriptor. They still grant no launch authority;
the runtime separately validates allocation, lifetime, launch geometry, and
device compatibility.

## Selector retirement

Production has no selector. An unset qualification-oracle environment enters
the sole production transaction; `production-v1` is rejected if supplied as
either an obsolete pipeline value or an oracle name. Versioned `V1`/`V2`/`V3`
suffixes identify frozen records and protocols, not selectable implementations.
Production build inputs use only `FE2O3_PRODUCTION_BUILD_CONFIG_V1` with the
`fe2o3-production-build-config-v1` schema. Worker V2 config, expected-identity,
envelope, and source-debug controls are qualification-only and cannot be mixed
with that production namespace. The qualification build carries an optional
non-publishing oracle token; there is no production-or-qualification route
enum that could acquire another production implementation.

Migration follows these rules:

1. Add no workload-specific production implementation or selector.
2. Move exact-profile entry points behind qualification-oracle tests or tools.
3. Migrate a semantic slice only after ordinary attributed Rust passes the
   production transaction and differential tests match its existing oracle.
4. Once a slice migrates, keep the old implementation only as a qualification
   oracle until its differential coverage is no longer needed.
5. For a kernel-containing crate, unsupported production behavior is terminal.
   `legacy-v1` and exact-profile selectors are never fallbacks.
6. Host-only Rust code may continue through rustc LLVM; that is not a second
   device compiler implementation.
7. Keep non-authoritative comparisons only in qualification tooling. The
   compiler API has no implementation selector, and exact-profile qualification
   oracles retire as their differential coverage migrates.

Production became the sole unselected compiler transaction after the first scalar slice
completed its compile, host-interface, artifact, and hardware gates. It has no
selector. An incomplete production transaction now fails closed instead of silently
entering legacy codegen. Retained oracles are absent from feature-free Cargo
and backend builds. Each requires the package-local
`qualification-oracles-test-only` feature and an explicit
`FE2O3_QUALIFICATION_ORACLE_V1` value. Unselected host-only dependency units do
not use this marker: the wrapper omits fe2o3's managed rustc arguments and
backend descriptor so rustc uses its built-in LLVM backend directly.

The 2026-08-20 compiler review made this distinction structural. Qualification
names come from one feature-gated table, while production has no corresponding
variant or selector. The backend has one protected publication call, Cargo has
one production intake without a schema selector, and production recovery is a
separate state machine from V1/V2 qualification recovery. Shared oracle
collection and frontend-record validation do not weaken the boundary:
`ProductionCompilation` still receives only the move-only production closure
and cannot call the oracle helper. See
`compiler-convergence-review-2026-08-20.md` for the deletion inventory and
remaining complexity bounds.

## Migration order

The vertical slices migrate through the same transaction in this order:

1. fill and vector arithmetic;
2. scalar arithmetic, branches, and structured control flow;
3. loops, helpers, cross-crate generic and const-generic calls;
4. multiple kernels in one final crate;
5. global, private, and workgroup memory;
6. barriers, one wave operation, and scoped atomics;
7. scalar GEMM and parameterized tiled GEMM;
8. reductions, softmax, attention, and MoE.

For each slice, the old implementation becomes a differential oracle. Tests compare
canonical MIR, Kernel IR, ABI, artifact structure, numerical results, canaries,
synchronization behavior, and terminal cleanup before its selector is removed.

## Active issue alignment

| Issue | Role in the one production pipeline |
|---|---|
| #140 | Owner-authenticated graph handles and sealed transformation execution |
| #174 | Workload-neutral same-session MIR owner; General GEMM is its first demanding consumer |
| #106 | First mechanically checked producer of the generic MIR-to-KIR correspondence |
| #145 | Typed general AMDGPU to LLVM construction, not artifact authority |
| #146 | Pinned upstream LLVM and in-process LLD Worker consumer |
| #147 | Differential and hostile qualification for the LLVM/Worker boundary |
| #173 | General GEMM oracle for retained compiler/Worker/finalizer ownership and late-machine binding |
| #175 | Production transaction integration, migration order, and selector retirement |
| #176 | One workload-neutral rustc semantic MIR importer for both entry paths |
| #177 | Canonical semantic MIR to general Kernel IR lowering |
| #178 | Owner-authenticated deterministic middle-end transformations |
| #179 | Generic retained finalization and inspected AMDHSA artifact owner |
| #180 | Typed host-interface generation from canonical KIR and inspected ABI |
| #181 | Differential migration and exact-profile selector retirement |

## Parallel implementation lanes

Work remains parallel only at frozen ownership boundaries:

| Lane | Primary write ownership | Exit criterion |
|---|---|---|
| Session safety | `fe2o3-pliron`, owner-handle tests | #140 sealed pass execution with poisoning and receipts |
| Rust import | `fe2o3-mir-model`, `dialect-mir`, rustc importer module | both rustc entry paths return the same generic MIR owner |
| MIR to Kernel IR | `fe2o3-lower-mir-kernel`, correspondence tests | general `KernelModule` for the first scalar/control-flow slice |
| Kernel/GPU passes | dialect and lowering services | deterministic checked pass sequence over owner handles |
| AMDGPU/LLVM | `fe2o3-amdgcn-model`, `fe2o3-lower-amdgcn-llvm` | complete typed target contract and canonical handoff |
| Worker/finalizer | Worker handoff and `fe2o3-hsaco-finalize` | generic retained inspected-artifact owner |
| Host/runtime | generated host and protected runtime adapters | ABI/descriptor agreement and one-shot checked launch |
| Migration/oracles | integration tests, scripts, evidence docs | each old selector removed after differential hardware gates |

Shared root manifests, exports, selectors, and the production transaction are
owned by the integrator. Lane changes merge only after their canonical records
and hostile fixtures are frozen, preventing parallel work from creating new
routes.

## Critical milestones

1. **Compiler middle end:** #140 pass execution plus one importer, general
   Kernel IR module, and deterministic scalar/control-flow transformations.
2. **First production slice:** attributed scalar/control-flow Rust reaches an
   inspected gfx942 artifact and generated host interface through only the
   production transaction.
3. **Safety semantics:** memory, barriers, wave operations, and scoped atomics
   use the same transaction with source-spanned rejection and hostile tests.
4. **Rust and verification:** #106 evidence is consumed by the generic #174
   MIR-to-KIR receipt; no profile-selected semantic replacement remains.
5. **Parameterized GEMM:** ordinary attributed Rust GEMM reaches inspected
   HSACO through the production transaction; #173 remains only an oracle.
6. **Selector convergence:** all exact-profile production selectors are gone,
   default kernel compilation uses the one transaction, and unsupported code
   fails without fallback.

No milestone changes a parity row until its protected evidence policy and
hardware gates independently qualify that row.
