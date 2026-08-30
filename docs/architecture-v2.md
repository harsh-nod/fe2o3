# fe2o3 Architecture v2

Status: living architecture and implementation contract.

This document describes both the permanent architecture and the boundary of
the current implementation. Sections that describe an incomplete general form
are explicit about that status. Historical commit-specific checkpoints belong
in milestone documents and receipts, not in this living overview.

The v2 architecture preserves the working AMD runtime and artifact path while
replacing the expression recognizer with a general compiler pipeline. It also
adds a source-level verification path without placing the compiler inside the
Verus trust claim.

Related documents:

- [cuda-oxide parity matrix](cuda-oxide-parity-matrix.md)
- [production compiler convergence V1](production-pipeline-convergence-v1.md)
- [verification model](verification-model.md)
- [GPU safety contract v1](gpu-safety-contract-v1.md)
- [implementation roadmap](implementation-roadmap-v2.md)
- [general typed dispatch V1](general-typed-dispatch-v1.md)
- [compiler-execution deployment bundle V1](compiler-execution-deployment-bundle-v1.md)

## One Executable Architecture

The permanent architecture has one executable route:

```text
Rust kernel collection -> semantic MIR -> verified middle end -> Kernel IR
    -> typed AMDGPU/LLVM lowering -> upstream LLVM/LLD -> inspected HSACO
    -> Worker V3 verification -> generated typed pure-KFD dispatch
```

No Cargo feature, environment variable, macro option, workload profile, or
test configuration may select another compiler, publication, load, or launch
implementation. `Fresh`, `Recovered`, and `Ready` are restart states inside
this transaction, not pipeline variants. Host compilation through ordinary
rustc is a phase of the same Cargo plan and does not compile device kernels.

Version suffixes remain only where bytes cross an ownership boundary: wire
formats, canonical records, receipts, identities, and protocol messages. A new
schema version migrates the same transaction; it does not add a route. Useful
legacy comparisons may survive only as inert fixtures or offline differential
tools with no artifact, load, or launch authority.

The repository has completed this route deletion. Default and all-feature backend builds contain no qualification selector or executable workload oracle. `FE2O3_QUALIFICATION_ORACLE_V1` remains only as a rejected legacy sentinel. `fe2o3-hsaco-finalize` still houses shared Worker V3 mechanics under frozen V2 protocol names where the live V3 graph consumes them; those names are wire history, not selectable routes.

## Current Implementation Snapshot

The repository has implemented a bounded vertical realization of the v2
architecture, centered on exact `gfx942:xnack-` profiles:

- `#[kernel]` emits versioned registrations and generated host markers. The
  custom backend delegates ordinary host code to `rustc_codegen_llvm`, then
  discovers roots and collects reachable, monomorphized device MIR.
- The compiler has structured MIR import, rustc-derived ABI and layout
  evidence, verified Kernel IR, target-gated AMDGPU lowering, and exact
  fail-closed profiles for representative elementwise, scalar GEMM, tiled GEMM,
  row softmax, Flash Attention, MoE, Wave64 collective, LDS, and atomic slices.
- The production-directed finalizer runs outside rustc and uses one pinned
  upstream LLVM build for module linking, optimization, target-machine object
  emission, and in-process LLD linking. It neither uses COMGR nor shells out to
  `clang`, `llc`, or `ld.lld`.
- Every successful production Worker response retains a bounded derivation
  chain for the exact linked LLVM module, optimized LLVM module, generated
  object, ordered request relocatables plus generated object, canonical
  path-independent LLD invocation, and final HSACO. Rust independently decodes
  the record and recomputes its identity, request-derived object order, linker
  invocation, and final payload identity; strict Worker V3 replay also requires
  every measured stage identity to agree. Recovered host admission reconstructs
  the complete finalizer owner, binds its identity into host lineage, and lends
  it with the exact replay bytes to the protected verifier. The verifier repeats
  the reconstruction independently, promotion compares both identities, and
  the accepted decision retains the second move-only owner. These identities
  record exact content and policy custody; they do not establish semantic
  preservation or grant publication, load, or launch authority.
- Compiler provenance now has a canonical six-pin `CompilerClosureV2`, with
  an explicit Cargo-to-trampoline-to-wrapper transition protocol, and a
  `RustcInvocationDescriptorV3` that preserves the exact V2 process and
  environment while adding the complete closure. Protected release and the
  broker-to-wrapper raw sealed closure transfer are implemented. The wrapper
  constructs and seals V3 and installs it at fd 199 for rustc; the backend
  revalidates the exact process, target, role pins, and closure before
  production publication.
