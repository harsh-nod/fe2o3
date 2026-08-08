# Direct LLVM and Device FFI Milestone

Status: active; the bounded alpha/zeta `gfx942` compiler-to-HSACO vertical
slice plus raw and generated-safe MI300X execution are implemented through
commit `dc9738e367c392f7716eacb8459ca73fa32abbbb`.

This milestone connects the existing direct LLVM/LLD worker and device FFI
contracts to the artifact transaction, bundle, and runtime paths. Its end state
is a reproducible external-device-code link that executes in both directions on
RDNA and CDNA hardware without COMGR.

The gates below are intentionally narrower than the repository-wide G1-G8
roadmap. They describe one end-to-end milestone and do not replace the broader
CUDA-Oxide parity gates in `implementation-roadmap-v2.md`.

## Implemented gfx942 Vertical Slice

As of commit `daf0b459ced07a25376670c83b1474eaebcd1a68`, the opt-in
`kernel-ir-worker-v2` path implements a bounded G1-G5 publication slice for a
real Cargo crate with the non-empty alpha and zeta General-V3 kernel profiles:

- the external `cargo fe2o3 build` path loads the sealed backend and rustc
  collects both alpha/zeta kernel roots. A separate two-root Worker V2 fixture
  also collects one shared helper exactly once; helper calls resolve from the
  canonical Rust source identity to one collected internal definition, exact
  predeclared signature, and canonical export symbol, while ambiguous,
  uncollected, duplicate, or signature-incompatible contracts fail closed;
- rustc emits verified Kernel IR, an attempt-scoped textual LLVM handoff, and an
  exact symbol-role manifest, using rustc source-owner identity to accept device
  FFI imports;
- rustc lowers the exact alpha/zeta source forms through typed Kernel IR. The
  accepted forms require trusted thread-index provenance, a bounds-checked
  `DisjointSlice::get_mut` `Some` edge that dominates every payload use, strict
  `gfx942` floating-point policy, and the exact one-dimensional 256-thread
  launch contract. Lookalike calls, escaped payloads, merged or false bounds
  edges, wrong targets, and relaxed floating-point pipelines fail closed;
- macro and rustc reconstruction authenticate the logical name and export name
  before assigning the source roles `scale`, `input`, `output`, `a`, `b`, and
  `bias`. Exact role names, ABI layout, effects, crate/kernel binding, and host
  contract identity agree independently; renamed or signature-lookalike roots
  retain positional `argN` fields and cannot acquire the alpha/zeta adapter;
- Cargo consumes that manifest, constructs the closed request, and requires
  byte-identical output from GenericLink and Worker V2 executions;
- the measured out-of-process worker uses LLVM and LLD library APIs directly to
  parse, link, optimize, emit AMDGPU code, and link both kernels and their helper
  into one `gfx942` HSACO; neither COMGR nor command-line linking is used;
- Cargo independently checks the raw HSACO target, exact two-kernel export set,
  descriptors, and AMDHSA metadata. Descriptor-free COV5 retains raw
  compatibility while descriptor-bearing COV6 reconciles explicit and complete
  kernarg sizes. After optimization, the direct LLVM worker removes
  `amdgpu-no-implicitarg-ptr` and restores the canonical 256-byte COV6 implicit
  block so metadata, native descriptors, and host admission agree; and
- the artifact transaction durably publishes the exact admitted raw or
  finalized bytes and an attempt-bound provenance receipt, with adversarial
  substitution, legacy-marker migration, raw/finalized process crash recovery,
  redo, and generation-isolation tests.

The same snapshot adds independently tested foundations around that publication:

- `Gfx942TwoKernelBundleV1` binds exactly two canonically ordered kernel entries
  to one digest-validated native payload and one exact proof record per kernel;
  duplicate proof keys, stale contracts, payload substitutions, and
  cross-kernel proof swaps are rejected;
- `MultiKernelProofAdmissionV1` admits each request only against its own kernel,
  source, contract, authenticated-proof, and shared finalized-executable
  identities, while explicitly granting no load or launch authority;
- the Worker V2 host admission path revalidates every kernel sharing the exact
  finalized payload and can select two distinct compiler-generated marker types
  while retaining the admitted bundle and rejecting marker, ABI/effect, target,
  physical-layout, or executable substitutions; and
- the reviewed HSA lifecycle can load one code object and resolve a fixed set of
  distinct symbols into a non-clone kernel set that borrows the executable.
  Duplicate requests and native symbol, kernel-object, or derived-identity
  aliases fail closed, and the executable cannot be safely unloaded while the
  set remains live.

