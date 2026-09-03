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
input kind, expected final symbol, bitcode, or standalone worker-evidence claim.

`stage_compiler_ffi_envelope_v1` consumes and privately retains the complete envelope. Its public
surface exposes only a domain-separated staged identity and target/version/count/blocker inspection.
It cannot reveal contract or generic linker closures and cannot create or bind standalone worker evidence.
Because the neutral envelope has public constructors, staging does not authenticate rustc origin.
The live managed Worker V3 transaction does not upgrade this caller-constructible staging surface.
Instead, rustc publishes a semantic handoff containing the exact textual LLVM module, the complete
envelope, and a compiler-derived symbol-role manifest. `cargo-fe2o3` consumes that handoff exactly
once for the matching producer and build attempt. Its nested module payload retains the versioned
`CompilerModuleHandoffV2` codec; that label describes serialized bytes, not a V2 authority route.

The older `G4FfiClaimEnvelopeV1` path below remains caller-constructible assertion-only plan
scaffolding. It is not the real rustc observation and cannot upgrade caller-authored generic worker evidence.

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
managed module; the separate live Worker V3 transaction carries one compiler-derived textual
module with explicit kernel, helper, export, and import roles.

Successful staging returns only `StagedFfiLinkPlanV1`. Its public surface exposes the complete staged
identity and a non-authoritative count/blocker summary. Raw plan, input, provider, symbol-evidence,
canonical-byte, and reduced-closure fields are inaccessible. It cannot construct a worker request or consume a worker output.

The standalone Worker V1 request, response, output, constructor, and execution APIs are retired.
Worker Protocol V2 remains an internal supervised LLVM/LLD wire format, and its decoder rejects
retired V1 bytes. It can only be constructed through the protected Worker V3 transaction; successful
protocol validation grants no publication, loading, or launch authority.

## Worker V3 HSACO admission and publication

Worker Protocol V2 remains the supervised LLVM/LLD engine wire format used by the
current compiler handoff. It is not a public artifact route. The standalone raw
legacy inspection, canonical-finalization, and publication APIs have been
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
`inspect_protected_worker_v3_hsaco_v1`, which derives the
target, code-object version, symbol closure, descriptor state, and launch
contract from the exact retained V3 evidence and independent HSACO inspection.
Callers cannot supply or weaken those facts.

`finalize_protected_worker_v3_hsaco_v1` consumes that V3 inspection.
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

`revalidate_protected_worker_v3_finalizer_derivation_v1` performs the same
strict reconstruction from borrowed envelope components and returns a compact,
move-only `RevalidatedProtectedWorkerV3FinalizerDerivationV1`. It retains the
exact transcript, source and binding, measured worker, request, compiler-module,
link-plan, linked-module, optimized-module, object, native-input, in-process LLD,
raw-HSACO, finalization, and finalized-HSACO identities. Recovered host admission
and the protected verifier call this shared validator independently; host
lineage includes the first identity, promotion compares both, and an accepted
decision retains the verifier-owned result. The validator uses bounded transient
copies only where an external provider interface requires ownership. Legacy
compact V2 transcripts remain decodable for inspection but fail production
revalidation because they do not retain exact derivation bodies.

These checks establish structural consistency and custody for the sole Worker
V3 route. They do not authenticate compiler origin, prove compiler refinement or
kernel semantics, prove what implementation produced a measured worker binary,
or independently grant HSA load or launch authority. The scalar-GEMM descriptor
validator is retained because Worker V3 authority consumes it; no scalar-GEMM
worker publication route is retained.

## Production semantic debug attachment

The frozen Worker V3 semantic-to-LLVM association may carry a separately versioned, bounded,
authority-free debug extension. A legacy bare V3 association remains valid and is reported as an
explicit unavailable attachment; malformed wrappers and direct carrier substitution fail closed.
An available attachment retains exact Source Map V2, semantic MIR, canonical KIR V7 projection,
typed schedule-unavailable status, and a partial Source-to-MIR-to-KIR map. Final admission joins all
13 frozen association axes, decodes the exact outer canonical KIR V8, requires the carried V7
projection to decode to the identical `Module`, and checks every map edge against the exact V4
statement correspondence, semantic MIR call-site span, and Source Map operation site.

