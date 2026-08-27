# Workspace Layers and Parallel Ownership

Status: normative Wave 1 workspace policy, updated through the 2026-08-19
bounded Pliron scalar-add checkpoint. Issues
[#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) remain open. The landed
crates make both epics infrastructure-enabled and complete one exact scalar
compile/finalize slice plus a qualification-only execution observation; they do
not implement a general production Pliron pipeline or persistent GPU
execution.

This policy refines the [Pliron Wave 0 architecture](pliron-wave0-architecture.md)
and preserves the existing production compiler, artifact, proof, publication,
and runtime behavior. The words MUST, MUST NOT, SHOULD, and MAY are normative.

## Purpose

Issues #134 and #135 introduce several independently implementable compiler,
runtime, and verification components. They must be able to land alongside
ordinary kernel, backend, runtime, evidence, and documentation work without
making the root manifest, compiler composition, or existing monoliths permanent
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
dialect registration, production-route boundary, qualification feature, and release composition
are integration-owned files. Feature agents SHOULD NOT edit them. A dependency
addition that changes `Cargo.lock` is a separate, integration-reviewed commit.

Generic workspace checking runs the complete supported workspace graph through
`cargo fe2o3 check --workspace --all-targets`. A sealed, metadata-derived map
identifies every exact Cargo target source and whether its package structurally
requires a compiler-derived binding. The host-only wrapper injects the normal
crate-name/ordered-metadata binding for those targets, including transitive
managed libraries, and leaves ordinary targets unbound. Manifest-declared
examples that do not participate in generic checking remain explicit Cargo
exclusions. This route admits no codegen backend, artifact directory,
publication action, worker, or GPU authority; it is not compiler qualification
or artifact production.

Generic CPU testing partitions the manifest entries with `rustc_check=true` and
`artifact_qualification=none` by that same structural projection. Artifact
qualification is a separate, closed route classification and does not erase
the source-artifact inventory. Ordinary entries use raw Cargo. Managed entries
use the feature-free
`cargo fe2o3 test --locked --all-targets -p <package>` path; `--all-targets` is
required, while caller `--target`, `--config`, Cargo-side `-Z`, `--doc`,
`--no-run`, and ambient compiler, rustdoc, protected fe2o3, and runner selections
are rejected. Configured compiler, protected fe2o3, loader, and runner selection
is rejected; configured rustdoc is overridden with the disabled selection, and
ambient loader variables are scrubbed. The raw and managed lists remain
package-name independent, sorted, disjoint, and exhaustive.

The host-test route trusts workspace source and non-protected Cargo
configuration, build scripts, procedural macros, linkers, and tests. Its fixed
runner closes the test child's environment and descriptor boundary; it is not a
sandbox. The runner opens and hashes Cargo's original test executable, executes
the retained original while Cargo's path remains stable to preserve ordinary
`current_exe` and `$ORIGIN` behavior and prevent directory-entry substitution
between pin and execution, then rechecks it afterward. That behavior does not
freeze same-inode writes and grants no immutable-artifact, origin, backend,
HSACO, publication, GPU, or performance-prediction authority. Ordinary Cargo
host artifacts are still produced, and trusted test code remains able to access
the user's files, network, and device nodes.

This projection is deliberately package-wide and feature-independent. Every
regular `*.rs` file outside the exact generated Cargo target-directory boundary
participates. Every exact Cargo target
root reported by metadata MUST also be a package-owned UTF-8 `.rs` path, matching
the rustc invocation parser's input contract. Conventional `mod`,
literal `#[path]`, and literal `include!` edges are considered resolved only
when they name a source already scanned beneath the same package root. A
dynamic, missing, or package-external edge conservatively selects a package
that has no observed binding declaration. When the package contains an
explicit fallback and no direct unnamespaced typed kernel, that observed
fallback wins and the external edge does not select the wrapper. The external
content remains uninspected; an unnamespaced typed kernel hidden there can make
ordinary compilation fail, but cannot grant compiler or publication authority.
Nested Cargo package roots are always separate, uninspected ownership
boundaries; an edge from the parent into one is an external selection boundary.
A package MUST NOT mix a directly observed compiler-derived binding with an
explicit fallback namespace anywhere in the complete projection, even when
Cargo features or targets make the sources mutually exclusive. Unparseable
scanned sources are rejected. This strict package ownership rule avoids feature-dependent binding selection
and is checked with mixed-target and external-edge adversaries.

Each Cargo target source MUST be owned beneath its package root. Cross-package
source reuse uses a package-owned target root with an explicit external
module/include edge; when no direct fallback is observed, that unresolved
boundary makes the owning package managed, but the projection scanner does not
follow or authenticate the included file.
The exact Cargo `target_directory` reported by metadata is a generated-output
boundary: declared sources beneath it are rejected, and an in-package target
subtree is skipped without inspection. Other package-tree entries are opened
descriptor-relatively so symlinks and special files fail the availability
check even though only regular `*.rs` files are parsed for binding ownership.
Cargo metadata paths, manifests, directories, and opened package sources are
revalidated during each bounded scan, and generic CI recomputes the exact
managed set after the binding-only checks and tests. It also revalidates both
CPU-test partitions after managed test execution. These are authority-free
policy snapshots, not authentication of later Cargo compilation inputs. The
protected Cargo-configuration scans before and after host tests diagnose a
persistent change but are not an atomic snapshot; neither mechanism makes a
TOCTOU claim beyond each individual retained scan. Artifact and publication
authority continue to require their separate authenticated source, worker, and
finalizer contracts.

Source depth, file size, entry, token, module-edge, byte, and name-byte limits
apply to each package scan. Separate aggregate package, Cargo-target, source
entry, source-file, byte, and name-byte limits bound the complete workspace
projection before the binding-check Cargo process is launched.
The source-token traversal limit applies after bounded source bytes have been
parsed by `syn`; extreme parser nesting remains a local availability limitation,
not a compiler, artifact, or publication authority boundary.

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
`fe2o3-lower-mir-kernel` retains a narrow bounded MIR-to-kernel conformance
service with context-bound results. The detached KIR-envelope and
kernel-to-GPU services were removed; ranked construction, bounds verification,
semantic MIR projection, and generic ranked-memory lowering belong only to the
production-owned transaction.

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

`fe2o3-runtime` is the sole pure-Rust gfx942 composition boundary over the
canonical AMDHSA loader and `fe2o3-kfd`. Its safe API prepares a complete
address-free request and has one consuming execution transition. That transition
requires an unsafe Worker V3 authority implementation and independently matches
the final object, kernel, complete invocation contract, and checked KFD device.
The exact-artifact hardware diagnostic exercises it with a manually asserted
unsafe authority; no production implementation exists yet. Production
execution must consume verifier authority in this crate rather than adding
another host-runtime route.

### Integration

Integration owns CLI composition, rustc codegen integration, qualification
comparison, and end-to-end differential orchestration. Integration may compose
any production layer but MUST NOT depend on examples or test fixtures.

`fe2o3-compiler-api` defines one inert production request contract. `cargo-fe2o3` and `rustc-codegen-fe2o3` own the sole managed production composition. No workload oracle is compiled; the legacy selector is rejected.

The retired scalar-add fixture join was never a general frontend or backend and has been removed. The qualifying runtime historically observed `gfx942:sramecc+:xnack-`, a COV6
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
| Transformations | Production transaction plus retained MIR conformance service | Context-bound MIR-to-kernel conformance remains; detached GPU lowering is retired |
| KIR custody | Production transaction | Canonical KIR is retained inside the sole compiler-owned transaction |
| Proof overlays | `fe2o3-proof-contracts`, `dialect-proof` | Solver-neutral records and inert Pliron overlay landed; proof integration remains open |
| AMD lowering | AMD model/dialect/lowering crates | Existing implementation extracted to `fe2o3-amdgcn-model`; future Pliron AMD lowering remains open |
| Production composition | `cargo-fe2o3`, `rustc-codegen-fe2o3` | One Worker V3 route from attributed Rust source; no workload oracle is compiled |

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
SHOULD be small commits that preserve the sole production route and include positive,
negative, canonical round-trip, and compatibility tests proportional to the
boundary changed.

The integration owner exclusively performs:

1. shared dependency and lockfile changes;
2. root workspace and dependency-policy changes;
3. central dialect registration;
4. compiler and release-pipeline composition;
5. cross-layer conflict resolution and final generic/hardware qualification.

At the compiler API boundary, every request enters the same production backend
and must fail closed without fallback. Qualification comparisons are test
oracles, not selectable compiler implementations.

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
