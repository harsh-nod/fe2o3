# fe2o3 Architecture v2

Status: proposed architecture and implementation contract.

This document describes the target architecture for fe2o3. It is not a
description of the current implementation. The current backend is a useful
bootstrap: it discovers `#[kernel]` functions, walks reachable MIR, recognizes a
limited set of elementwise expressions, emits AMDGPU LLVM IR, builds HSACO
files, and delegates host code generation to `rustc_codegen_llvm`.

The v2 architecture preserves the working AMD runtime and artifact path while
replacing the expression recognizer with a general compiler pipeline. It also
adds a source-level verification path without placing the compiler inside the
Verus trust claim.

Related documents:

- [cuda-oxide parity matrix](cuda-oxide-parity-matrix.md)
- [verification model](verification-model.md)
- [implementation roadmap](implementation-roadmap-v2.md)

## Goals

1. Keep host code, executable kernel code, and kernel specifications in one
   Rust source tree.
2. Compile ordinary host code with the standard Rust LLVM backend.
3. Compile monomorphized device code through a target-neutral GPU pipeline.
4. Support AMDGPU first without baking HIP, AMD wave size, or HSACO into the
   target-neutral APIs.
5. Let Verus prove source-level functional and safety properties against an
   explicit GPU execution model.
6. Generate the host launch API and device argument packing from one ABI
   manifest.
7. Make raw launch and raw device memory operations explicit `unsafe`
   boundaries.
8. Make builds deterministic and bind source, proof, ABI, target, and device
   payloads together.
9. Preserve a path to NVPTX and SPIR-V without requiring either target for the
   AMD parity milestone.

## Non-goals

- Verifying rustc, fe2o3 lowering, LLVM, ROCm, the driver, or GPU hardware with
  Verus.
- Supporting `std`, unwinding, a device heap, or dynamic dispatch in the first
  parity release.
- Hiding target-specific operations behind misleadingly portable names.
- Requiring every kernel to be verified. Checked and explicitly unsafe kernels
  remain supported and are visibly distinguished.
- Replacing the working HIP runtime before the v2 compiler can run existing
  examples.

## One Source, Three Consumers

The product boundary is the Rust source, not a custom host codegen backend.
`cargo fe2o3` coordinates three consumers of that source:

```text
                         one Rust source tree
                    executable code + contracts
                               |
              +----------------+----------------+
              |                |                |
              v                v                v
          Verus driver     normal rustc     device frontend
              |             host LLVM       rustc_public MIR
              |                |                |
              v                v                v
        proof manifest     host objects       mir.* IR
                                                |
                                                v
                                              gpu.* IR
                                                |
                                      +---------+---------+
                                      |                   |
                                      v                   v
                                  AMDGPU lower       future targets
                                      |
                                      v
                                    HSACO
                                      |
              +-----------------------+-------------------+
              v
       versioned artifact bundle + generated typed launch API
```

The algorithm is not copied into separate host, device, and proof files.
Specifications and proof-only state are erased from executable compilation.
Host-only wrappers and device entry shims may be generated, but they are not
independent implementations of the kernel.

### Build order

For a final application crate, `cargo fe2o3 build` performs these logical
steps:

1. Resolve the Cargo graph, features, target set, and kernel instantiations.
2. Run the device frontend on the same final crate graph and collect concrete
   device mono-items.
3. Verify requested kernel instances and emit proof records when verification
   is enabled.
4. Lower device mono-items, finalize target payloads, and produce a versioned
   artifact bundle.
5. Compile host code with ordinary rustc and link or embed the bundle.
6. Reject the build when source, ABI, proof, or payload identities disagree.

The implementation may overlap independent steps, but the artifact bundle is
not valid until all required identities agree.

## Permanent Component Boundaries

The crate names below are architectural ownership boundaries. Exact package
names may change during bootstrap, but dependencies must continue to point
downward through these layers.