Exact production admission also constructs the additive Semantic Debug Transformation Map V2
sidecar documented in `../../docs/semantic-debug-transformation-map-v2.md`. It retains one-to-one,
one-to-many, many-to-one, many-to-many, and eliminated endpoint relations independently of an
optimization label. The current V4 correspondence authenticates preservation and elimination, but
does not classify multi-operation lowering spans; those remain exact one-to-many relations with
`ProducerDidNotClassify`, not fabricated duplication. Legacy artifact-only admission has no V2
projection.

The finalizer also independently replays the retained whole-module neutral-KIR-to-target-KIR-to-
pre-descriptor-LLVM evidence. For exact V8 inputs, the bounded source/ISA correlation API joins the
Source Map V2 span and node, semantic MIR, neutral KIR coordinate, target KIR coordinate, Worker-
input LLVM pseudo-probe coordinate, and sparse four-byte final-HSACO pseudo-probe interval. It
supports forward queries by exact source node or span and reverse queries by exact metadata kernel
ordinal plus aligned symbol-relative PC. The record set preserves one-to-many, many-to-one,
duplicate/coalesced, eliminated, and no-source operations instead of inferring missing provenance.

This sparse correlation is descriptive evidence, not a complete source-to-machine refinement. It
does not establish optimized or final LLVM custody, instruction scheduling, complete ISA coverage,
live-PC ownership, or runtime authority. Non-anchor and unaligned PCs remain typed unavailable. A
V9 replay with the exact current V7 source-projection producer gap is also typed unavailable; the
finalizer does not infer a V9 source projection. `ExactInputsAndArtifact` means the declared byte
axes and available V4 semantic members were joined to the identical finalized artifact.

`ProductionSourceIsaCatalogV1` is the bounded durable projection for later name-free diagnoses. It
retains every admitted record in canonical order together with the correlation, finalized semantic
map, raw Source Map V2, artifact, and target-structural identities. Its canonical decoder rebuilds
only an inert claim; query access requires reconstructing the complete catalog from an independently
admitted correlation and byte-for-byte equality. The admitted catalog indexes source-node,
source-span, MIR-node, MIR-coordinate, neutral-KIR-node, neutral-KIR-coordinate, target-KIR,
semantic-operation, compiler-handoff LLVM, and sparse-ISA axes. Unknown or ambiguous sites are never
replaced with a seeded best match. V9 wire claims remain typed unavailable. The catalog remains
observation-only and grants no debugger, profiler, publication, or runtime authority. The complete
wire and query contract is documented in `../../docs/production-source-isa-catalog-v1.md`.

`ProductionKirV7StructuralBridgeV1` is the separate bounded bridge from simulator/Diagnosis KIR
V7 coordinates into this production catalog. Admission independently verifies exact canonical V7
and production V8 bytes, the canonical Source Map V2 subject and content identity, exact finalized
artifact bytes, and every catalog and target-structural identity. The current compiler projection
decodes V7 and V8 to the identical `Module`, so the bridge explicitly catalogs block-entry,
operation, and terminator coordinates as one-to-one identities. An exact catalog-handoff query is
available only for operation coordinates: a workgroup-barrier operation can therefore retain the
catalog's `NoSourceProvenance` result, while a following `Return` remains a typed structural-only
terminator. Source/MIR duplication, coalescing, and elimination remain in the Source/ISA catalog
rather than being recast as KIR-version migration.

Canonical bridge bytes decode to inert claims. Query access requires exact reconstruction against
the admitted catalog. Reordered, duplicated, substituted, over-limit, or non-identity records fail
closed. V1 accepts only an already-admitted V8 Source/ISA catalog; V9 unavailability is reported by
the upstream finalizer/catalog admission and is not re-labeled as a reachable bridge result. The
bridge proves structural coordinate identity only; it proves no semantic refinement, schedule,
complete ISA coverage, source attribution for every site, live PC, GPU observation, or debugger,
profiler, publication, load, launch, or runtime authority.

The frozen bridge wire, admission, query, and nonauthority contract is documented in
`../../docs/production-kir-v7-structural-bridge-v1.md`.

`ProductionProfilerKirArchiveV1` is the bounded restart boundary for that
producer evidence. Preparation consumes an already-finalized protected Worker
V3 owner and records the exact build attempt, outer semantic handoff, ordered
external-provider payloads, compact finalizer transcript, and finalized HSACO.
The canonical archive binds every byte with a domain-separated checksum and
identity. Its strict decoder rejects truncation, trailing data, reserved-field
changes, duplicate or reordered tagged sections, provider-ordinal changes, and
component or aggregate bound violations.