- Compiler-execution attestation has fixed canonical policy, challenge,
  request, and receipt records. The protected authority service admits one
  hardened nondumpable `no_new_privs` process, one private descriptor-only
  service boundary, the retained loader-independent static issuer executable,
  and an immutable service-owned sealed Ed25519 key that matches the caller
  policy. It rehashes the exact executable and key during continuity checks,
  owns a signed crash-safe freshness journal, and independently retains and
  revalidates an authority-free observation of the admitted rustc process and
  its three fixed capability descriptors. It joins that observation to the
  exact current production-slot V3 publication under a move-only lease and
  permits signing only from an internally constructed occurrence. A private
  guard retains publication currentness through signing and durable commit;
  public issuer methods accept no caller-selected occurrence. A separate
  descriptor-relative Worker ledger verifies the signed receipt against the
  exact request and current rollback anchor, durably commits only an immediate
  successor, and is the sole source of the issuer's move-only publication ACK
  capability. Before Worker commit, the issuer durably prepares the exact
  external-anchor challenge, exchanges it over the supervisor-admitted endpoint,
  and persists an exact signed proposed-position receipt. The canonical Worker
  V2 record atomically embeds that complete receipt, and recovery requires exact
  equality with the local anchor journal while preserving the current receipt
  across successor preparation. It then reacquires the canonical Worker record
  and published anchor journal. Legacy V1 records and record-without-journal
  state fail closed. Issuer recovery accepts
  only the exact cross-journal crash positions. The fixed canonical packet
  codec and allocation-free bounded `SOCK_SEQPACKET` service now consume that
  admitted issuer; all direct transition methods are private and exact replay
  resolves lost responses. A descriptor-only musl-static issuer entrypoint now
  enters through a syscall-only shim that restores nondumpability before musl
  or Rust startup and admits fixed FDs 3 through 11. FDs 10 and 11 carry the
  supervisor-admitted external-anchor endpoint and exact service pidfd. A sealed launch manifest
  binds exact rustc PID/UID/GID to the exact pinned policy, and the static build
  gate pins that shim as the ELF entry and rejects every dynamic-loader edge and undefined symbol. After complete
  admission and durable recovery, the issuer emits one canonical readiness
  record binding its PID, manifest, and policy through an atomic nonblocking
  pipe. The protected supervisor first performs authority-free program
  admission: it validates both source images, binds the issuer to the sealed
  policy, copies each into a distinct anonymous mode-0555 memfd, requires
  `F_SEAL_EXEC` and complete content seals, reopens it read-only, and repeats
  measurement and static-profile validation. A second move-only state binds
  that chain to the canonical sealed signing-key capability, dedicated non-root
  service UID/GID, and a retained service-owned mode-0700 root without exposing
  a descriptor, key, or signing operation. One gated
  `clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)` lifecycle now inherits and
  self-checks the exact locked child profile, independently observes that
  profile and every unchanged namespace in the parent, repeats authority
  continuity before release, installs the manifest and issuer at FDs 198 and
  199 and the twelve sources at FDs 200 through 211, admits exact
  readiness, and owns pidfd cancellation and exactly-once reaping. The
  backend consumes the inherited service and policy descriptors, acquires the
  exact receipt after V3 handoff publication, and carries it through the sole
  top-level V2 load envelope into host admission. Host lineage and the Worker V3
  verifier request now bind the exact subject, complete carriage, and
  independently reconstructed finalizer derivation. The
  application runner creates a separate child-bound fd 195, reaches the same
  fixed supervisor before ACK, exposes no policy fd 202, and retains readiness
  through exit. A one-use host auditor consumes that endpoint and verifies the
  issuer signature over a fresh challenge and the exact receipt-bearing V3
  current-record result. The protected service derives a client-verifiable
  external recovery challenge, queries the admitted anchor, then reacquires the
  Worker record before signing. The client independently verifies the retained
  advance receipt, fresh proposed-position recovery receipt, and every
  transition coordinate against its original carriage. The result authenticates
  the signed current-head observation but remains authority-free until
  independently administered monotonic deployment and protected-key custody are
  established. Promotion
  compares every receipt, occurrence, Worker-ledger, sequence, and rollback
  coordinate and requires nonzero independent protected-policy, ledger, and
  external rollback verification identities. The concrete protected verifier,
  privileged distinct-UID deployment and real Cargo-to-KFD qualification remain
  open.
- Production has one unselected compilation transaction. Cargo owns it as
  `ManagedProductionBuild`, whose `Fresh`, `Recovered`, and `Ready` values are
  restart states rather than pipeline variants. The backend configuration and
  kernel-containing codegen path are identical in default and all-feature
  library builds. Legacy compiler workload modules can compile only inside a
  feature-enabled unit-test binary; the Cargo and host Worker V2 application
  transfer, consumer, retained descriptors, and compatibility error alias have
  been deleted from every build.