| Component | Responsibility | Must not own |
|:--|:--|:--|
| `fe2o3-rustc-front` | Kernel discovery, final mono-item collection, `rustc_public` conversion, source spans | GPU lowering, host launch packing |
| `fe2o3-mir` | Rust layout-aware types and `mir.*` operations | AMD intrinsics, HIP handles |
| `fe2o3-kernel-ir` | `gpu.*` operations, SIMT domains, effects, address spaces, barriers, atomics, capabilities | Rust compiler types, HIP calls |
| `fe2o3-transforms` | Verification, mem2reg, canonicalization, divergence/effect analyses | Target command execution |
| `fe2o3-amdgpu` | AMDGPU lowering, OCML/OCKL calls, target features, code-object metadata | Host borrow policy, Verus proof claims |
| `fe2o3-artifacts` | Versioned neutral bundle and identity records | Compilation and loading policy |
| `fe2o3-host` | Generated typed modules, prepared launches, argument ownership | MIR inspection, target lowering |
| `fe2o3-core` | HIP resource wrappers, streams, events, buffers, raw launch | Kernel type discovery |
| `fe2o3-contracts` | Shared launch/spec vocabulary and erased proof markers | Solving proofs, code generation |
| `fe2o3-verifier` | Verus invocation, policy checks, proof manifest creation | Claiming compiler correctness |
| `cargo-fe2o3` | Build graph orchestration, tool discovery, cache keys, inspection commands | Semantic lowering logic |

`fe2o3-hip-sys` remains the narrow raw FFI layer. The current
`rustc-codegen-fe2o3` can host adapters while the new layers are introduced,
but it is not the permanent owner of host compilation.

## Frontend and Device Extraction

### Explicit extraction driver

V2 uses an explicit rustc driver invocation for device extraction. The driver
collects the final application's monomorphized kernel roots and their reachable
device call graphs, then converts them through `rustc_public` types. Normal
host compilation does not run through fe2o3's codegen backend.

The current `CodegenBackend` integration remains available as a compatibility
adapter until the extraction driver reaches feature parity. New compiler
features must be implemented below a frontend trait so both entry paths feed
the same importer during migration.

### Collection rules

The frontend must:

- identify kernels through generated metadata, not substring matching alone;
- collect concrete generic and const-generic instances from the final crate;
- walk direct calls, function items, closure shims, drop glue, and supported
  intrinsics;
- retain source spans and rustc layouts;
- collect cross-crate device functions without compiling unrelated host code as
  device code;
- reject reachable unsupported `std`, allocation, unwinding, and FFI paths with
  a call chain in the diagnostic;
- prune only branches proven dead by an explicit compiler transform;
- produce deterministic function and kernel identities.

Library crates may declare kernels and device functions. Artifact finalization
occurs at the final binary, cdylib, or explicitly selected device-library
boundary so concrete monomorphizations are known.

## Intermediate Representations

### `mir.*`: Rust semantic IR

`mir.*` is a layout-aware representation of executable Rust MIR. It preserves
the information needed to implement Rust correctly:

- integer and float widths;
- structs, tuples, arrays, enums, discriminants, and padding;
- references, raw pointers, slices, provenance, and address spaces;
- control flow, calls, assertions, drops, and unreachable edges;
- constants, statics, relocations, and source locations;
- volatile and atomic memory semantics.

Import starts with one memory slot per non-zero-sized MIR local. A verified
mem2reg pass then creates SSA values. This keeps cross-block MIR translation
simple while avoiding permanent stack traffic.

### `gpu.*`: target-neutral kernel IR

`gpu.*` is the semantic boundary shared by compilation, static analysis, and
proof correspondence. It explicitly models concepts that are implicit or
encoded as calls in Rust MIR:

- grid, workgroup, thread, subgroup, and lane domains;
- one-, two-, and three-dimensional coordinates;
- global, constant, workgroup/LDS, private, and generic memory spaces;
- memory effects and allocation provenance;
- barriers, fence scope, convergence, and execution epochs;
- atomics with operation, ordering, and scope;
- uniform and divergent values and control flow;
- static and dynamic resource requirements;
- target capability requirements.

