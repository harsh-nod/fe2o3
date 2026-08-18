# Workspace Layers and Parallel Ownership

Status: normative Wave 1 workspace policy for issues #134 and #135. This
document defines repository ownership and dependency boundaries. It does not
claim that the Pliron pipeline or persistent GPU execution is implemented.

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
models. Planned crates include `fe2o3-mir-model`, `fe2o3-compiler-api`,
`fe2o3-proof-contracts`, `fe2o3-service-model`, and `fe2o3-host-api`.

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

The Pliron framework owns context construction, dialect registration APIs,
operation verification, transformation passes, pass receipts, and the single
KIR bridge. Planned operation families are `mir.*`, `kernel.*`, `schedule.*`,
`tile.*`, `gpu.*`, `proof.*`, `dispatch.*`, and `autotune.*`.

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

### Integration

Integration owns CLI composition, rustc codegen integration, pipeline
selection, legacy adaptation, shadow comparison, and end-to-end differential
orchestration. Integration may compose any production layer but MUST NOT depend
on examples or test fixtures. It is the only layer that selects `Legacy`,
`PlironShadow`, or `PlironV1`.

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

Issue #134 can proceed in the following non-overlapping lanes:

| Lane | Primary write ownership | Integration input |
|---|---|---|
| Source/model extraction | `fe2o3-mir-model`, frontend adapters | Canonical model and compatibility tests |
| Pliron context | `fe2o3-pliron` | Registration and verifier API |
| Dialects | One `dialect-*` crate per operation family | Registered operations and round-trip tests |
| Transformations | One `fe2o3-lower-*` family | Input/output verifier and receipt tests |
| KIR bridge | `fe2o3-kir-pliron-bridge` | Versioned lossless conversion tests |
| Proof overlays | `fe2o3-proof-contracts`, `dialect-proof` | Canonical obligations and erasure tests |
| AMD lowering | AMD dialect/lowering crates | LLVM export and physical ABI tests |
| Driver | `fe2o3-compiler-driver`, legacy adapter | Shadow comparison and selector tests |

Dialect agents MUST NOT edit central registration or production selection.
They provide a registration function and focused tests for the integration
owner to compose. A Pliron pass cannot publish, load, tune, or launch an
artifact directly.

## Issue #135 Ownership

Issue #135 depends on stable #134 contracts but has independent model, host,
and proof lanes:

| Lane | Primary write ownership | May begin when |
|---|---|---|
| Service model | `fe2o3-service-model` | Canonical identity conventions are stable |
| Scheduler proofs | `fe2o3-service-verus` | Model transitions and properties are versioned |
| Host typestates | `fe2o3-service-host` | Service model API is reviewed |
| GPU operations | scheduler/service dialect crates | Required #134 dialect APIs are registered |
| AMD synchronization | AMD lowering family | GPU memory/order semantics are verified |
| Integration and qualification | compiler driver and external fixtures | All consumed stages emit canonical receipts |

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

`PlironShadow` remains inspect-only. `PlironV1` must fail closed without legacy
fallback. The existing legacy path remains the production default until a
separate reviewed milestone satisfies the Wave 0 gates.

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
