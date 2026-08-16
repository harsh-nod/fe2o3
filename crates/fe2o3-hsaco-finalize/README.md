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

A future compiler integration is responsible for creating the canonical table, embedding exactly
one zero-digest `.fe2o3.kd.v1` section after kernel metadata is known, and invoking this post-link
step before packaging. That responsibility intentionally remains outside `rustc-codegen-fe2o3`,
`cargo-fe2o3`, and this first finalization slice.

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
role does not imply LLVM bitcode, and current rustc output does not provide the required exact module:
the backend still emits per-kernel textual IR and omits non-kernel exports.

Successful staging returns only `StagedFfiLinkPlanV1`. Its public surface exposes the complete staged
identity and a non-authoritative count/blocker summary. Raw plan, input, provider, symbol-evidence,
canonical-byte, and reduced-closure fields are inaccessible. It cannot call
`construct_worker_request_v1` or consume a Worker V1 output.

Every `WorkerRequestV1`, `WorkerResponseV1`, and `WorkerOutputV1` is permanently classified as
`WorkerEvidenceClassV1::GenericLink`. Worker V1 has no field for the complete staged FFI identity, so
generic evidence can never satisfy an FFI-bound evidence API. A caller can independently construct a
generic request with similar inputs and symbol strings, but that request and its output carry zero
FFI provenance. Its API and wire bytes are unchanged, and no V1-to-V2 conversion exists.

## Worker V2 raw-HSACO publication

Worker Protocol V2 is a separate framing domain connected to the managed Cargo build flow. After
consuming the compiler handoff, `execute_reproducible_first_build_worker_v2` derives exact import,
export, kernel-entry, kernel-descriptor, helper, and unresolved-import roles from the retained
manifest rather than accepting an operator-supplied final-symbol list. It binds the pinned
executable, worker and LLVM build identities, target, code-object version, structured options,
complete envelope identity, compiler module, every external provider, final symbol closure, and
output bound. A GenericLink candidate establishes the first-build output identity; success requires
the V2 execution to reproduce those bytes exactly. Both executions use the supervised direct
LLVM/LLD worker and no COMGR path.

`inspect_worker_v2_raw_hsaco_v1` then consumes the sealed reproducibility evidence and independently
checks the exact raw HSACO against its retained lineage, target, code-object version, symbol-role
manifest, defined-symbol closure, descriptors, and `gfx942` launch metadata. It accepts no caller
replacement for those policies. This admission is deliberately distinct from canonical
`.fe2o3.kd.v1` descriptor-table finalization, which does not run on the Worker V2 publication path.

`finalize_inspected_worker_v2_hsaco_v1` is an opt-in bridge from that admitted raw evidence to the
existing canonical finalizer. When the raw HSACO contains one valid zero-digest `.fe2o3.kd.v1`, it
patches only the digest, independently verifies the result, cross-checks target, code-object
version, kernel closure, and launch metadata against the retained raw policy, and returns an opaque
`PreparedFinalizedWorkerV2HsacoV1`. The value privately retains both raw and finalized lineage.
This is structural integrity evidence only: the embedded descriptor's compiler, source, ABI,
layout, effect, and build-evidence claims remain unauthenticated.

Current Worker V2 output may omit `.fe2o3.kd.v1`. In that case the bridge returns an owning
`MissingAuthenticatedDescriptorSourceEvidenceV1` blocker. It records the admitted lineage, target,
code-object version, policy, and observed kernels but does not expose or fabricate a descriptor
table. In particular, Rust ABI, layout, effect, and source claims are never inferred from
executable metadata. Neither the structural result nor the blocker grants publication, loading, or
launch authority, and this bridge is not yet connected to `cargo-fe2o3` publication.

`prepare_worker_v2_hsaco_publication_v1` consumes the admitted evidence and returns the typed
`PreparedWorkerV2HsacoPublicationV1` bridge. Its durable plan and upstream evidence identity remain
private. `publish_prepared_worker_v2_hsaco_v1` uses that bridge with the matching producer and live
attempt registry to publish the exact admitted bytes and an attempt-scoped durable provenance
receipt; `cargo-fe2o3` then completes the same build attempt. The prepared value supports exact
in-process reconciliation, but enough intent is not yet persisted to recover in a new process after
the compiler handoff has already been consumed.

Neither the handoff, reproducibility evidence, raw-HSACO admission, typed bridge, nor publication
receipt authenticates the compiler or its origin, authenticates or binds Verus verification, grants
HSA loading authority, or grants kernel-launch authority. On `mi300x`, the ignored
`worker_v2_real_source_publishes_inspected_gfx942_hsaco` and
`worker_v2_real_source_links_an_external_bitcode_provider` tests pass with an unoptimized Debug
worker for `gfx942:xnack-`, through durable publication. Those tests do not load or launch the HSACO,
and no optimized Release-worker result is claimed.

### Tiled GEMM V1 structural artifact policy

