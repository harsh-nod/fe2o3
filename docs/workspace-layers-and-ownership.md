# Workspace Layers and Parallel Ownership

Status: normative Wave 1 workspace policy, updated through the 2026-08-19
bounded Pliron scalar-add checkpoint. Issues
[#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) remain open. The landed
crates make both epics infrastructure-enabled and complete one exact scalar
compile/finalize/execute slice; they do not implement a general production
Pliron pipeline or persistent GPU execution.

This policy refines the [Pliron Wave 0 architecture](pliron-wave0-architecture.md)
and preserves the existing production compiler, artifact, proof, publication,
and runtime behavior. The words MUST, MUST NOT, SHOULD, and MAY are normative.

## Purpose

Issues #134 and #135 introduce several independently implementable compiler,
runtime, and verification components. They must be able to land alongside
ordinary kernel, backend, runtime, evidence, and documentation work without
making the root manifest, compiler selector, or existing monoliths permanent
merge bottlenecks.

The workspace is therefore organized around stable architectural layers, not
issue numbers. A crate has one owning layer. Dependencies between crates must
respect the checked-in policy in
`scripts/workspace-dependency-policy.json`, which is enforced from generic CI
by `scripts/workspace_dependency_policy.py`.

## Workspace Topology

Immediate crates are workspace members through `crates/*`. Nested test fixtures
and all example packages remain explicit members because their locations and
membership are part of existing compile-fail, integration, and tutorial test
contracts.

Adding an immediate crate under `crates/` automatically makes it a workspace
member. The same change MUST add the package name to exactly one layer in the
dependency policy unless that package name is already reserved there. An
unclassified member fails generic CI. New nested fixtures and examples MUST
remain explicit root members and use a declared fixture path prefix.

The root `Cargo.toml`, `Cargo.lock`, layer policy, shared dependency versions,
dialect registration, production pipeline selector, and release composition
are integration-owned files. Feature agents SHOULD NOT edit them. A dependency
addition that changes `Cargo.lock` is a separate, integration-reviewed commit.

## Layers

### Canonical contracts

Canonical contracts own versioned records, stable identities, wire encodings,
public compiler/host interfaces, target descriptions, and Pliron-independent
models. The current boundaries include `fe2o3-mir-model`,
`fe2o3-compiler-api`, `fe2o3-proof-contracts`, `fe2o3-service-model`,
`fe2o3-host-api`, `fe2o3-amd-target`, and the existing artifact, descriptor,
completion, invocation, compiler-lineage, and authority contracts listed in
the machine-readable policy. A type in this layer may be canonical
representation without being a durable wire format or an authenticated
statement; each owning crate states which of those stronger contracts, if any,
it provides.

This layer MUST remain independent of rustc implementation objects, Pliron
handles or text, LLVM objects, Verus executors, HSA/HIP handles, process-local
addresses, and integration drivers. It may depend only on itself and external
libraries admitted by the relevant canonical format contract.

### Rust frontend

The Rust frontend owns kernel/device source APIs, macros, rustc-facing source
authentication, admitted Rust semantics, and extraction contracts. It may
depend on canonical contracts and itself. It MUST NOT select a production
pipeline or depend on Pliron, target lowering, host runtime, or verification
implementations.

### Pliron framework

The Pliron framework owns context construction and identity, dialect
registration APIs, operation verification, detached transformation services,
and the single KIR bridge. Generic pass execution is withheld until Pliron
provides owner-aware operation handles as tracked by
[#140](https://github.com/harsh-nod/fe2o3/issues/140). Planned operation families are
`mir.*`, `kernel.*`, `schedule.*`,
`tile.*`, `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`.

`fe2o3-pliron` constructs the pinned D0 context and private identity anchor and
validates bounded pass plans without executing them. Seven always-Pliron target-neutral dialect shells implement
`kernel.*`, `schedule.*`, `tile.*`, `gpu.*`, `proof.*`, `dispatch.*`, and
`autotune.*`. `dialect-mir` is primarily the compatibility facade over
`fe2o3-mir-model`; its bounded `mir.*` Pliron module/function/block shell is
available only through the non-default `pliron` feature. These are verified
in-memory representations, not a connected compiler pipeline.
The `kernel.*` shell additionally owns ranked-memory and closed-CFG operations
with local MLIR-style verifiers. `fe2o3-kernel-analysis` owns their bounded,
non-mutating whole-function bounds stage and terminal pre-lowering check.
Its target-neutral analyses remain available without the default
`authenticated-machine-effect` feature, so the production Pliron owner does
not inherit process-control machinery; rustc enables that feature explicitly
where finalized-machine evidence is required.
`fe2o3-pliron` composes those pieces through a closed production recipe and a
move-only constructed-to-bounds-verified typestate transition; callers never
receive its retained Pliron function pointer. This does not create a second
lowering route or relax the owner-handle requirement.
`fe2o3-kir-pliron-bridge` implements an opaque context-bound exact-byte
canonical KIR envelope, while `fe2o3-lower-mir-kernel` and
`fe2o3-lower-kernel-gpu` implement narrow bounded detached lowering services
with context-bound results. The ranked construction/bounds transition is a
production-owned pre-lowering stage; semantic MIR projection and generic
ranked-memory GPU lowering remain unimplemented.

The mandatory [general Kernel IR check pipeline](general-kernel-check-pipeline-v1.md)
runs before Pliron projection or transformation. It is a closed target-neutral
analysis sequence over immutable `fe2o3-kernel-ir`, not a substitute for the
withheld generic Pliron execution API. Its reports cannot create compiler,
artifact, or runtime authority.

This layer may consume canonical contracts and admitted frontend models. It
MUST NOT depend on target backend, host runtime, Verus execution, compiler
driver, or fixture crates. `fe2o3-kernel-ir` remains outside this layer and
buildable without Pliron.

### Target backend

The target backend owns AMD-specific operations, AMD legalization, LLVM export,
the pinned upstream LLVM target-machine boundary, in-process LLD finalization,
and physical HSACO validation. It may consume canonical, frontend, Pliron, and
verification contracts as required by the current legacy finalizer. It MUST
NOT depend on host runtime, integration drivers, or fixtures.

No production path may introduce COMGR or shell-mediated GPU linking.

`fe2o3-amdgcn-model` currently owns the existing Pliron-independent AMDGPU
vocabulary and strict lowering implementation. `dialect-amdgcn` is a thin
compatibility re-export under the historical package name; it is not yet the
future `amdgcn.*` Pliron dialect. Canonical AMD target identities and
capabilities remain in `fe2o3-amd-target`. The production-directed finalizer
continues to use one pinned upstream LLVM build, target-machine object emission,
and in-process LLD linking in the isolated worker.

### Verification

Verification owns proof checking, proof-package validation, Verus adapters, and
service/scheduler proof implementations. Proof code consumes canonical
statements and admitted source semantics; it does not grant artifact or launch
authority by dependency direction. It MUST NOT depend on target backend, host
runtime, integration drivers, or fixtures.

### Host runtime

Host runtime owns allocation, launch, completion, HSA/HIP adaptation, artifact
loading, service host typestates, and persistent-worker runtime mechanics. It
may consume canonical, frontend, backend, and verification contracts. It MUST
NOT depend on Pliron implementation objects, compiler drivers, or fixtures.

`fe2o3-service-host` is classified here because it owns the host-side service
typestate boundary, but its current P1 implementation is deliberately
authority-free. It consumes only canonical `fe2o3-service-model` and
`fe2o3-host-api` records, retains caller storage borrows, and performs no
allocation, load, launch, queue publication, execution, runtime wait,
authentication, proof, or storage release.

### Integration

Integration owns CLI composition, rustc codegen integration, pipeline
selection, legacy adaptation, shadow comparison, and end-to-end differential
orchestration. Integration may compose any production layer but MUST NOT depend
on examples or test fixtures. It is the only layer that selects `Legacy`,
`PlironShadow`, or `PlironV1`.

`fe2o3-compiler-api` defines those three selectors as inert request data.
`fe2o3-compiler-driver` routes exactly one selected, configured backend and
revalidates its bounded output. `fe2o3-legacy-compiler` only defines the
dormant adapter contract for the current implementation owner. No production
selection path depends on the new driver or adapter at this checkpoint; the
working legacy and opt-in Kernel IR routes remain composed in
`rustc-codegen-fe2o3`.

`fe2o3-pliron-scalar-add-v1` is an intentionally narrow integration crate. It
composes one checked-in backend fixture, the admitted Pliron/V2 lineage, the
pinned upstream LLVM/in-process-LLD worker, exact HSACO finalization, and a
sealed one-shot HSA consumer for `gfx942:xnack-`. It is not a general frontend,
backend, authority framework, or reusable runtime API. In particular, its
backend fixture is not Rust user source, and its compile-time checkout policy
records repository/build provenance rather than an external signature or
attestation. The qualifying runtime observed `gfx942:sramecc+:xnack-`, a COV6
descriptor kernarg alignment of 8, and a runtime storage alignment of 16.

### Fixtures

Examples and nested test fixtures may exercise any layer. Production crates
MUST NOT depend on them. A fixture is never an authority-bearing implementation.

## Dependency Matrix

`yes` means the source row may declare a dependency on the target column. This
is an architectural allowance, not a requirement.

| From / to | Contracts | Frontend | Pliron | Backend | Verification | Runtime | Integration | Fixture |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Canonical contracts | yes | no | no | no | no | no | no | no |
| Rust frontend | yes | yes | no | no | no | no | no | no |
| Pliron framework | yes | yes | yes | no | no | no | no | no |
| Target backend | yes | yes | yes | yes | yes | no | no | no |
| Verification | yes | yes | no | no | yes | no | no | no |
| Host runtime | yes | yes | no | yes | yes | yes | no | no |
| Integration | yes | yes | yes | yes | yes | yes | yes | no |
| Fixture | yes | yes | yes | yes | yes | yes | yes | yes |

The machine-readable policy is authoritative when this table and the checker
disagree. Updating a direction requires architecture review and a focused
negative test demonstrating why the old prohibition is no longer valid.

All Cargo dependency kinds are checked: normal, build, dev, and target-specific
dependencies. Moving a forbidden edge into a build script or test does not
make the direction acceptable. Cross-layer conformance tests belong in a
fixture or integration crate.

## Issue #134 Ownership

Issue #134 remains open and can proceed in the following non-overlapping
lanes. "Landed" below means representation or routing infrastructure exists;
it does not mean production compilation exists.

| Lane | Primary write ownership | State at the 2026-08-18 checkpoint |
|---|---|---|
| Source/model extraction | `fe2o3-mir-model`, frontend adapters | Canonical model extracted; general frontend integration remains open |
| Pliron context | `fe2o3-pliron` | Pinned context, private identity anchor, registration, and bounded pass planning landed; generic execution awaits owner-aware handles |
| Dialects | One `dialect-*` crate per operation family | Seven target-neutral shells plus feature-gated `mir.*` shell landed |
| Transformations | One `fe2o3-lower-*` family | Narrow context-bound MIR-to-kernel and kernel-to-GPU detached services landed; full production ladder remains open |
| KIR bridge | `fe2o3-kir-pliron-bridge` | Opaque context-bound exact-byte V1-V5 envelope landed; complete semantic bridge gate remains open |
| Bounded vertical-slice composition | `fe2o3-pliron-scalar-add-v1` | Exact backend-fixture-to-MI300X scalar-add route landed with sealed one-shot consumption; Rust user-source integration, external attestation, and generalization remain open |
| Proof overlays | `fe2o3-proof-contracts`, `dialect-proof` | Solver-neutral records and inert Pliron overlay landed; proof integration remains open |
| AMD lowering | AMD model/dialect/lowering crates | Existing implementation extracted to `fe2o3-amdgcn-model`; future Pliron AMD lowering remains open |
| Driver | `fe2o3-compiler-driver`, legacy adapter | API routing and dormant adapter landed; production selection and shadow comparison remain open |

Dialect agents MUST NOT edit central registration or production selection.
They provide a registration function and focused tests for the integration
owner to compose. A Pliron transformation service cannot publish, load, tune,
or launch an artifact directly.

## Issue #135 Ownership

Issue #135 remains open. It depends on stable #134 contracts but has
independent model, host, and proof lanes. The P0/P1 representations below do
not execute a persistent service.

| Lane | Primary write ownership | State at the 2026-08-18 checkpoint |
|---|---|---|
| Service model | `fe2o3-service-model` | P0 identities, transitions, invariants, and independent property classifications landed |
| Scheduler proofs | `fe2o3-service-verus` | Package boundary reserved; proof implementation remains open |
| Host typestates | `fe2o3-service-host` | Authority-free P1 lifecycle/ticket/borrow adapter landed; runtime binding remains open |
| Host operation contracts | `fe2o3-host-api` | Inert compile/admit/load/dispatch/wait records landed; executors remain open |
| GPU operations | scheduler/service dialect crates | Required general service operations and compilation remain open |
| AMD synchronization | AMD lowering family | Persistent-service memory/order lowering remains open |
| Integration and qualification | compiler driver and external fixtures | Production service composition, execution, and qualification remain open |

The service model MUST NOT depend on Pliron, Verus, HSA, HIP, or host process
types. Persistent host code MUST NOT import Pliron handles. Verification and
runtime may independently consume the same canonical service transitions.

## Parallel Change Protocol

Each feature lane owns its crate, tests, and local documentation. Changes
SHOULD be small commits that preserve the legacy default and include positive,
negative, canonical round-trip, and compatibility tests proportional to the
boundary changed.

The integration owner exclusively performs:

1. shared dependency and lockfile changes;
2. root workspace and dependency-policy changes;
3. central dialect registration;
4. compiler selector and release-pipeline composition;
5. cross-layer conflict resolution and final generic/hardware qualification.

At the compiler API boundary, `PlironShadow` is inspect-only and `PlironV1`
must fail closed without legacy fallback. Neither selector is wired into the
production compiler at this checkpoint. The existing legacy path remains the
production default until a separate reviewed milestone satisfies the Wave 0
gates.

## Policy Checker

Run the focused gate with:

```text
scripts/ci-local.sh workspace-policy
```

The checker invokes `cargo metadata --locked --format-version 1 --no-deps` and
parses JSON with Python's standard library. It does not require `jq`, network
access, compilation, or GPU hardware. Diagnostics are sorted, and any
unclassified workspace member, invalid policy declaration, or forbidden local
dependency fails the generic CI lane.