- Default `cargo-fe2o3` unit and integration tests compile this same production
  transaction. Backend unit tests may inspect temporary qualification fixtures,
  but `cfg(test)` and Cargo features no longer change compiler or runtime
  routing.
- The explicit rustc extraction driver is feature-independent and enters the
  same `ProductionCompilation` typestate as backend codegen. Its
  `ExtractionOnly` custody can run general checks, semantic MIR import, ranked
  projection, verified Kernel IR lowering, and deterministic gfx942 LLVM
  extraction, but cannot publish a compiler-module handoff. Target binding and
  KIR-to-LLVM serialization are shared deterministic transforms. The compiler
  records exact neutral/target KIR identities, profile, kernel ID, and LLVM;
  `fe2o3-verifier` independently replays both transforms and compares the exact
  result. Real AMDGPU tests cover safe and unsafe collection, dynamic ranked
  bounds, reference binding, and a loop-carried BF16/F32 MFMA GEMM.
- A protected continuation of that transaction now carries the WG64 `i32` LDS
  reduction through a compiler-bound inert handoff, measured upstream LLVM
  target APIs, in-process LLD, and COV6 inspection. The source, semantic MIR,
  Kernel IR, LLVM LDS allocation, and AMDHSA descriptor all agree on 256 static
  group bytes. The sibling scoped-atomic kernel uses the same continuation.
  This is the first bounded source-authentic workgroup code-object slice, not a
  second compiler pipeline. It grants no load or launch authority; current
  execution must be added through Worker V3 and the KFD runtime.
- Production orchestration has one fixed Cargo plan. The first phase always
  builds the selected crate graph for `amdgcn-amd-amdhsa` through the fe2o3
  backend and commits its generated-artifact generation. The second phase
  builds or runs the same selection for the pinned rustc host target with
  ordinary rustc and no device compiler controls. Users cannot select either
  target or choose a different ordering.
- `fe2o3-compiler-execution-protocol` owns the canonical inert issuer policy,
  attestation, receipt carriage, and bounded service packets.
  `fe2o3-runtime-protocol` owns the production load envelope, application
  handoff, and sealed static-application identity, and re-exports the compiler
  records for its existing envelope API. Feature-free `cargo-fe2o3`
  and `fe2o3-host` share the same Worker V3 runtime envelope. The retired Worker V2 bundle is no longer a workspace package. V1/V2/V3 suffixes that remain on
  records are frozen wire versions, not selectable compiler implementations.
- `fe2o3-compiler-execution-client` owns the one bounded client state machine
  for protected compiler receipt recovery and issuance. It resumes durable
  Ready, Prepared, or Issued state over one unnamed `SOCK_SEQPACKET` peer; it is
  a transport component of the production transaction, not another compiler
  pipeline. Its direct-parent handoff creates that socketpair in the post-fork
  selected child, transfers only the service endpoint, and binds it to exact
  child credentials and a live pidfd. The protected supervisor authenticates
  that handoff and now materializes the exact sealed twelve-source static-launcher input,
  including the separately admitted external-anchor endpoint and pidfd,
  while retaining private output/readiness endpoints. It now consumes that
  state through gated clone3/pidfd launch, exact readiness typestate, and a
  descriptor-free one-record readiness publication back over the authenticated
  Cargo control connection. The supervisor retains the same pidfd in serving
  custody after Cargo observes EOF. The Cargo wrapper now admits a fixed
  root-owned client profile, installs policy fd 202 and a child-created service
  channel at fd 195 for the selected rustc, authenticates the fixed listener's
  distinct UID/GID, and gates fresh publication on exact readiness. The
  application runner uses a fresh child-created fd 195 through the same
  supervisor without inheriting fd 202, and `fe2o3-host` can consume its signed
  challenge-bound receipt-bearing current-record response as move-only audit
  evidence. The external anchor now has a durable single-writer transition
  engine, exact connected-packet service loop, sealed deployment manifest, and a
  role-separated signing-key capability bound to that manifest. Its
  descriptor-only process entrypoint admits the shared locked profile and exact
  deployment-bound sealed executable before reading the key, opens only existing
  state, closes all unrelated descriptors, and serves the sole exact peer. Its
  production persistence and packet paths have exhaustive injected-crash coverage
  around cleanup, create, write, file sync, rename, directory sync, receive,
  exchange, and send. Restart admits only the exact prior or proposed state and
  exact challenge replay advances at most once. The distinct-UID root coordinator,
  endpoint/pidfd transfer, measured supervisor spawn, profile gate, canonical
  readiness, and combined supervisor/anchor custody are implemented. Exact
  service-account/socket provisioning, combined privileged qualification, and
  final verifier authority remain pending.
