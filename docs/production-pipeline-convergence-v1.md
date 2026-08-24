# Production compiler convergence V1

This document defines the implementation shape for
[#175](https://github.com/harsh-nod/fe2o3/issues/175). It narrows the compiler
work under [#134](https://github.com/harsh-nod/fe2o3/issues/134) to one
production route. Existing scalar, GEMM, attention, collective, and MoE routes
remain qualification oracles while their behavior migrates. They are not
additional production architectures.

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

| Boundary | Current state through `f001409c70` | Remaining production wiring |
|---|---|---|
| Protected release and Cargo broker | The release contract validates `CompilerClosureV2`; the broker transfers a sealed raw closure capability to the binding wrapper. | Preserve that admitted closure through every later protected boundary. |
| Exact rustc invocation | The wrapper constructs and seals V3 for protected captures and installs its exact immutable image at fd 199 for rustc. V2 capture remains for compatibility and receives no fd 199 capability. | Admit V3 inside the backend and compare its argv, cwd, complete environment, role-specific pins, target, and full closure with the live process. |
| Compiler module handoff | Closure-bound V2 publish/consume records and APIs exist on the shared V1/V2 handoff engine. | Switch the protected producer and consumer call sites from V1 to V2. |
| Worker publication restart | Closure-bound V2 persist/recover/clear records and APIs exist on the shared V1/V2 publication-intent engine. | Wire V2 into the protected publication and restart call sites and their restart-marker state. |
| Compatibility | Frozen V1 closure, invocation, handoff, and publication-intent surfaces remain available; current production call sites still use V1 where noted above. | Migrate callers explicitly without changing V1 wire formats or silently upgrading V1 records. |

The broker-to-wrapper raw closure capability is not the rustc invocation
capability and stops at the wrapper. Until backend V3 admission and the
protected V2 handoff and restart call sites are connected, the sealed transport
is coordination evidence, not end-to-end compiler provenance or production
readiness.

The implementation uses one move-only typestate owner, conceptually
`ProductionCompilationV1<'tcx, Stage>`. A transition consumes the previous
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
    <- ProductionCompilationV1 typestate transaction
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
migration bridges. They cannot be called by `ProductionCompilationV1`, and
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

Production will use one selector backed by the existing `PlironV1` compiler
contract. The rustc backend may expose the user-facing name `production-v1`,
but that name identifies the single transaction, not a workload.

Migration follows these rules:

1. Add no new workload-specific `CodegenPipeline` production variant.
2. Move exact-profile entry points behind qualification-oracle tests or tools.
3. Migrate a semantic slice only after ordinary attributed Rust passes the
   general route and differential tests match its existing oracle.
4. Once a slice migrates, remove its production selector immediately.
5. For a kernel-containing crate, unsupported production behavior is terminal.
   `legacy-v1` and exact-profile selectors are never fallbacks.
6. Host-only Rust code may continue through rustc LLVM; that is not a device
   compiler route.
7. After all slices migrate, remove `Legacy`, `PlironShadow`, and exact-profile
   selection from production device compilation. Shadow/oracle execution may
   remain in test-only tooling without candidate authority.

The default switched to `production-v1` after the first scalar slice completed
its compile, host-interface, artifact, and hardware gates. An incomplete
general route now fails closed instead of silently entering legacy codegen;
every retained oracle route requires an explicit selector.

The 2026-08-20 compiler review made this distinction structural. Selector names
now come from one table, every route has an explicit production-or-oracle
purpose, and a test proves that only `production-v1` is production-capable.
Shared oracle collection and frontend-record validation do not weaken the
boundary: `ProductionCompilationV1` still receives only the move-only production
closure and cannot call the oracle helper. See
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

For each slice, the existing route becomes a differential oracle. Tests compare
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
   use the same route with source-spanned rejection and hostile tests.
4. **Rust and verification:** #106 evidence is consumed by the generic #174
   MIR-to-KIR receipt; no profile-selected semantic replacement remains.
5. **Parameterized GEMM:** ordinary attributed Rust GEMM reaches inspected
   HSACO through the general route; #173 remains only an oracle.
6. **Selector convergence:** all exact-profile production selectors are gone,
   default kernel compilation uses the one transaction, and unsupported code
   fails without fallback.

No milestone changes a parity row until its protected evidence policy and
hardware gates independently qualify that row.
