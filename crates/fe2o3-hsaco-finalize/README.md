# fe2o3-hsaco-finalize

`fe2o3-hsaco-finalize` performs bounded post-link finalization of an already embedded canonical
`DeviceDescriptorTableV1` in an AMDHSA HSACO. The one normative ELF section is
`.fe2o3.kd.v1`. It is an 8-byte-aligned, file-backed `SHT_PROGBITS` section with no ELF flags,
so it is neither allocated, writable, executable, nor compressed.

The finalizer accepts at most `fe2o3_hsaco::MAX_HSACO_BYTES` and one descriptor table of at most
`fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES`. V1 deliberately clones the bounded whole
file. It hashes the complete HSACO under `FE2O3/AMDHSA-CODE-OBJECT/V1\0` with only the schema's
fixed 32-byte digest field zeroed, patches that field, then independently reparses, reinspects,
decodes, and recomputes the result. This canonical digest is distinct from any transport or raw
payload digest.

Every path first uses `fe2o3_hsaco::inspect_and_bind_kernel_descriptors`, so every metadata kernel
must resolve through real `STT_FUNC` and `STT_OBJECT` symbols, RO/RX load mappings, and a valid
64-byte AMDHSA kernel descriptor. The embedded table must then agree with that bound evidence on
code-object version, canonical target, complete kernel-name/symbol closure, kernarg size and
alignment, flattened explicit argument order/kind/offset/size/address/access/alignment facts, static
group memory, and represented launch constraints. Optional pointee alignment is checked against the
canonical source element alignment. When present, LLVM `.access` must equal the canonical declared
contract; optimized `.actual_access` may narrow a read-write contract but may never broaden it.
Absence of either field is absence of evidence. True volatile or pipe qualifiers fail closed because
V1 has no canonical representation for either; source type-name strings are not compared. V1 has no
wavefront-size field, so the binding layer checks that fact between metadata and the AMDHSA
descriptor while the table adds no second declaration to compare.

The V1 table intentionally describes caller-provided host arguments. Runtime-populated hidden
arguments are excluded from its flattened argument list, but their boundary and the complete
kernarg size remain checked against metadata. Evidence identities, evidence digests, capabilities,
producer strings, and source identities remain untrusted declarations. Finalization proves only
internal byte integrity and declared metadata closure. It is not Verus verification, compiler
attestation, module-load authority, launch authority, or evidence that a target device matches.

Bounded `rustc-codegen-fe2o3` profiles now construct the canonical table and embed exactly one
zero-digest `.fe2o3.kd.v1` section in the compiler-owned LLVM module. The production-directed
worker emits the object through pinned upstream LLVM target-machine APIs and links it through the
in-process LLD library API. `cargo-fe2o3` invokes this post-link finalizer for descriptor-bearing
COV6 output before publication. This is exact-profile plumbing, not general descriptor derivation
or compiler-correctness evidence.

## Multi-input native link plans

`MultiInputLinkPlanV1` is a linker-independent description of a reproducible native link.
It binds a canonical concrete AMD target to one or more SHA-256-addressed AMDGPU relocatable inputs,
bounded structured options, an expected executable HSACO identity, and a complete provenance DAG.
Inputs, options, nodes, and parent edges have one canonical order. Duplicate inputs, conflicting
digest lengths, conflicting options, target mismatches, output/input aliasing, unknown parents,
cycles, orphan nodes, and incomplete output-to-input closure fail closed. The output node's direct
parents must be exactly the complete input set.

The plan has a domain-separated stable identity and canonical byte representation. It can verify a
candidate output's expected digest and size without executing a linker. A direct LLVM/LLD worker remains
responsible for mapping each supported option through a structured API, preserving the canonical
input order, inspecting the produced AMDGPU object, and independently finalizing its embedded
descriptor table. A plan does not prove that LLVM/LLD ran, that an option is supported, that the bytes
are valid AMDGPU ELF, or that any device can load or launch them. The existing single-HSACO
inspection and finalization functions are unchanged.

## Compiler FFI claims and handoff

`rustc-codegen-fe2o3` now constructs a real `CompilerFfiEnvelopeV1` from its private successful
`CollectionResult` and `DeviceFfiClosure`. The LLVM-free neutral type lives in
`fe2o3-compiler-ffi`. It commits to the canonical target and code-object version plus every import
and export's shared contract ID, direction, explicit required-definition role, source owner, symbol,
physical ABI, effects/effect-ABI identity, and semantic identity. It contains no artifact provider,
input kind, expected final symbol, bitcode, or Worker V1 claim.

`stage_compiler_ffi_envelope_v1` consumes and privately retains the complete envelope. Its public
surface exposes only a domain-separated staged identity and target/version/count/blocker inspection.
It cannot reveal contract or generic linker closures and cannot create or bind Worker V1 evidence.
Because the neutral envelope has public constructors, staging does not authenticate rustc origin.
The live managed Worker V2 path does not upgrade this caller-constructible staging surface. Instead,
rustc publishes an attempt-scoped `CompilerModuleHandoffV2` containing the exact textual LLVM module,
the complete envelope, and a compiler-derived symbol-role manifest. `cargo-fe2o3` consumes that
handoff exactly once for the matching producer and build attempt.