- The compiler-execution deployment builder publishes one exact 14-file static
  source bundle only after a separate static musl verifier admits its
  caller-pinned canonical manifest and Git commit. Admission is
  descriptor-relative, rejects alternate inventory and metadata, double-reads
  every bounded file, cross-checks `BUILD-INFO` and `SHA256SUMS`, and retains
  the manifest and 13 content files in sealed anonymous custody. A root-only
  static installer consumes only that custody, constructs and verifies one
  exact 12-directory/14-file offline root, synchronizes it bottom-up, and
  publishes the complete content-addressed root with one durable no-replace
  rename. Exact existing roots are revalidated and reacquired; conflicting
  roots are never replaced. The installed value retains its sealed evidence for
  fresh revalidation. A deterministic 71-package Ubuntu 24.04 systemd base is
  independently digest-pinned, copied into a sealed memfd after two identical
  reads, checked for the exact SquashFS V4 profile, and retained with an empty
  root-owned qualification-parent descriptor. A root-only transaction then
  creates and descriptor-retains the exact empty base/root mount points and
  disposable upper/work/run/state/evidence directories; its 21-checkpoint fault
  campaign always restores an empty parent. This closes disposable-root
  preparation and staging, not mount composition or live system deployment:
  isolated boot and root/distinct-UID systemd execution qualification remain
  open.
- Versioned artifact, descriptor, durable-publication, and HSA records exist.
  Host execution has one workload-neutral Worker V3 graph. An arbitrary
  manifest cannot manufacture a Rust signature, verifier decision, load
  authorization, or dispatch authority.
- `fe2o3-runtime` now owns one invocation-specific pure-KFD authority gate. It
  binds the exact object and length, selected kernel closure, materialized
  image, kernarg, initial buffer bytes and declared effects, pointer fixups,
  geometry, resources, timeout, KFD mechanics manifest, and checked GPU unique
  ID. The SHA-pinned LDS diagnostic passes this gate on MI300X using a manually
  asserted unsafe authority. `fe2o3-host` now privately implements that
  authority only for a joined, move-only invocation constructed from an
  authenticated Worker V3 executable, macro-generated arguments, retained
  current publication, runtime preparation, and the same checked device. A
  scalar-GEMM test passes this path only with the explicit synthetic-verifier
  test feature. The production compiler now places a V4 proof association in
  the frozen V3 capsule envelope. Worker V3 independently decodes the five
  exact compiler stages, reimports the nested signed aggregate
  MIR-to-live-PLIRON receipt under its embedded key, checks its PLIRON identity
  against middle-end V5, and retains that move-only owner beside signed
  compiler-currentness evidence throughout the HSA lifecycle. Host admission
  and the protected verifier also independently reconstruct the exact compact
  finalizer derivation; the accepted decision retains the verifier-owned result
  and rejects a foreign derivation even when the finalized HSACO bytes match.
  The production
  capsule also replaces the backend-private association-only AMDGPU transcript
  with the independently replayed target-KIR-to-LLVM record. This proves exact
  deterministic derivation by the reviewed serializer. The Worker continuation
  now also independently validates the response identity, request-derived input
  order, LLD policy, final HSACO, and bootstrap/replay equality for the measured
  linked-module, optimized-module, and object identities. Neither custody chain
  is formal semantic preservation. The signature does not authenticate compiler
  origin by itself and grants no machine, load, or launch authority. In default builds the
  verifier trait is sealed against external implementations and the decision
  constructor is crate-private. No reviewed concrete production verifier
  exists yet, so ordinary generated application execution remains fail-closed
  rather than accepting caller-asserted hashes or safety bits.
- Verus models and proof-carrying artifact schemas exist for bounded kernels
  and safety obligations. There is no general reviewed source-to-machine or
  Verus-to-machine refinement proof, so source proof, compiler evidence,
  machine-code inspection, and GPU execution remain separate claims.
- The ownership refactor establishes canonical MIR, compiler, proof,
  host-operation, and service-model contracts; an explicit compiler
  API and sole managed Worker V3 composition boundary; a pinned Pliron D0 shell; seven
  target-neutral dialect shells; a feature-gated `mir.*` Pliron shell; an
  opaque context-bound exact-byte KIR/Pliron envelope; bounded MIR-to-kernel
  and kernel-to-GPU lowering services; an owner-held textual bridge; and an
  authority-free service-host typestate adapter. The rustc frontend retains an
  exact typed MIR/CFG graph for a return-only supported subset and rejects all
  other observed MIR semantics terminally.