The same source checkpoint also generates exact alpha/zeta host adapters from
the independently modeled signatures. Each adapter owns its signature-specific
`Arguments`, source-index capabilities, layout, dispatch identity, safe
`prepare` entry point, and linear prepared value. Host preparation validates
the semantic witness, selected artifact entry, exact ABI/effects, checked
allocation-relative regions, alias admission, geometry, physical COV6 layout,
and hidden-argument initialization before its synchronous `dispatch` method can
enter the reviewed HSA boundary.

The production authority foundations are implemented, but the alpha/zeta Cargo
path does not yet compose all of them.
The artifact transaction persists and recovers exact Worker V2 raw or finalized
publication state across process restart. Recovery validates the journal,
publication kind, plan, upstream identity, route/admission, backend receipt,
and completed attempt before clearing durable state; V1 raw markers migrate to
the V2 format. Finalized-bundle admission consumes the sealed preparation and
durable publication, retains and revalidates the exact currentness lease, and
independently checks the concrete container, selected entry, payload, target,
physical ABI, and launch constraints. The authenticated load state machine and
reviewed `fe2o3-hsa-runtime` adapter then retain currentness, executable, symbol,
queue, kernarg, and completion lifetimes. These are existing production-facing
host/runtime mechanisms, not missing designs.

General V3 also has checked buffer views, typed binding/packing, generated
alpha/zeta `Arguments`, safe preparation and dispatch SPI, and a rustc backend
emitter for binding-derived semantic-witness host objects. The Worker V2
integration builds both roots, links and validates both witnesses, publishes
one inspected COV6 HSACO, and can export those exact bytes for the hardware
test. The remaining composition gaps are narrower: Cargo's Worker V2
artifact-container adapter is still inert and compiled only for tests, and no
production implementation of `WorkerV2PrerequisiteAuthenticatorV1` authenticates
the compiler, verifier, ABI, and executable-effect prerequisites required by
the load state machine. The hardware harness therefore accepts the exported
HSACO and digest through an opt-in test boundary and calls the reviewed raw
unsafe HSA adapter directly instead of the generated alpha/zeta safe SPI. This
does not establish production safe dispatch. No Verus result or machine-code
effect/refinement evidence is yet authenticated and bound to the emitted code.

The generic and adversarial suites pass. On the MI300X `gfx942` lane, three
ignored Worker V2 integration tests pass with an unoptimized Debug worker:
`worker_v2_real_source_publishes_inspected_gfx942_hsaco` and
`worker_v2_real_source_links_an_external_bitcode_provider`, plus
`worker_v2_real_source_publishes_two_kernels_with_one_shared_helper`. Together
they cover the direct real-source path, the closed external LLVM
bitcode-provider path, and real-Cargo publication of an exact two-kernel symbol
set with one canonically collected and lowered helper. A separate live
HIP/HSA observation confirms one exact `gfx942` MI300X device correlation.

A fresh current native Worker V2 build on `mi300x` also passes all three CTests.
This confirms the native LLVM/LLD protocol and bounded COV6 test boundary only;
it does not load or execute alpha or zeta and is not an archived parity result.

Commit `8f81306` fixed the earlier `emitObject` failure by making the measured
LLVM include directories and pinned major-version definition public usage
requirements of the worker protocol library. Every protocol consumer therefore
uses the same pinned LLVM headers, with a compile-time major-version check.
Commit `10a1fc8` propagates the exact requested target features into AMDGPU
`TargetMachine` creation and checks their ELF flags and AMDHSA metadata.

This older evidence is limited to the named MI300X host and unoptimized Debug
worker. Those integration tests compile, link, inspect, and publish `gfx942`
HSACO, and the runtime observation correlates the HIP and HSA views of the
device. They do not establish Verus proof, compiler refinement, or production
runtime authority.

Native worker CTests additionally link a descriptor-bearing COV6
request with two entries and one shared helper, preserve both metadata entries
and both `.kd` symbols, require AMDHSA ELF ABI version 4, and reject mismatched
descriptor metadata. This is a native LLVM/LLD boundary test, not archived
MI300X execution evidence. Canonical `.fe2o3.kd.v1` authentication is
implemented downstream. Exposing the Cargo Worker V2 artifact-container
adapter outside tests remains open.

Commits `d3d23fc` and `0e7d46e` close the two COV6 convention failures: the
finalizer accepts AMDHSA metadata that reports the explicit prefix while the
compiler descriptor commits the complete kernarg, and the worker canonicalizes
the post-optimization implicit ABI back to the complete 256-byte block. Commits
`f49b252` through `4acbcc5` add the bounded typed lowering, guards, exact named
ABI identity, generated host adapters, and authenticated logical/export role
selection. Commit `daf0b459` exercises those pieces in one genuine Worker V2
build, validates both backend witnesses, requires the exact alpha/zeta COV6
kernel set, and exports the newly created artifact without overwriting an
existing evidence file.