`inspect_tiled_gemm_v1_structural_worker_v2_hsaco_v1` is a separate sealed
specialization of Worker V2 raw admission. It preserves the existing generic
WG256 policy and selects an exact WG64 contract only for the declared
direct-global `tiled_gemm_v1` profile. Admission requires COV6,
`gfx942:xnack-`, wave64, required workgroup `[64, 1, 1]`, maximum flat
workgroup 64, zero LDS, exact entry and descriptor symbols, and one embedded
unfinalized canonical descriptor table.

Metadata must contain eight explicit fields for four slices: each global
pointer is followed by its `u64` length at offsets `0, 8, ..., 56`. A and B use
`u16` storage; C and D use `f32`. The explicit span is 64 bytes and the COV6
implicit suffix starts at offset 64 with size 256, producing a 320-byte kernarg
segment. Descriptor admission additionally requires exact subgroup, matrix,
AMD-wave, and AMD-MFMA declarations plus the direct-global zero-LDS logical
argument contract.

This policy does not inspect `.text` or bind it to a trusted lowering. `u16`
storage does not prove BF16 interpretation, and an AMD-MFMA capability
declaration does not prove that the body contains or correctly uses an MFMA
instruction. Synthetic tests intentionally use arbitrary `.text` bytes to
make that boundary executable.

`finalize_tiled_gemm_v1_structural_worker_v2_hsaco_v1` uses the existing
in-process LLVM/LLD Worker V2 lineage and canonical finalizer. It independently
verifies the finalized structural envelope, reruns exact metadata checks, and
requires the finalized descriptor admission to equal the raw admission. It
adds no COMGR or shell linker path. Canonical finalization does not add
kernel-body or ISA-semantic validation.

The older 288-byte frontend probe remains a separate evidence profile: eight
BF16 plus four F32 by-value fragments and 32 explicit bytes. Substitution
between that probe and this structural 320-byte profile fails closed. Neither
typed result authenticates compiler or code origin, validates the kernel body,
proves BF16/MFMA semantics or Verus verification, or grants publication, load,
or launch authority.

### Row-softmax V1 structural artifact policy

`inspect_row_softmax_v1_structural_worker_v2_hsaco_v1` is a separate sealed
Worker V2 specialization for `row_softmax_v1`. It requires COV6,
`gfx942:xnack-`, wave64, required workgroup `[64, 1, 1]`, maximum flat
workgroup 64, absent max-num-workgroups metadata, zero LDS and private segment,
exact entry and descriptor symbols, and one unfinalized canonical descriptor
table.
The bound executable entry must be a real function symbol in an executable
mapping, but its instruction bytes are not interpreted.

The measured upstream LLVM 22.1.8 metadata must omit the kernel-kind field while
decoding to Normal, emit `uses_dynamic_stack=false`, and omit uniform-workgroup,
cluster, workgroup-processor, gfx revision, enqueue, workgroup-size-hint, and
vector-type-hint fields. Source language must be exactly OpenCL C 2.0. SGPR,
VGPR, AGPR, SGPR-spill, and VGPR-spill fields must be present with the measured
values `42`, `88`, `44`, `44`, and `28`.

The argument array must be present and contain exactly four explicit fields
followed by nineteen COV6 hidden fields. The hidden sequence is block counts,
group sizes, remainders, global offsets, grid dimensions, hostcall buffer,
multigrid synchronization, heap V1, default queue, completion action, and queue
pointer at the exact LLVM-emitted offsets and sizes. Missing, reordered,
duplicated, qualified, or additional arguments fail closed.

Metadata and the canonical descriptor must agree on two F32 slice pairs:
`input: &[f32]` and `output: DisjointSlice<f32>`, with pointer/length fields at
offsets `0, 8, 16, 24`. The explicit span is 32 bytes and the COV6 implicit
suffix starts there with size 256, for 288 bytes total. Capability and build
evidence remain unauthenticated declarations.

The fixed row length of 64 is not present as a runtime value in descriptor or
AMDHSA metadata, so artifact admission cannot validate either slice length or
an actual host launch. The value is only an intended host-profile requirement,
not a property exposed by admitted artifact evidence. Arbitrary `.text`
remains structurally admissible. No
result proves functional softmax, an `exp` implementation, reduction order,
NaN/infinity behavior, numerical error bounds, memory safety, non-aliasing,
race freedom, or Verus verification. It authenticates neither source nor
compiler origin and grants no publication, HSA load, or launch authority.

`finalize_row_softmax_v1_structural_worker_v2_hsaco_v1` uses the same existing
upstream LLVM/LLD Worker V2 lineage and canonical descriptor finalizer. It adds
no COMGR path and independently repeats the exact structural checks after
digest finalization.

The separate row release gate binds this profile to a manifest-only commit and
requires two fresh builds against one caller-supplied reviewed manifest digest.
That is host-specific compiler/code-object integrity evidence. It is not origin
authentication, GPU execution, source-to-machine refinement, or runtime
authority.

Both inspected and finalized profile values retain their generic Worker V2
lineage privately. They cannot be converted into generic prepared-finalization
or publication values, and profile finalization consumes inspected evidence so
the same admission cannot be replayed.