- The Pliron LLVM lane has a live graph-derived extractor, deterministic bounded
  LLVM-assembly serializer, and a workload-neutral production handoff. Its
  request bridge feeds the sole Worker V3 production transaction.
  `pliron-llvm` v0.17.0 is used with
  `default-features = false` for its typed dialect only. The bridge binds the
  exact request but grants no object, link, publication, load, or launch
  authority.
- The closed gfx942 General GEMM profile retains target, module, global,
  function, CFG, instruction, type, and per-item policy on the live graph, with
  separately hashed bounded non-graph inputs for stage identities, device
  libraries, origins, and obligations. Its historical Worker V2 realization has been removed; the bounded semantics now enter the workload-neutral production handoff.
- Historical bounded scalar-add Worker V2 execution remains documented only in repository history. Its standalone join and runtime consumer were retired after the production graph converged on the shared Rust-source Worker V3 path.
- The exact MI300X run completed with
  `evidence=69238ad704470649b9811b41cf0194bb392be8116a1b0618adb1dcbe7e1bbd4f`
  against ROCr 1.18 runtime image
  `7010eba894569c044749b71b63ff782080c4a91e19ff24d6dc93e857045ab37e`.
  This closes the bounded #159 finalization and #161 execution slices. The
  embedded backend fixture is structurally parsed into the typed scalar model,
  but it is not Rust user source. The checkout policy and success marker are
  repository-consistency records, not an external signature or CI attestation.

This is not general Rust GPU compilation or cuda-oxide parity. The exact
implemented and missing surfaces are maintained in the
[cuda-oxide parity matrix](cuda-oxide-parity-matrix.md); reproducible commands
and strength labels are defined by the [testing guide](testing.md) and parity
evidence policy.