Decoding creates only an inert owner. Admission reruns the complete Worker V3
finalizer replay before deriving a fresh Source/ISA catalog, V7-to-V8 bridge,
and Characteristic projection. Compiler instrumentation, catalog, bridge, and
projection gaps remain distinct typed-unavailable results. The archive does
not authenticate the external origin of its bytes and retains no compiler,
publication, load, launch, profiler-collection, or runtime authority. See
`../../docs/production-profiler-kir-archive-v1.md` for the wire and trust
boundary.

`ProductionSourceIsaCharacteristicCollectionV1` is an additive, bounded producer-side projection
over an admitted catalog and bridge. It independently verifies the exact target-bound KIR V8 bytes
and classifies operations only by structure: global plain/guarded stores, workgroup
plain/guarded/matrix-tile loads and stores, workgroup barriers, and the exact
BF16-to-F32 M16N16K16 wave64 matrix multiply-accumulate profile. It retains every exact catalog
record for each matching coordinate, reports source ambiguity without selecting a best match, and
keeps pre-KIR eliminations as separate facts rather than attributing them to a survivor. A
target-KIR record whose backend anchor was eliminated remains attached to its characteristic with
an empty sparse-anchor list; it is not recast as a pre-KIR elimination. Stable catalog ordinals
preserve exact duplicate records and multiplicity.
Explicit per-witness, aggregate-correlation, characteristic, and elimination budgets return typed
unavailability.

This collection is the producer-to-observer conversion boundary, not an observer wire format.
`release_production_source_isa_characteristic_projection_v1` fallibly copies its exact bindings,
structural kinds and memory forms, coordinates, stable catalog ordinals, duplicate sparse anchors,
and separate pre-KIR facts into `fe2o3-source-isa-observation`. The observer binding includes the
literal KIR version and the catalog and structural-bridge canonical byte lengths. The adapter
recomputes producer-only attribution and count summaries before release.

`readmit_exact_production_source_isa_characteristic_projection_v1` accepts a decoded inert claim
only when it equals a fresh projection of independently admitted producer evidence. This equality
does not return compiler or runtime authority. No workload or kernel name, fixed operation
ordinal, debugger authority, profiler authority, complete machine coverage, schedule, or live GPU
state is inferred by either direction. Compiler-handoff LLVM coordinates and sparse final-HSACO
anchors do not prove optimized/final LLVM custody, decoded ISA opcode semantics, instruction
scheduling, execution, or performance.

The ignored real-Worker acceptance
`production_source_isa_catalog_admits_real_worker_kernel_family_matrix` rebuilds this catalog for
scalar-elementwise, uniform workgroup-collective, and tiled-coordinate canonical semantic-MIR
fixtures on both gfx942 and gfx950. These are not ordinary attributed-Rust source inputs. The
fixtures enter through canonical semantic MIR, lossless MIR-to-KIR evidence,
target binding, semantic-anchor LLVM lowering, the pinned LLVM Worker, and final HSACO admission;
the test never constructs catalog records directly. For every family it exercises exact queries in
both directions across source, MIR, neutral KIR, target KIR, compiler-handoff LLVM, and sparse ISA,
re-admits the canonical catalog, rejects cross-family and cross-target catalog substitution, and
requires observed coalesced and eliminated mappings across the matrix. The schema and producer
declare `Duplicated` representable, and independent producer/parser tests preserve duplicate and
duplicated-and-coalesced cardinality, but no current ordinary-source or canonical semantic-MIR
real-Worker fixture has produced an observed duplicated round trip. Existing hostile artifact and
semantic-map cases in the same suite retain their typed fail-closed checks. This is a kernel-family
acceptance matrix, not evidence of complete ISA coverage, optimized/final LLVM custody, a
production schedule, live-PC ownership, or debugger/profiler/runtime authority.

The separate protected `cargo-fe2o3` characteristic V2 matrix uses unmodified ordinary attributed
Rust for elementwise fill, workgroup reduction, and tiled BF16 GEMM, then consumes the same
producer projection through the sealed build observer and Broker V3 envelope. That adapter is
implemented; its six `gfx942`/`gfx950` cells remain unrun until a qualified authority service
provides the exact measured environment bindings.
