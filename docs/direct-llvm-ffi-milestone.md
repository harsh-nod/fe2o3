# Direct LLVM and Device FFI Milestone

Status: active implementation plan.

This milestone connects the existing direct LLVM/LLD worker and device FFI
contracts to the artifact transaction, bundle, and runtime paths. Its end state
is a reproducible external-device-code link that executes in both directions on
RDNA and CDNA hardware without COMGR.

The gates below are intentionally narrower than the repository-wide G1-G8
roadmap. They describe one end-to-end milestone and do not replace the broader
CUDA-Oxide parity gates in `implementation-roadmap-v2.md`.

## Program Invariants

1. The rustc process never loads the native link worker or LLVM libraries.
2. One measured LLVM build parses, links, optimizes, emits AMDGPU objects, and
   invokes LLD through library APIs.
3. Every input, option, target, requested symbol, worker, and output is bound by
   a canonical identity before publication.
4. Link and finalization failures publish no artifact and invalidate stale
   output from the same attempt.
5. FFI declarations grant no launch, memory, proof, or safety authority.
6. Only finalized, inspected, manifest-bound output may enter an artifact
   container or bundle.
7. Hardware execution is evidence for the exact target and commit only. It is
   not source-level proof or compiler refinement.

## G1: Canonical Link Closure

Build a closed `WorkerRequestV1` from a validated multi-input link plan and
device FFI closure. Reject missing, duplicate, conflicting, noncanonical, or
unreferenced inputs and symbols before process execution.

Exit gate:

- request construction is deterministic and round-trips through the wire codec;
- input bytes match all declared identities and bounds;
- target, code-object version, options, imports, and exports agree;
- mutation, truncation, permutation, and symbol-conflict tests fail closed.

Primary ownership: `fe2o3-hsaco-finalize` request construction and tests.

## G2: Supervised Worker Execution

Execute a descriptor-pinned worker with bounded stdin, stdout, stderr, time,
environment, and process-tree lifetime. Verify the worker/toolchain measurement
and response identity before returning inert output evidence.

Exit gate:

- pathname substitution cannot change the executed object;
- short writes, early exits, hangs, output floods, malformed responses, and
  descendant processes are handled deterministically;
- successful responses bind the exact request, worker, output bytes, and
  diagnostics;
- no API in this gate can publish or load an artifact.

Primary ownership: `fe2o3-hsaco-finalize` executor module and adversarial tests.

## G3: Native LLVM/LLD Pipeline

Complete the C++ worker pipeline for bounded LLVM bitcode and relocatable AMDGPU
objects. Use LLVM linker, verifier, optimizer, target-machine, and LLD library
APIs from one configured build.

Exit gate:

- bitcode plus bitcode, bitcode plus object, and multi-object fixtures link;
- required imports resolve and requested exports survive optimization/linking;
- target and code-object mismatches fail before output publication;
- one pinned ROCm LLVM development build produces a reproducible inspected
  HSACO without shelling out to COMGR or command-line link tools.

Primary ownership: `tools/fe2o3-llvm-link-worker` and native tests.

## G4: Compiler FFI Closure

Collect concrete import and export contracts from the final monomorphized device
graph. Resolve declarations across crates and translate the closed set into G1
link requirements without trusting source names alone.

Exit gate:

- same-symbol contracts must agree on direction, ABI, address spaces, effects,
  target, code-object version, and semantic identity;
- Rust-to-external and external-to-Rust closures are emitted deterministically;
- unresolved, duplicate, generic, host-only, or spoofed declarations fail with
  stable diagnostics and source ownership;
- cross-crate tests demonstrate logical-name isolation.

Primary ownership: `rustc-codegen-fe2o3` device FFI collection and tests.

## G5: Transactional Link Publication

Run G1-G3 inside a build attempt and finalize the returned code object before
atomic publication. Every failure removes or invalidates stale output owned by
that attempt while preserving unrelated artifacts.

Exit gate:

- the attempt state machine records request, worker, response, finalization,
  and publication identities in order;
- crash/restart tests recover or remove incomplete attempts deterministically;
- digest, descriptor, metadata, symbol, or target mismatches publish nothing;
- publication is atomic and scoped to the exact package, kernel set, and target.

Primary ownership: `fe2o3-artifact-transaction` integration types and tests.

## G6: Authenticated Bundle Binding

Represent direct-link provenance and the closed FFI contract in the versioned
artifact model. Bind finalized payload bytes to manifests and multi-kernel
bundle selection without granting runtime authority from descriptive evidence.

Exit gate:

- container and bundle codecs are canonical, bounded, and backward compatible;
- link evidence names the request, worker/toolchain, response, output, and FFI
  closure identities;
- any evidence or payload substitution breaks validation;
- unknown versions and capabilities fail closed.

Primary ownership: `fe2o3-artifacts` model, codecs, and robustness tests.

## G7: Typed Loading and Bidirectional Execution

Load only G5/G6-authenticated output through the typed runtime and execute
minimal bidirectional FFI fixtures: Rust calls an external device function and
external device code calls an exported Rust function.

Exit gate:

- generated launch packing is derived from the finalized manifest;
- wrong context, target, symbol, ABI, or artifact identity fails before launch;
- both directions execute with independent CPU oracles on `gfx1151` and
  `gfx942`;
- resources and linked module lifetimes remain valid through completion and
  all safe APIs retain the existing alias and launch admission checks.

Primary ownership: runtime integration plus dedicated FFI examples and hardware
tests.

## G8: Reproducibility, Evidence, and Release Gate

Make the end-to-end path repeatable in local and remote CI, with negative
corpora and evidence records that cannot overstate coverage.

Exit gate:

- CPU-only protocol, codec, transaction, and malformed-input suites pass;
- direct-link compile lanes pass for `gfx1151`, `gfx942`, and compile-only
  `gfx950`;
- hardware execution and differential results are commit- and target-pinned;
- two clean builds produce identical request and artifact identities;
- parity rows 27, 28, and 39 change only when their persisted evidence meets the
  dashboard policy.

Primary ownership: CI scripts, differential fixtures, evidence dashboard, and
release documentation.

## Dependency And Merge Order

```text
G1 request closure ----+----> G2 supervision --+
                       |                       |
G3 native worker ------+-----------------------+--> G5 transaction
                       |                       |          |
G4 compiler closure ---+-----------------------+          +--> G7 execution
                                                              ^
G6 bundle binding --------------------------------------------+

G8 tests every merged gate and owns the final evidence update.
```

G1 and G6 are shared schema owners. Changes needed by another gate are proposed
as a small fixture-backed patch and integrated by the owning gate. G7 does not
block G1-G6 implementation: its fixtures and host-side rejection tests can be
built against inert test artifacts until G5 is available.