Issues [#134](https://github.com/harsh-nod/fe2o3/issues/134) and
[#135](https://github.com/harsh-nod/fe2o3/issues/135) remain open. The
[workspace ownership policy](workspace-layers-and-ownership.md) records which
infrastructure has landed and which production stages remain reserved.

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

### `90b6fe3` multi-kernel checkpoint

The current implementation has a bounded realization of steps 1, 2, and 4 for
one `gfx942` profile. An external Cargo fixture supplies two kernel roots and a
shared helper. The frontend gives the helper one canonical source identity;
Kernel IR lowering checks each internal call against the collected helper's
declared signature; and the direct upstream LLVM/LLD production transaction
emits one inspected, durably published HSACO. The canonical V1 artifact
container then represents
two kernel entries over that one native payload, with an independently keyed
proof binding for each entry.

Host admission can select either compiler-generated kernel marker while
retaining the exact artifact, executable, target, physical layout, effects,
and launch identities. The HSA adapter can resolve a fixed set of distinct
symbols and returns a non-clone set that borrows the executable, preventing
safe unload while any selected native kernel is retained.

The boundary is intentionally incomplete. The second host selection is inert,
the HSA set establishes native identity rather than typed ABI authority, and
dispatch still uses the exact vecadd physical layout and hidden-argument
initializer. The checkpoint therefore demonstrates a multi-kernel compiler,
artifact, selection, and lifecycle spine, not general safe multi-kernel
execution or cuda-oxide parity.

## Permanent Component Boundaries

The crate names below are current ownership boundaries. Dependencies must
continue to point downward according to the machine-checked
[workspace layer policy](workspace-layers-and-ownership.md).

| Component | Responsibility | Must not own |
|:--|:--|:--|
| `fe2o3-rustc-front` | Kernel discovery, final mono-item collection, `rustc_public` conversion, source spans | GPU lowering, host launch packing |
| `fe2o3-mir-model` | Pliron-independent semantic MIR types, executable schema/wire, control-flow analysis, and mem2reg | Pliron handles, AMD lowering, runtime handles |
| `dialect-mir` | Historical MIR compatibility facade; optional bounded Pliron `mir.*` shell behind feature `pliron` | Durable MIR identity, production selection, target lowering |
| `fe2o3-kernel-ir` | Canonical target-neutral Kernel IR, SIMT domains, effects, address spaces, barriers, atomics, and capabilities | Pliron identity, Rust compiler types, HIP calls |
| `fe2o3-pliron` | Pinned Pliron context, private context identities, registration, and bounded pass-plan validation | Generic pass execution over contextless pointers, fe2o3 dialect semantics, production selection, artifact authority |
| `dialect-kernel`, `dialect-schedule`, `dialect-tile`, `dialect-gpu`, `dialect-proof`, `dialect-dispatch`, `dialect-autotune` | Bounded target-neutral Pliron representation shells | Target legalization, compiler selection, proof or runtime authority |
| Production KIR custody | Canonical KIR remains owned by the sole compiler transaction | Accepting detached raw Pliron modules, reconstructing KIR from text, target or artifact authority |
| `fe2o3-lower-mir-kernel` | Narrow deterministic MIR-to-kernel conformance service with context-bound results and terminal unsupported errors | In-tree Pliron pass semantics, production selection, AMD lowering, artifact production, fallback |
| `fe2o3-amd-target` | Canonical AMD target identities, features, and capability contracts | Compiler execution and runtime observation |
| `fe2o3-amdgcn-model` | Existing strict AMDGPU vocabulary, legalization/lowering, OCML/OCKL selection, and LLVM text generation | Pliron object identity, host borrow policy, artifact/launch authority |
| `dialect-amdgcn` | Compatibility re-export of `fe2o3-amdgcn-model` | Claiming an implemented `amdgcn.*` Pliron dialect |
| `fe2o3-compiler-api` | Target-neutral request, snapshot, receipt, diagnostic, and output contracts | Running a compiler or publishing its candidate |
| `fe2o3-build-authority`, `fe2o3-rustc-invocation`, `fe2o3-compiler-execution-protocol`, `fe2o3-compiler-closure-capability`, `fe2o3-artifact-transaction` | Canonical compiler provenance, exact invocation, inert execution-attestation records, sealed closure/invocation/policy/launch coordination, move-only sealed signing-key custody, and attempt-scoped handoff/publication records | Compiler semantics, LLVM execution, artifact authorship, signing operations, receipt issuance, or load/launch authority |
| `fe2o3-artifacts` | Versioned neutral bundle and identity records | Compilation and loading policy |
| `fe2o3-host` | Generated Worker V3 arguments, verifier admission, argument ownership, the private joined KFD invocation authority, and the HSA-backed migration implementation | MIR inspection, target lowering, verifier proof production, or raw launch authority |
| `fe2o3-runtime` | Sole safe pure-KFD composition boundary, invocation identity, authority matching, effect-preserving completed buffers, and terminal execution policy | Constructing Worker V3 proof authority or accepting caller-asserted descriptive identities |
| `fe2o3-core` | HIP resource wrappers, streams, events, buffers, and capability observations; the default/production surface keeps raw module and launch mechanics private, while `qualification-unsafe-launch` is currently enabled only by the checked-in standalone external-HSACO numerical examples | Kernel type discovery, Worker V3 publication, protected execution, artifact-currentness, or downstream production raw-launch authority |
| `fe2o3-host-api` | Inert target-neutral compile/admit/load/dispatch/wait records | Executing those operations or authenticating authority |
| `fe2o3-service-model`, `fe2o3-service-host` | Executable-free service semantics and authority-free borrow-retaining host typestates | Persistent execution, runtime waits, progress proof, storage-release authority |
| `fe2o3-contracts`, `fe2o3-proof-contracts` | Shared launch/spec vocabulary, erased proof markers, and solver-neutral property records | Solving proofs, code generation, proof promotion |
| `fe2o3-verifier` | Verus invocation, policy checks, proof manifest creation | Claiming compiler correctness |
| `cargo-fe2o3` | Build graph orchestration, tool discovery, cache keys, inspection commands | Semantic lowering logic |

`fe2o3-hip-sys` remains the narrow raw FFI layer. The current
`rustc-codegen-fe2o3` can host adapters while the new layers are introduced,
but it is not the permanent owner of host compilation. At this checkpoint it
still owns the working production compiler composition; the new compiler
driver is not wired into that selection path.

## Frontend and Device Extraction

### Explicit extraction driver

The explicit rustc driver invokes the fixed AMDGPU rustc session, collects the
final application's monomorphized kernel roots and reachable device closure,
and enters the same production collector, semantic importer, checks, and
typestate transaction as backend codegen. Normal host compilation still uses
ordinary rustc and does not run through fe2o3's codegen backend.

The driver is an analysis and evidence entry, not another compiler pipeline.
Its extraction-only custody cannot publish, finalize, or launch. The backend
remains the authority-bearing integration point until publication and
finalization are moved behind a workload-neutral transaction owner shared by
the fixed production orchestration.

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

The target architecture uses Pliron because the pinned cuda-oxide
baseline already demonstrates a Rust-native MIR importer, verification,
mem2reg, dialect conversion, and LLVM export. V2 must keep serialization and
pass interfaces independent of Pliron object identity. That boundary permits a
future MLIR lower half without changing the source API, artifact manifest, or
verification model.

The current implementation pins Pliron v0.17.0 commit
`5bdf861bf03e7f20242b25717fb653336d02e487` and provides a bounded context,
private context-identity, registration, and pass-plan shell. Generic pass
execution is intentionally absent because upstream `Ptr<T>` values do not
carry owner provenance; [#140](https://github.com/harsh-nod/fe2o3/issues/140)
tracks that prerequisite. Seven target-neutral operation-family shells and the feature-gated
`dialect-mir` shell construct and verify in-memory Pliron values. They do not
yet import general rustc MIR, run the target pipeline above, lower to AMDGPU,
or emit an executable. Separately, the closed scalar slice constructs real
`pliron-llvm` operations and derives a canonical executable handoff from their
live graph; that bounded path is not a general MIR-to-AMDGPU pipeline.

The target architecture is selective rather than a blanket exclusion. Only
the Pliron LLVM dialect/lowering layer may use `pliron-llvm`, and every such
dependency MUST use `default-features = false`. The optional `llvm-sys`
converter is not part of the production route, including inside the isolated
worker. `pliron-llvm` owns only transient `llvm.*` representation and dialect
verification. fe2o3 owns the bounded canonical V2 handoff, stable identities,
stage receipts, evidence, and deterministic bounded LLVM-assembly serializer;
Pliron handles, printer output, and upstream diagnostics are not canonical
handoff data or authority.

The retired scalar-add fixture route parsed a checked-in backend fixture rather than Rust user source and never established general Rust-source lowering. Its dedicated finalizer and one-shot consumer have been removed. Current kernels enter the shared ranked-PLIRON and KIR path from attributed Rust source before using the common target backend.
The pinned surface and missing gfx942 semantics are audited in
[pliron-llvm-gfx942-coverage.md](pliron-llvm-gfx942-coverage.md).

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

Multiple entries may reference one native payload. Shared payload identity does
not merge entry authority: kernel ID, exported symbol, source identity, ABI,
effects, launch contract, and proof key remain independently bound. A module
loader owns the executable; selected kernel values borrow that loader and add
their entry-specific identity. Duplicate names, symbols, native kernel objects,
or proof keys fail closed rather than aliasing two logical entries.

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

### Descriptor-driven multi-kernel dispatch

Bounded generated declarations, finalized descriptors, multi-entry artifacts,
and typed preparation exist for reviewed profiles. The general rule remains:
the macro-generated declaration and finalized entry descriptor are the only
safe route from Rust arguments to kernarg bytes. Ordinary rustc compiles that
declaration without a custom-backend host object. Worker V3 independently
matches its binding and complete argument layout to the admitted compiler
descriptor. The serialized manifest is untrusted input until that match and
independent code-object inspection succeed. A loader must never create a safe
Rust signature by interpreting manifest bytes alone. The exact V1 accepted argument profiles,
authority transitions, rejection suite, and remaining exit gate are specified
by the
[general typed dispatch contract](general-typed-dispatch-v1.md).

The production transition is:

```text
inherited Cargo Worker V3 handoff + generated kernel type
        |
        v
RecoveredWorkerV3PinnedDescriptorV1
        + generated expectation + production verifier
        |
        v
AuthenticatedWorkerV3ExecutableV1<K>
        + generated host-memory arguments + geometry
        + CheckedGfx942XnackMinusDevice
        |
        v
GeneratedWorkerV3KfdInvocation<'allocation, K>
        |
        v
GeneratedWorkerV3KfdInvocation::execute
        |
        v
completed buffers after queue teardown
```

The concrete type names may evolve, but these ownership rules do not:

1. Recovery pins one durable publication, descriptor, target, and application
   handoff lineage before verification begins, but carries no physical device
   identity. Verification proves a theorem for the admitted target rather than
   one observed device. The KFD application transition consumes the exact
   checked physical device only when invocation authority is created; the
   temporary HSA migration route separately owns its HIP observation at HSA
   authorization.
2. Authentication binds one generated expectation to the recovered compiler,
   proof, effect, and executable evidence. Invocation authorization consumes
   that exact decision; no intermediate authority is cloneable or
   caller-created.
3. Generated adapters bind values by source argument index to the existing
   `GeneratedArgumentPackingPlanV1`. The plan writes explicit fields by checked
   descriptor offsets, zeroes padding, preserves scalar bit patterns, and
   retains every buffer borrow, provenance witness, mutability/effect class,
   and alias admission. The implemented KFD specialization encodes host values
   explicitly little-endian, leaves pointer fields zero until KFD allocation,
   derives fixup offsets from the admitted physical components, and retains
   exclusive output borrows through validated completion. It cannot be
   substituted for the HSA specialization and grants no execution authority by
   itself; only the joined transition can construct the private runtime
   authority.
4. The runtime initializes only the loader-inspected implicit COV6 region and
   verifies the complete kernarg size/alignment, selected descriptor, and
   static-plus-dynamic resources. It knows no Rust signature; the generated
   invocation and Worker V3 authority must independently name the same complete
   address-free contract.
5. Preparation binds geometry and packed bytes to the exact kernel, executable,
   ABI, launch contract, retained publication generation, and checked KFD
   device. Values from another entry, publication, or device are not
   interchangeable even when their bytes match.
6. Dispatch consumes launch authority, publishes one AQL packet, and releases
   resources only after quiescence. The executable cannot unload while a
   prepared or submitted invocation is live.

This design makes vecadd one generated instance of the same general rule. It
does not make every Rust type device-safe: G2 still controls which layouts and
language semantics the compiler accepts. The retired host raw-dispatch escape
hatch is not part of any build.

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

The historical compatibility path used ROCm command-line clang and `ld.lld`.
The production-directed link path uses an out-of-process, pinned fe2o3 worker
with upstream LLVM 22.1.8 that calls LLVM module-linking,
optimization, target-machine, and LLD library APIs directly. The worker keeps
ROCm LLVM out of rustc's process, where rustc's independently built LLVM is
already loaded. Requests are bounded canonical records with exact input,
target, option, symbol-resolution, toolchain, and output identities; they do
not contain shell commands, arbitrary linker flags, or implicit library search
paths. COMGR is not part of this architecture. Textual LLVM emission may remain
as an inspection format but is not the semantic IR boundary. For the selective
scalar slice, deterministic bounded LLVM assembly is the worker transport; the
canonical V2 handoff remains the semantic boundary, and the exact assembly
bytes and digest are bound to its identity.

The successful compiler response also binds the exact serialized linked and
optimized modules, generated object, ordered native-link inputs, canonical
path-independent LLD argument sequence, and returned HSACO. The generated
object is required to be the final ordered input. Fixed policy arguments and
the real `lld::lldMain` invocation share one construction path, so policy drift
changes the custody identity. Rust reconstructs the expected input order and
linker identity independently before accepting the response or a durable
Worker V3 replay, then requires all worker-measured stage identities to remain
equal across replay. This closes the measured LLVM-to-HSACO derivation-custody
gap; formal LLVM-to-machine refinement remains a separate proof obligation.

The existing lowering implementation is owned by `fe2o3-amdgcn-model` and
re-exported by the historical `dialect-amdgcn` facade. A future general
`amdgcn.*` Pliron dialect and its `gpu.*` lowering must preserve this finalizer
boundary. The implemented scalar slice uses selective `pliron-llvm` only to
construct and verify transient `llvm.*`, then uses fe2o3's canonical V2
handoff and serializer. Neither the producer nor the worker invokes the
`pliron-llvm` `llvm-sys` converter. The isolated measured upstream LLVM 22.1.8
target machine and its in-process LLD remain the sole machine-code and HSACO
authority; the dialect layer must not invoke LLVM code generation, COMGR, or
shell-mediated GPU linking. The bridge remains a non-authoritative request
binder. Commits `fd6520d88`, `70f9c5ad7`, `e016833d3`, `c9e8ca702`,
`62efd243e`, and `228c88ed9` close one exact scalar backend-fixture route
through MI300X load, dispatch, wait, and unload. Its checkout policy and marker
are not externally authenticated, and the result makes no CUDA-Oxide parity,
general memory-safety, or race-freedom claim.

## Remaining Migration from Bootstrap Paths

### Retain During Migration

- HIP/HSA initialization, streams, buffers, modules, errors, and HSACO loading
  only as isolated offline qualification oracles with no production authority;
- ROCm discovery and target detection in `cargo-fe2o3`;
- kernel root and reachable-call collection tests;
- current examples as end-to-end regression cases;
- LLVM-to-HSACO finalization and metadata inspection;
- the current emitter as a differential bootstrap oracle.

### Replace

- flat `MirOpRecord` streams with typed `mir.*` operations;
- record sketches and elementwise expression recognition with general passes;
- direct textual elementwise LLVM templates with `gpu.*` to AMDGPU lowering;
- filename-based sidecar discovery with authenticated Worker V3 envelopes;
- every legacy raw or typed-prepared launch surface with generated Worker V3
  host-memory arguments admitted by the one application verifier and pure-KFD
  runtime graph;
- the HSA-backed Worker V3 load/dispatch migration implementation with the
  invocation-specific pure-KFD authority transition.

### Redesign

- `ThreadIndex` as a branded, non-transferable index-space witness;
- `DisjointSlice` as an allocation- and index-space-aware writable view;
- `#[kernel]` to generate stable metadata and cooperate with erased contracts;
- host async APIs so Rust lifetimes cover queued device execution;
- build caching around complete source/proof/target/toolchain identities.

The production backend has one structured compiler route. Frozen wire versions
and qualification-only oracles remain for compatibility and evidence, but they
are not selectable production compiler implementations.

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