Rust language operations lower from `mir.*` to ordinary target-neutral ops.
Device APIs lower to `gpu.*` ops. Target backends then legalize only the
capabilities supported by the selected device.

### IR framework

The near-term implementation uses Pliron because the pinned cuda-oxide
baseline already demonstrates a Rust-native MIR importer, verification,
mem2reg, dialect conversion, and LLVM export. V2 must keep serialization and
pass interfaces independent of Pliron object identity. That boundary permits a
future MLIR lower half without changing the source API, artifact manifest, or
verification model.

## Capabilities, Not CUDA Vocabulary

Portable APIs express semantics. Vendor extensions name the vendor:

```text
Portable:  Subgroup, Ballot, Shuffle, WorkgroupMemory, MatrixMultiply
AMD:       AmdWave, AmdMfma, AmdWmma, AmdDsPermute
Future:    NvidiaWarp, NvidiaWgmma, NvidiaTma
```

A kernel records required capabilities in its manifest. Compilation fails with
a source-level diagnostic when the selected target cannot satisfy them. Wave
width is a target property or an explicit generic parameter; portable code may
not assume 32 lanes.

Capabilities carry semantic contracts as well as feature names. For example,
an async copy capability states source/destination spaces, alignment,
completion protocol, visibility, and participating scope. Similar-looking
instructions are not treated as equivalent unless those contracts match.

## Kernel ABI and Artifact Bundle

The artifact manifest is the canonical serialized description of the
host/device boundary, but it is not authority by itself. A sealed runtime token
becomes authoritative only after matching a compiler-generated host descriptor,
the complete manifest and payload digest, inspected code-object metadata, the
observed device target, and the owning context. Neither a launch macro nor a
loader independently guesses or elevates an ABI declaration.

`DeviceCopy` is structural host-side byte-copy validity evidence, not device ABI
or semantic evidence. Integer fields may encode host addresses or handles. A
safe typed launch may allow device interpretation only after manifest-derived
type and ABI identities match and the required provenance, address-space, and
capability evidence is present.

Each monomorphized kernel entry contains at least:

```text
bundle format version
kernel ID and exported symbol
source and executable semantic hashes
crate graph, feature, target, and compiler identities
ordered argument fields, offsets, sizes, alignments, and address spaces
Rust type and layout identities
mutability, ownership, and alias classes
launch contract and resource requirements
required target capabilities
one or more device payload references
optional proof record and assurance level
debug/source map reference
```

Payloads can include AMDGPU LLVM bitcode, relocatable AMDGPU objects, and
targeted HSACO images. The container is target-neutral and versioned; loading
policy belongs to the AMD runtime. Unknown mandatory fields or capability bits
must cause rejection rather than silent fallback.

The bundle and each payload are content-addressed. Proof records bind to the
executable semantic hash, not just a source filename or kernel symbol.

## Generated Launch Surface

The kernel macro generates a marker type and a typed host declaration. Artifact
finalization supplies the concrete ABI and payload. The generated safe method
accepts only a `PreparedLaunch<K>` branded for the same kernel and context.

The host API has three levels:

1. Safe typed launch with checked geometry, typed arguments, retained borrows,
   and an artifact identity match.
2. Checked but unverified launch, visibly reported as `Checked` assurance.
3. Raw launch, which is `unsafe` and documents ABI, lifetime, alias, geometry,
   resource, and synchronization obligations.

Async operations borrow or own every referenced allocation until a completion
event proves the device is finished. Dropping a submitted future cannot free
its resources early. Cross-stream accesses require an event dependency or an
explicit unsafe obligation.

## Verification Integration