That exported `gfx942` COV6 HSACO has SHA-256
`3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
On the MI300X host, the opt-in raw hardware harness loaded one executable,
resolved alpha and zeta, and ran both kernels for lengths `1`, `255`, `256`,
`257`, and `1023`. Every case matched its independent CPU oracle and preserved
prefix/suffix canaries. This is valuable compiler, linker, ABI, and hardware
evidence for the exact digest and `gfx942` target. At `dc9738e`, a second run
passed the same matrix through generated checked slice capabilities, typed
preparation, the reviewed lifecycle, and safe dispatch. It uses test-only
semantic witnesses and an explicitly fake prerequisite authenticator, and both
harnesses inject an external digest-pinned HSACO. Neither run is evidence of
production proof-authenticated dispatch or CUDA-Oxide parity. Complete remains
`0`.

## Ordered Next Milestones

1. **Implemented: durable claim and lease recovery.** Reacquire a fresh non-clone lease only
   after revalidating the persisted receipt, complete plan, exact files,
   current generation, path identity, and lock.
2. **Implemented schema: canonical Worker V2 load envelope.** Preserve raw/final snapshots and
   encode the container, bundle/proof evidence, descriptor lineage, finalized
   identity, and durable claim. Never serialize the process-local lease.
3. **Next integration: production Cargo, recovered host admission, and application handoff.**
   Publish the envelope before clearing restart state, pass a read-only pinned
   descriptor, reacquire the lease, and re-run all structural, semantic,
   physical ABI, marker, and currentness checks. Remove the external-HSACO
   handoff from the generated-safe MI300X lane while retaining the explicit
   fake-authenticator label.
4. **Machine-code effect validation tied to evidence.** Analyze the finalized
   alpha/zeta payload and its closed call graph with a bounded, versioned
   validator. Bind the accepted global loads/stores, address derivations,
   descriptor/effect identities, analyzer/toolchain identity, and exact payload
   digest into machine-code evidence consumed by admission. Unknown calls,
   indirect memory effects, effect expansion, or any byte substitution fail
   closed. Hardware success remains separate evidence.
5. **Verus proofs and proof-artifact binding.** Prove bounds, address overflow
   freedom, injective writes/race freedom, and alpha/zeta functional results.
   Bind source, crate/kernel identity, ABI/effects, launch contract, Verus and
   solver identities, proof result, machine-code evidence, and finalized
   payload into the artifact. Negative source/proof mutations and stale proof
   replay must be rejected.
6. **Production prerequisite authenticator.** Join reviewed persistent Verus,
   measured compiler, Rust-layout, proof-to-executable, machine-effect, and
   rollback freshness evidence. Only this final joined value may implement the
   unsafe authenticator; it accepts no caller-provided evidence digest.
7. **Split mutable views.** Add a safe partition operation that can produce two
   simultaneous non-overlapping mutable views of one allocation while retaining
   parent allocation identity and exact byte regions. Cover overlap, overflow,
   lifetime escape, rejoin/drop order, packing, and in-flight alias rejection,
   then execute a same-allocation multi-view kernel on MI300X.
8. **Feature and architecture breadth.** Generalize the exact vertical slice
   only after the preceding authority and evidence gates: more signatures and
   control flow, aggregates, async/runtime features, core AMD operations, then
   `gfx1151` and `gfx950` compile/hardware lanes. Each addition needs explicit
   capability gating and its own negative, differential, and hardware evidence.

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

Current boundary: typed selection, generated exact alpha/zeta packing and
preparation, a linear two-symbol HSA lifecycle, and raw synchronous dispatch are
implemented. General V3 has emitted semantic witnesses, checked buffer views,
and signature-specific `Arguments` whose slice capabilities retain exact
allocation-relative regions. Both kernels execute from one shared HSACO against
independent CPU oracles on MI300X.

Finalized Worker V2 bundle admission, currentness leases, the authenticated
load state machine, generated alpha/zeta safe dispatch SPI, and the reviewed
runtime adapter already exist. The generated-safe MI300X test now exercises
those runtime pieces with explicit test authority. Canonical lease
reacquisition and the load-envelope schema now exist. This gate still requires
production Cargo and application handoff, recovered host admission, and a production
`WorkerV2PrerequisiteAuthenticatorV1`. Bidirectional
external-device FFI, `gfx1151`, machine-code effect evidence, and Verus
refinement also remain open.

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