The older `G4FfiClaimEnvelopeV1` path below remains caller-constructible assertion-only plan
scaffolding. It is not the real rustc observation and cannot upgrade generic Worker V1 evidence.

`G4FfiClaimEnvelopeV1` is the exact public contract for a future adapter from private
`rustc-codegen-fe2o3::CollectionResult` and `DeviceFfiClosure` state. That legacy trait remains
unimplemented; the real adapter produces the separate neutral envelope described above. All values
entering the legacy path are labeled assertion-only caller claims; G4 wording does not make them
compiler attestations. Each symbol retains its authoritative `reserved-fe2o3-symbols` contract ID,
direction, exact physical ABI grammar, target, code-object version, effects, semantic identity,
declaration owner, and provider-class claim. Declaration ownership is separate from unauthenticated
artifact producer metadata. Symbol, physical-ABI, effects, and effect-to-pointer compatibility use
the same typed V1 parser as `fe2o3-macros` and `rustc-codegen-fe2o3`, exported by
`reserved-fe2o3-symbols`.

Compiler-required symbols are deliberately distinct from exact expected final defined symbols. The
latter remain absent unless the caller supplies `ExpectedFinalDefinedSymbolsClaimV1` with exact
identity-and-kind coverage for every canonical plan input, attributed to bounded inspection or an
authenticated-manifest claim. This crate validates structural coverage but does not authenticate the
evidence source.

`stage_g4_ffi_link_plan_v1` matches exact input identities, kinds, roles, producers, symbol providers,
target, code-object version, ordering, cardinality, aggregate bounds, and optional all-input symbol
evidence. Rust definitions or kernels require exactly one neutral `CompilerModule` input claim. That
role does not imply LLVM bitcode. This legacy G4 assertion path still does not carry the exact
managed module; the separate live Worker V2 handoff carries one compiler-derived textual module
with explicit kernel, helper, export, and import roles.

Successful staging returns only `StagedFfiLinkPlanV1`. Its public surface exposes the complete staged
identity and a non-authoritative count/blocker summary. Raw plan, input, provider, symbol-evidence,
canonical-byte, and reduced-closure fields are inaccessible. It cannot call
`construct_worker_request_v1` or consume a Worker V1 output.

Every `WorkerRequestV1`, `WorkerResponseV1`, and `WorkerOutputV1` is permanently classified as
`WorkerEvidenceClassV1::GenericLink`. Worker V1 has no field for the complete staged FFI identity, so
generic evidence can never satisfy an FFI-bound evidence API. A caller can independently construct a
generic request with similar inputs and symbol strings, but that request and its output carry zero
FFI provenance. Its API and wire bytes are unchanged, and no V1-to-V2 conversion exists.

## Worker V3 HSACO admission and publication

Worker Protocol V2 remains the supervised LLVM/LLD engine wire format used by the
current compiler handoff. It is not a public artifact route. The standalone raw
Worker V2 inspection, canonical-finalization, and publication APIs have been
retired, along with exact tiled-GEMM, row-softmax, and workgroup-sync finalizer
specializations.

`execute_protected_reproducible_first_build_worker_v3` is the production
transaction entry. It consumes `ConsumedCompilerModuleHandoffV3` directly and
accepts no V1 or V2 transaction fallback. The caller provides the exact durable
V3 publication receipt and parent-retained compiler closure. Before execution,
the entry redecodes the complete outer handoff and checks its attempt, slot,
transaction, semantic capsule, invocation digest, compiler closure,
capsule-to-module pair, final-module commitment, and embedded compiler-module
relationships.

The V3 path reuses the bounded upstream LLVM/LLD engine and its V2-framed worker
exchange. This retained internal mechanism does not expose a standalone Worker
V2 admission or publication authority. Raw output enters only through
`inspect_protected_production_v1_worker_v3_raw_hsaco_v1`, which derives the
target, code-object version, symbol closure, descriptor state, and launch
contract from the exact retained V3 evidence and independent HSACO inspection.
Callers cannot supply or weaken those facts.

`finalize_inspected_protected_worker_v3_hsaco_v1` consumes that V3 inspection.
It patches only a valid zero-digest canonical `.fe2o3.kd.v1` table, reparses the
result, and checks target, kernel closure, ABI, resource, and launch observations
against the retained inspection. Descriptor-free output fails closed as
`MissingAuthenticatedProtectedDescriptorSourceEvidenceV3`; executable metadata
is never used to invent Rust ABI, layout, effect, or source claims.

`prepare_protected_worker_v3_compact_finalizer_replay_v2` moves the finalized
owner into a bounded restart transcript without duplicating the large module,
provider, request, response, raw-HSACO, or finalized-HSACO payloads. Publication
preparation, persistence, and recovery rederive the V3 binding, replay both
worker exchanges, re-inspect and re-finalize the raw HSACO, and require exact
source, plan, finalization, and byte identities before durable publication.

These checks establish structural consistency and custody for the sole Worker
V3 route. They do not authenticate compiler origin, prove compiler refinement or
kernel semantics, prove what implementation produced a measured worker binary,
or independently grant HSA load or launch authority. The scalar-GEMM descriptor
validator is retained because Worker V3 authority consumes it; no scalar-GEMM
worker publication route is retained.