Verus consumes the same kernel source and contracts. Proof-only definitions are
erased before rustc device extraction. The Verus adapter proves properties of a
formal source-level GPU model and emits a proof record. It does not inspect
AMDGPU machine code and does not establish compiler correctness.

The build accepts a proof record only when all of the following match:

- kernel and monomorphized type identity;
- executable semantic hash after proof erasure;
- launch contract and target-neutral capability contract;
- crate features and relevant configuration;
- Verus version, solver version, and approved axiom policy;
- verification model version.

See [verification-model.md](verification-model.md) for proof obligations and
the complete trust boundary.

## AMDGPU Lowering

The AMD backend maps `gpu.*` semantics to AMDGPU LLVM IR and ROCm device
libraries:

- workgroups and threads to AMDGPU workgroup/workitem intrinsics;
- subgroups to wave32 or wave64 operations with an explicit width contract;
- workgroup memory to LDS address space and metadata;
- scoped atomics and fences to AMDGPU/LLVM synchronization scopes;
- float math to OCML/OCKL or correctly selected LLVM intrinsics;
- matrix capabilities to target-gated MFMA/WMMA operations;
- kernels and metadata to HSA code objects.

ROCm clang/lld remains the bootstrap finalizer. A COMGR-based in-process
finalizer can be added later behind the same interface. Textual LLVM emission
may remain as an inspection format but is not the semantic IR boundary.

## Migration from Current fe2o3

### Retain

- HIP initialization, streams, buffers, modules, errors, and HSACO loading;
- ROCm discovery and target detection in `cargo-fe2o3`;
- kernel root and reachable-call collection tests;
- current examples as end-to-end regression cases;
- LLVM-to-HSACO finalization and metadata inspection;
- the current emitter as a differential bootstrap oracle.

### Replace

- flat `MirOpRecord` streams with typed `mir.*` operations;
- record sketches and elementwise expression recognition with general passes;
- direct textual elementwise LLVM templates with `gpu.*` to AMDGPU lowering;
- filename-based sidecar discovery with versioned embedded bundles;
- safe-looking raw launch packing with typed prepared launches and explicit
  unsafe raw methods.

### Redesign

- `ThreadIndex` as a branded, non-transferable index-space witness;
- `DisjointSlice` as an allocation- and index-space-aware writable view;
- `#[kernel]` to generate stable metadata and cooperate with erased contracts;
- host async APIs so Rust lifetimes cover queued device execution;
- build caching around complete source/proof/target/toolchain identities.

The old and new compilers run side by side until the v2 path passes every
current example and the relevant parity gates. Removal of the recognizer is a
deliberate gate, not an early cleanup task.

## Architectural Invariants

Every implementation change must preserve these invariants:

1. Normal host objects are produced by the standard host backend.
2. There is one executable kernel implementation in source.
3. Argument layout and launch contracts come from the manifest, but only a
   sealed match of compiler descriptor, manifest, payload, code object, device,
   and context can authorize loading or launch.
4. A safe launch cannot be created from an arbitrary symbol, raw pointer list,
   or unbranded geometry.
5. Proof records cannot be reused after an executable, contract, feature,
   target-neutral semantic, or model change.
6. A `Verified` label never implies that the compiler or runtime was verified.
7. Target-specific operations are capability-gated and explicitly named.
8. Every accepted IR module passes structural and semantic verification before
   lowering.
9. Unsupported Rust or GPU semantics fail with diagnostics; they do not lower
   approximately.
10. Cross-crate and generic kernels are finalized only after concrete device
    instances are known.

## Decision Tests

A proposed design belongs in v2 only if it can answer all of these questions:

- Which layer owns the behavior?
- What typed or serialized contract crosses that layer boundary?
- How is the behavior tested without requiring all other layers?
- How does the artifact record it when it affects ABI, launch, or proof?
- Is it portable semantics, an AMD equivalent, or intentionally not
  applicable?
- What unsafe or trusted assumption remains?

If those answers are unclear, the feature is not ready for parallel
implementation.
