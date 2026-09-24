# Production compiler convergence V1

This document defines the implementation shape for
[#175](https://github.com/harsh-nod/fe2o3/issues/175). It narrows the compiler
work under [#134](https://github.com/harsh-nod/fe2o3/issues/134) to one
production transaction. The former scalar, GEMM, attention, collective, and MoE compiler oracles have been deleted. They are not additional production architectures. Differential evidence may remain as
inert fixtures or offline tools, never as another route in production crates.

## Nominal P4 continuation

The nominal descriptor continuation retains the original source, authenticated
bindings and actual P4 optimized output O. Its consuming native-text continuation
uses the existing target-bound emitter and LLVM22 layout binder, embeds one V3
descriptor section, and retains that same owner. It does not advance O to a later
optimization stage or reconstruct an independent executable graph.

Replay checks source-to-O history, nominal source ABI, physical argument packing,
target requirements, all five stored symbol vectors, a source-derived catalog,
and the complete emitted text including the descriptor suffix. Input storage
transfers unchanged; the caller reserves only the returned additional receipt.
This is an inert compiler boundary, not a Worker V3 handoff or launch authority.
Nonempty source catalogs and deferred matrix/LDS/atomic/synchronization
requirements still reject rather than discard their obligations.

The source harness covers eight ordinary-MIR guarded-read cases and two
retained-MIR private-helper cases across gfx942/gfx950. The latter assert genuine
helper/call erasure; they do not claim ordinary-MIR helper retention. Exact
retained-MIR flags are checked by the request validator and full-argv digest,
not by portable metadata alone. Tests check allocation-capacity receipts,
unchanged descriptor/O backing, text substitution, resource refusals, and
consuming-error cleanup. Diagnostic work/peak observations include negative
checks and are not performance measurements. These source tests must be run
explicitly with their ignored parent tests; declarations alone are not coverage.

The following consuming source-proof continuation retains that native owner
unchanged and borrows its authenticated ranked roster into the existing
Direct/Erased proof assembler. It shares collector/preflight/context and exact
source/N/E/catalog/launch binding with the existing output handoff. The original-N
packet is separately checked byte-for-byte against the retained source and
catalog, and its subject must match each of the three typed proof rosters.
Framing, graph hashes, or an erased/optimized graph alone cannot satisfy this
join. The caller reserves only the additional owned packet receipt; consuming
failure drops the input and leaves its original reservation for caller retirement.

The ten source cases also exercise the consuming unsigned-proof refusal. Fresh
control owners cover work/storage refusal and corrupted native-text refusal.
Component tests cover exact packet bytes, each roster subject, route/graph
substitution, and resource limits. These are not successful signed-source runs:
genuine reference-bound Direct/Erased positives, including post-proof failure
cleanup, still require the protected proof runtime and remain unvalidated.

Protected worker execution, proof/artifact custody, generated safe launch and
target-matched GPU qualification remain separate unfinished integration work.
This boundary does not complete #272 milestones or qualify the tutorial corpus.

The finalizer now has a version-explicit nominal V3 continuation using the same
strict Worker V3 first-build transaction and shared ELF/physical checks. It
retains exact descriptor bytes, binds them to the retained ABI receipt, checks
the export manifest, rejects same-width nominal substitutions and mixed schema
sections, and patches only the canonical digest field. Raw reconstruction
rechecks the artifact independently. Fixture-worker tests cover descriptor-derived
launch and original transaction retention; their synthetic ELF/receipts do not
establish production compiler or protected proof provenance.

Typed nominal publication now consumes that finalizer owner, retaining exact
descriptor bytes through the existing compact replay and durable transaction.
Restart reconstruction selects the strict descriptor codec from the exact ABI
receipt and repeats inspection/finalization before publication. A closed internal
schema distinction shares the existing publication authority bridge; it does not
flatten nominal V3 into V1. Legacy identities retain their domain and encoding,
while nominal finalization uses a distinct domain over the same custody axes.
Typed V1 and nominal recovery APIs reject each other's artifacts. Published
nominal output can transfer its exact current-publication lease and replay parts
without acquiring compiler, proof, load or launch authority.

The nominal publication owner can now enter the same receipt-bearing V2 load
envelope, retaining its exact compiler subject, carriage, replay and publication
lease through durable restart. Version-explicit host roster readmission repeats
independent finalizer replay, checks the exact normalized V3 ABI receipt and
exports, and resolves physical kernels against the complete canonical roster.
The move-only owner retains nominal types without a V1 projection and rechecks
currentness before returning a successful revalidation. Nominal host identities
use a separate domain; the legacy preimage remains unchanged. Synthetic fixtures
cover retirement/restart, foreign compiler occurrences, retained-directory
recovery, and two-kernel rosters with opposite descriptor/physical orders.

This readmission is inert: exact descriptor-receipt association is not compiler
source semantics or verification authority, and it cannot load or launch.
The normal CLI, generated verified host launch, and nominal P4 source-proof owner
are not yet joined to this continuation. Native-aware production lineage
construction and independent replay remain required. Descriptor traversal
quotas and existing bounded artifact/replay allocations are separate; publication
does not produce a complete resource receipt or claim whole-process metering.
The synthetic receipt/artifact tests do not qualify any tutorial kernel, and
M0-M7 remain open.

## Durable source replay

Native source replay has reconstructive typed ranked recipe transport through
`encode_production_ranked_recipe_v1` and `decode_production_ranked_recipe_v1`.
The versioned, little-endian schema preserves all 41 operation forms, 12
terminators, eight expression forms, numerical tolerance bits and ordered proof
claims. It enforces structural bounds before construction and rejects any
normalization that changes the encoded recipe. Diagnostic ranked text and graph
hashes are not parsed as substitute recipes.

`encode_production_ranked_source_rows_v1` and its decoder transport the lowerer's
ordered access and generated-effect rows in a separate versioned frame. This
preserves absent versus present output-extent proposals, all three ranked value
forms through the recipe's existing grammar, explicit effect origins and exact
recipe identities. Rows are not sorted or repaired. Structural decoding does
not establish source correspondence or the truth of an extent proposal; those
still require replay and canonical-owner rederivation.

The Direct and UnitLocal recipe-source verifier entrypoints first validate the
source packet, then resolve claims against its complete ordered signed effect
roster and checked aggregate commitments. They share the existing typed replay
continuations, which independently re-import signatures, recompile, rebuild V5
and aggregate subjects, and replay source correspondence. UnitLocal recovery
preserves original N and independently admitted E; it does not rerun the erasure
producer. Embedded test keys establish consistency, not protected origin.
`encode_native_compiler_source_packet_v1` now transports the five original source
frames, detached launch inputs, all nine staging digests, full individual
receipt wires and keys, recipes, source rows and diagnostic text in one inert
byte buffer. It preserves independent launch/staging/ranked counts and order,
both launch ranks, exact logical names, full bindings, workgroup options and
raw grid values. UnitLocal additionally carries actual E bytes, which receive
fresh V12 admission rather than a serialized verified flag. Neither grid fields
nor embedded verifying keys authenticate their origin.

Native backend packet preparation serializes this complete packet, replays it
through the existing recipe validators, and retains its bytes alongside the
source-proof owner and original N envelope. This is not yet a serialized capsule
or protected publication continuation. The packet has a dedicated versioned
schema and an explicit aggregate 4 MiB limit, including duplicate nested bytes.
This source-packet limit is independent of the existing refined-forwarding
output limit (64 MiB, including its complete B/C/S/O/I/J/K/P/H/L/R/F history).
The legacy ProofBinding receipt's 4 MiB cap cannot carry every admitted pair.
The paired `F2NRF1` carrier preserves both constituent formats and limits, with
an additional fixed 80 bytes of framing and terminal content identity. This is
not a legacy ProofBinding receipt. Distinct V4 capsule/handoff content codecs now
retain this complete carrier, but typed production dispatch is not integrated.
It may neither narrow advanced output to 4 MiB nor select the older
single-transition route.

The V4 capsule contains the unchanged V3 base plus mandatory F2NRF1, all within
the unchanged complete 160 MiB capsule ceiling. Its distinct frame/hash uses the
same private bounded-pair engine as F2NRF1. Shared decoding retains base receipts
and carrier in one allocation and rejects a base MIR too large to fit in the
complete source packet before deriving the V3 MIR receipt hash. The aggregate
hashes have already visited those bytes. This is not a semantic equality check:
unrelated well-framed members remain inert content.

The V4 handoff shares V3's bounded wire parser and identity-neutral pair encoder,
with explicit V4 outer/pair discriminators and hash domains. It binds complete
V4 capsule and V2 identities, validates target/final-commitment agreement, and
retains every nested payload in the same backing, including at nonzero enclosing
offsets. Direct seal APIs allow all three frames to be written in one final
allocation. Refused debits and callback/destructor panics precede mutation.
Tests at representative and maximum raw payload sizes verify no framing
allocations and unchanged decode allocation traces for a fixed metadata shape;
old V3 bytes, limits and rejection behavior remain.

V4 content decoding remains an unmetered codec like V3. The compiler-FFI layer
now defines a conservative versioned logical work-prepayment schedule, audited
against the pinned toolchain and parser limits, alongside shared metadata-storage
allowances. This is not instruction-exact accounting, RSS measurement or engine
replay. Production prepays those amounts and the full actual backing capacity on
the same ledger. Wire maxima do not imply that every replay fits the unchanged
256 MiB policy.

A private nondefault continuation now consumes the actual live final-F wire
owner into a V4 handoff. It requires retained protected invocation custody; an
extraction-only transaction cannot supply a substitute invocation. It derives
all fifteen base receipts from retained compiler fields and checked carrier
members. Original-N proof receipts remain unchanged. Target binding associates
N/catalog with actual B through the checked Direct or N-to-E erasure route;
the compact native lowering association separately binds actual F/catalog,
profile, carrier, descriptor, pre-descriptor LLVM, final LLVM and complete V2.
It is constructed after native text/descriptor replay, not by substituting a
legacy lowering receipt. Raw LLVM, V2 and domain-separated receipt identities
are separate coordinates, including their exact lengths.

The continuation reuses the carrier vector, prepays/reconciles its capacity
growth, moves the carrier inside the outer buffer, and writes base/V2/framing
before shared decoding. It retains the actual live compiler owner beside that
immutable transport. Revalidation rebuilds expected base receipts from the live
owner and compares every canonical byte. Public construction or decoding of the
compact association still grants no authority.

`recover_compiler_native_semantic_handoff_v4` now consumes a prepaid V4 transport
and extends the existing paired recovery with independent native capsule joins.
It compares original proof preimages, target N/catalog-to-B coordinates,
semantic-order workgroups, semantic/rustc/LLVM22 layout, actual F native lowering
and all thirteen semantic-to-LLVM identities. Exact embedded/outer V2 equality
is checked before borrowing the already decoded outer module; no second complete
V2 allocation is made. The immutable outer decoder's exact final-commitment
check remains mandatory. There is no legacy target/lowering admission fallback.

Those joins run while the source, history and independently replayed text
relation are still borrowed. Only afterward is actual F moved from that history
into the returned owner alongside the unchanged transport and signed source.
The caller must keep full enclosing backing capacity (including spare capacity)
and decoded metadata paid, then reserve the returned additional owner storage.
Exact/one-short tests check storage cleanup without resetting work or prior
failures. Signed two-root fixtures use actual profile layout digests, both source
routes, both AMD profiles, independent root ordering, and resealed substitutions.
They establish content consistency with public test keys, not protected origin.

The artifact transaction now has a distinct, metered V4 schema over the shared
transaction/currentness engine. It preserves exact attempts, pinned file custody,
cooperative locks and one-shot consumption, with no legacy decoder or sidecar
fallback. Recovery strictly decodes V4 before returning a receipt; lease minting
streams the occurrence hash without allocating a complete payload. These are
inert content/custody records, not compiler execution or artifact authority.

`recover_compiler_native_semantic_handoff_token_v4` runs the same native semantic
checker while retaining the transaction token's lock and unchanged backing.
Only afterward may the caller consume the exact occurrence. Refusal, resource
failure or unwind before the ready-to-consumed rename leaves the occurrence
unconsumed; failures after rename do not permit replay. The generic ownership
adapter is not an authority gate: the verifier adapter supplies the concrete
privately constructed source/F owner. Its original backing snapshot is checked
again before consumption.

V4 payload capacity, decoded metadata, hash scratch and owner headers share the
caller's work/storage ledger and unchanged 256 MiB cap. Returned storage is
admitted but unreserved: retain the original token reservation, then reserve
the additional mapped-owner amount. Consumed-owner storage is prepaid before
commit. Scoped cleanup preserves accepted work, peaks and first denials; a
substituted ledger is never released as if it were the original. Filesystem
registry and directory metadata retain the existing separate protocol bounds;
this is not whole-filesystem work accounting or an RSS bound.

[Native execution subject V2](compiler-execution-subject-v2.md) now binds the
exact V4 occurrence, complete carrier-bearing capsule, cached invocation and
compiler closure under a distinct fixed 690-byte schema. It uses the existing
ledger, retains no payload backing, and grants no authority. Publication,
recovery and raw/concrete-verifier consumption reconstruct the same content.
The V1 wire/API remains frozen through shared codec mechanics and independent
golden tests; V2 has no legacy fallback.

[Native receipt transport V2](compiler-execution-receipt-transport-v2.md) now
binds opaque receipt bytes to that complete subject through shared sidecar
custody. Ready publication/recovery reconstructs the subject from V4; locked
recovery uses the exact raw token; consumed restart compares all stored subject
bytes after payload deletion. One move-only buffer retains the body, with actual
capacity charged and all native postcommit readback work prepaid. This is inert
transport, not protected issuance or an activated production consumer.

The [native Worker adapter](../crates/fe2o3-hsaco-finalize/README.md#native-sourcef-worker-integration)
now borrows the locked recovered token for preflight and consumes that exact
occurrence through the existing candidate/replay engine. Its separate identity
domains bind the whole receipt, carrier, actual F, descriptor and compiler
closure without changing the frozen Worker V2 wire or legacy V3 identities.
Returned inert evidence retains the consumed source/F owner and complete Worker
transcripts. Shared Rust staging/replay is conservatively prepaid on the original
ledger; process/LLVM accounting remains separate. Public-key fixtures and a
synthetic Worker test this connection, not protected or GPU execution.

The native finalization adapter retains that owner through the shared raw-HSACO
inspection and canonical descriptor finalizers. Native compact replay uses the
existing metadata tail and request/response reconstruction engine, while binding
the complete V4 outer and original occurrence under separate domains. Recovery
requires independently recovered source/F, producer-specific transaction
rederivation, exact Worker replay and repeated structural finalization. A
recovered transcript is explicitly distinguished from a freshly consumed
publication. Neither representation provides current protected execution or
semantic-to-machine authority.

The [2026-09-24 integration checkpoint](evidence/native-worker-finalization-replay-20260924.md)
records the native replay matrix, legacy regressions, source-path checks and
isolated service test, together with the remaining protected-proof boundary.

Inventory/preflight receipts remain inert until the compiler custody boundary
authenticates them. Native root/socket/pidfd service custody and service packet
codecs are separate supporting components, not a deployed native issuer. Issuer
admission, durable signing/ACK/currentness, Cargo intake, authorized finalizer
publication, runtime and generated-host consumers are still not connected end to
end. The native Worker continuation has no executable-artifact publication
authority conversion; default production remains unchanged. No tutorial kernel
gains production or hardware coverage from this library integration alone.
The signed ordinary-Rust producer fixture exercises base construction with an
explicitly synthetic invocation, but requires the protected proof runtime.
CPU content/resource tests do not replace that fixture, a positive protected
producer continuation, machine refinement, or protected production execution.

`recover_compiler_refined_forwarding_output_v1` independently recovers the signed
Direct or UnitLocal source from its complete packet, freshly admits every graph
in the existing final-output history, and invokes the complete source/history/
formal/descriptor/native-text checker. Only after that succeeds does it consume
the history and move the actual F graph with its original admission receipt into
an immutable owner alongside the recovered source proof and checked content
identities. The temporary history and packet adapters are not retained. No
replacement graph is decoded or synthesized during that move.

The nondefault refined-forwarding producer encodes F2RFO1 directly into the
carrier's final allocation, then copies the complete source packet from its
retained live owner into the other region. There is no intermediate full output
buffer. Both the live packet and the new carrier backing remain charged. The
legacy output getter still borrows only F2RFO1; a separate getter exposes the
complete carrier. Replay independently parses the carrier, compares its source
to the live owner and its fourteen output fields to freshly derived fields, and
uses `recover_compiler_refined_forwarding_carrier_v1` without format fallback.
Carrier framing, hashing and getters allocate no heap payloads; nested semantic
admission still runs through the existing complete source-to-F checker.

The producer's original paired-only recovered owner is dropped after its check;
the new V4 admission retains its recovered owner beside the outer transport.
Worker evidence and structural finalization retain that native owner; protected
artifact publication is not wired. Its output identity binds the
embedded fields, not an external Worker request: later admission must bind that
exact frame, NativeV2 and descriptor to the actual request and finalizer. Dropped
history is not available for self-contained replay, and this owner supplies no
protected origin, rustc ABI authentication, machine refinement or launch authority.

Genuine two-root tests cover both source routes and gfx942/gfx950 emitted output,
after producer and both input buffers are dropped. They retain exact N/E/source
rosters and actual F, reject cross-source histories and final output, and preserve
resource floors at exact/one-short limits. Separate nonzero-history tests confirm
the transferred allocation is actual F after induction and forwarding rewrites.
These cover both separate-input and paired recovery, and remain CPU consistency
tests using public test keys, not hardware results. The recovery ends at F and
uses descriptor V1. The separate loop-unroll route ends at U and requires actual
U and its F-to-U relation; nominal descriptor V3 also requires explicit applicable
admission. Neither may be substituted through this F/descriptor-V1 interface.

Work and live logical payload use the shared verification ledger. Resolver
callbacks can charge work but cannot replace or release the storage ledger;
ignored callback denials still fail. Parsed owner reservations remain live
through normalized capacity reconciliation, and returned receipts exclude
discarded temporary recipes and source rows. Correspondence counts and minimum
remaining wire sizes are checked before allocation; exact capacities are
prepaid and remain live throughout replay. Existing checked constructors and proof engines
retain their separate bounded scratch/work domains: this is not an RSS or
whole-process accounting claim.

The complete packet decoder borrows wire payloads and reserves its typed launch,
staging and signature metadata through replay. Returned proof receipts exclude
these discarded adapters. Tests recover Direct and UnitLocal after dropping
producer owners and packet bytes. Genuine signed two-root fixtures share one
semantic module and preserve source order `[0, 1]` independently of canonical
binding order `[1, 0]`, with distinct stores of 7 and 11. They compare complete
N/E, catalog and signed rosters, unchanged retained-storage receipts, cross-root
substitution failures, and second-root truncation cleanup. Exact/one-short work
and storage tests retain the caller's floor and failure history. These test-key
fixtures are not protected proofs. Typed native capsule
production/admission, compiler/Worker custody, generated safe launch, protected proof
execution and tutorial/GPU qualification remain outstanding. M0-M7 remain open.

The tensor leaf dependency now reuses the existing kernel-IR V8 grammar through
`encode_tensor_layout_leaf_v1` and `decode_tensor_layout_leaf_v1`. Both require
the shared verification budget, allocate no heap, and preserve the inherited
storage ledger. The complete fixed-size schema is at most 117 bytes, including
opaque and unsupported variants. Syntax round-trip preserves these variants;
independent semantic checks still reject them. A zero-fill declaration is not
authenticated zero-fill evidence. No module header or authority owner is created.

Tests cover literal bytes for BF16/FP8/FP4/mixed layouts, independent operand
swizzles, the full schema bound, malformed input, exact work quotas and historical
V7/V8 gates. Recipe transport reuses this leaf with prepaid fixed-size scratch;
neither codec supplies a protected proof or production/GPU qualification result.

## One transaction

The completed convergence target sends every kernel-containing final crate
through one rustc-owned transaction:

```text
authenticated rustc kernel closure
    -> canonical semantic MIR
    -> owner-authenticated executable MIR graph
    -> canonical target-neutral Kernel IR
    -> owner-authenticated executable Kernel/GPU graph
    -> typed AMDGPU legalization
    -> canonical typed LLVM handoff
    -> pinned upstream LLVM and in-process LLD worker
    -> independently inspected AMDHSA artifact
    -> generated typed host interface
```

`cargo fe2o3 build` and `cargo fe2o3 run` realize that transaction with one
fixed orchestration plan. Cargo first performs a device `build` for the fixed
AMDGPU target under the protected compiler closure. Only after the exact
generated-artifact generation commits does a fresh Cargo process build or run
the same package/feature/profile selection for the pinned host target using
ordinary rustc. Run payload arguments are forwarded only to that host process.
The caller cannot pass `--target`, and the host phase receives no fe2o3 backend,
wrapper, broker, device, build-manifest, qualification, or simulation controls.
This is phase separation inside one production build, not two compiler routes.

Compiler provenance is one cross-cutting input to this transaction, not a
second compiler route. The canonical `CompilerClosureV2` commits to six
role-specific SHA-256 pins:

1. Cargo executable;
2. static Cargo binding trampoline;
3. full `cargo-fe2o3` binding wrapper;
4. rustc executable;
5. complete rustc runtime tree; and
6. selected rustc codegen backend.

The closure also commits to the canonical Cargo-to-trampoline-to-wrapper
transition protocol, currently
`CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1`, and derives one aggregate
identity from the domain, protocol version, and ordered pins. The aggregate is
validated, not an independently trusted seventh pin.

`RustcInvocationDescriptorV3` is exactly one complete
`RustcInvocationDescriptorV2` process description, including cwd, final argv,
and complete sorted child environment, plus the complete canonical
`CompilerClosureV2` preimage. Construction cross-checks the duplicated rustc
and backend digests.

### Compiler provenance wiring

| Boundary | Current state | Remaining production wiring |
|---|---|---|
| Protected release and Cargo broker | The release contract validates `CompilerClosureV2`; the broker transfers a sealed raw closure capability to the binding wrapper. | Preserve the admitted closure through runtime authorization. |
| Exact rustc invocation | The wrapper constructs and seals V3 for production, installs its immutable image at fd 199, retains parent custody, and the backend revalidates argv, cwd, environment, target, role pins, and closure before V3 publication. Qualification V2 captures receive no fd 199 capability and are not retained as production custody. | Extend archived end-to-end evidence across the final application process boundary. |
| Compiler module handoff | Production has one mandatory protected-custody path and one V3 publication/consumption transaction. Before monomorphization, a device transaction must retain one admission containing both the authenticated gfx942 target and exact managed build attempt; preflight roots and post-monomorphization device work must agree exactly. The attempt and protected rustc invocation then move as one publication custody value, so no optional or late direct-publication branch remains. The ordinary publication branch and runtime schema selector are deleted. | Keep V1/V2 consumers confined to explicit qualification code until their oracles retire. |
| Worker publication restart | `ManagedProductionBuild` has only `Fresh`, `Recovered`, and `Ready` states. It performs strict V3 preflight, one-shot consumption, direct LLVM/LLD execution, independent inspection, durable publication, and load-readiness recovery. Recovered current-publication custody now reaches the private joined KFD invocation after Worker V3 authentication. | Replace synthetic verification and remove external HSACO-path injection from the inherited application hardware lane. |
| Application handoff | Production admits only the canonical Worker V3 load envelope and rejects intermediate runners. Cargo pins the application, binds the envelope, artifact-directory, and ACK descriptors into a fresh occurrence, validates the challenge-bound ACK, and retains current-publication custody through exit. Before ACK it also creates fd 195 inside that exact child, transfers the service endpoint and pidfd through the fixed supervisor, exposes only the connected `SOCK_SEQPACKET` through seccomp, and retains issuer readiness; policy fd 202 is never exposed. `fe2o3-host` exports a one-use move-only auditor that consumes fd 195 and verifies the issuer signature, retained external commit receipt, and fresh client-bound external recovery receipt for the exact Worker record without granting authority. The compiler's V4 proof association losslessly carries all five stage identities plus its exact signed aggregate MIR-to-live-PLIRON receipt inside the frozen V3 capsule envelope. Worker V3 independently reimports and cross-checks that receipt and retains it beside signed compiler-currentness evidence throughout authentication and publication. Singleton and multi-root roster decisions also retain independently decoded target-lineage owners that rederive semantic layout, check every target/data-layout/semantic-to-LLVM receipt coordinate, replay target KIR to LLVM, cross-bind final LLVM to handoff and finalizer state, and match COV6 plus exact per-root workgroups to the admitted descriptor roster and physical symbols. The generated KFD transition joins authenticated verifier evidence, generated host-memory arguments, current publication, launch geometry, and one checked device into the private production invocation authority. The HSA load/dispatch lifecycle remains only behind the explicitly named deprecated qualification feature and cannot grant production authority. Worker V2 application routes and production raw-HIP authority surfaces remain deleted. | Supply the reviewed crate-owned verifier that joins retained proof and target owners with protected key custody and independently administered monotonic-anchor deployment; add semantic and authenticated LLVM/final-machine refinement plus dynamic launch evidence; route an ordinary generated application through inherited KFD without external artifact injection; retire the legacy HSA qualification surface after its differential cases have replacement coverage. |
| Qualification isolation | Workload oracle features and executable oracle paths are deleted. `FE2O3_QUALIFICATION_ORACLE_V1` remains only as a rejected sentinel. Cargo always completes the same Worker V3 transaction under every feature set. | Keep historical V2 names confined to frozen wire compatibility used by Worker V3. |

The Cargo package resolves its build-configuration API directly
to `PreparedProductionBuildConfig`, with no feature-dependent compatibility
type alias and no no-op qualification conversion methods. It parses only
`FE2O3_PRODUCTION_BUILD_CONFIG_V1`; the profile enum, Worker V2 schema parser,
envelope controls, source-debug controls, and workload fields are deleted. The
release path always uses the production expected-identity namespace and
ordinary compiler-capability profile; route-dependent identity and S09
selection logic no longer exists in Cargo. The binding wrapper admits every
managed kernel root through protected
rustc and requires compiler-closure custody directly; it does not compile the
qualification-oracle predicates or qualification command preparation path.
The Cargo driver also binds the fixed gfx942 target profile and production
semantic-generation identity directly. Its backend preparation context has no
production-route boolean or simulation selection.
Once a compile is selected as a production kernel root, the binding wrapper
requires the concrete production manifest before it can begin an artifact
attempt. Production preparation and completion contain no Worker V1/V2,
in-rustc oracle, simulation, row-softmax, or empty-attempt dispatch. Production
capability intake also releases the broker's one-shot invocation
authority immediately after authenticating the transfer. The release
`CompilerCapabilities` shape has no retained invocation-authority field or
child-inheritance API. The S09 broker profile and pinned-Cargo transfer image
are deleted. Shared closure, backend, Cargo-image, and artifact validation
remains implementation-neutral and runs before production receives custody.

The feature-free rustc backend likewise does not compile
`QualificationSelection`, `SelectedQualificationOracle`, or
`RustcInvocationPolicy`.
It captures a selector-free production environment preflight, enters protected
V3 rustc admission directly, and requires the production device transaction to
complete directly for every discovered kernel. The qualification-feature build
has an optional non-publishing oracle token and an invocation-policy enum for
differential testing, but no compiler-route enum or release implementation
choice.

The `cargo fe2o3 simulate` command and its Cargo-side oracle graph are deleted.
On Linux, the standalone `fe2o3-kir-sim-cli` consumes exact verified canonical
KIR V7 without source, compiler, refinement-proof, artifact, load, launch, GPU,
timing, or performance authority. `fe2o3-kir-sim-trace` independently maps
ephemeral CPU-simulator events into collector-neutral Semantic Trace V1. These
model and differential tools remain authority-free test programs; production
`build` and `run` cannot select either one.

The host-consumer and shared hostile application fixtures accept only V3
inputs. The old V2 consumer binary, input adapter, Cargo feature, and hostile
fixture protocol implementation are deleted. All application-boundary
adversarial coverage now runs against the strict V3 path in generic-core CI;
the Cargo V2 publication/restart vertical and its fixture binaries are deleted.

Version suffixes remain on serialized records, identity domains, receipts, and
external protocol types. Private production methods and states are unversioned
because there is only one implementation. A new production schema must be an
explicit migration of the same transaction, never a selectable pipeline.

The implementation uses one move-only typestate owner, conceptually
`ProductionCompilation<'tcx, Stage>`. A transition consumes the previous
stage and returns the next. The owner retains:

- the active compiler session and #140-authenticated graph handles;
- the canonical record and identity at each completed semantic boundary;
- bounded before/after transformation receipts;
- source, ABI, layout, target, proof-obligation, and diagnostic provenance;
- the exact Worker request, response, finalized bytes, and inspection owner;
- no publication, load, launch, or runtime authority.

The owner may retain several graph handles in one session, but a semantic fact
has one authoritative representation at a stage. Side data is permitted only
when the graph cannot yet represent the fact. The graph and side data are
compared at every boundary, and the side-data field has a named removal issue.

## Entry convergence

The explicit extraction driver and compatibility codegen backend must both call
one importer:

```text
import_rustc_kernel_closure_v1(tcx, collected_roots, limits)
    -> OwnerControlledSemanticMirV1
```

The importer is workload-neutral. It discovers roots from authenticated
`#[kernel]` metadata and rustc identities, traverses the complete reachable
monomorphized device closure, and records typed rustc-independent semantics.
It never branches on an export name, source substring, workload identity, or
exact MIR transcript.

The imported representation must preserve the facts needed by later lowering:

- source spans and expansion/call-site origins;
- item, instance, generic, and const-generic identities;
- layouts, FnAbi modes, calling convention, unwind behavior, and relocations;
- locals, types, places, projections, operands, constants, assertions, drops,
  volatility, atomics, direct calls, tail calls, and control-flow edge meaning;
- pointer provenance, address-space requirements, and source-level capabilities;
- deterministic call chains for unsupported reachable behavior.

Ordinary scalar admission and General GEMM refinement consume this same owner.
They do not run separate importers. The current #174 work is accepted only when
generic capture is independent of ordinary-scalar authentication and scalar
lowering is a separate consuming adapter.

### Rustc and device target custody

The current compatibility backend analyzes the final crate in a host rustc
session while `cargo-fe2o3` separately configures the device compiler for
`gfx942`. These are two different target facts. Host-session layout and FnAbi
must never be relabeled as AMDGPU layout or FnAbi merely because device lowering
was selected.

Production collection therefore retains both the exact rustc layout context
and the fixed `gfx942:xnack-` device profile in one move-only token. The
semantic importer must consume that pair and fail closed on an unsupported
bridge. The intended convergence is for the explicit extraction driver and the
compatibility backend to enter the same importer under an AMDGPU rustc target
session; the compatibility backend may become a thin coordinator for that
session. Existing host-to-gfx942 conservative layout projections remain
qualification inputs and cannot mint production semantic identity. Until the
AMDGPU-session handoff exists, production stops before semantic-MIR admission.

## Canonical and executable IR

`fe2o3-mir-model` and `fe2o3-kernel-ir::Module` remain the canonical semantic
identity boundaries. Pliron operations are transient executable state.

The general Kernel IR module already represents functions, roles, signatures,
blocks, SSA values, control flow, memory effects, address spaces, barriers,
atomics, wave operations, matrix operations, capabilities, and inline assembly.
Profile records may validate or construct regression fixtures, but production
MIR lowering must emit the general module rather than select a profile-specific
replacement.

Conversion between canonical records and executable graph state is checked in
both directions. Identity never includes text rendering, traversal accident,
arena slot, pointer, process ID, or filesystem path. Frozen wire formats remain
byte compatible.

## Transformations

All mutable transformations execute through the sealed #140 service. A pass
receives an owner-authenticated operation handle and a bounded configuration;
it cannot receive or return a raw Pliron pointer.

### Session dependency boundary

The sealed service requires a dependency inversion around the dialect crates.
It must not be implemented as a public callback that receives `&mut Context`:
safe callback code could retain a contextless upstream `Ptr<T>` and recreate
the cross-session confusion that #140 is intended to remove.

The production dependency direction is:

```text
Pliron owner/registration core
    <- fe2o3 dialect definitions and typed constructors
    <- closed production Pliron session and transform adapters
    <- ProductionCompilation typestate transaction
```

The lower owner core contains context identity, bounded dialect-registration
actions, opaque handle mechanics, and fixed diagnostics. Dialect crates depend
only on that core and pinned Pliron APIs. The closed production-session layer
depends on the owner core plus the admitted dialect crates, owns the raw
`Context`, and directly invokes their typed constructors and transformations.
Its raw-context implementation is compiler-internal TCB code; it exposes no
callback, trait implementation point, context, pointer, value, block, type, or
attribute handle to callers.

Construction consumes a bounded canonical MIR or Kernel IR recipe and returns
an opaque root handle only after recursive verification and canonical
cross-checking. Transformation selection is a closed fe2o3-owned operation,
not an arbitrary caller-provided Pliron `Pass`. A transition consumes the
input-stage capability, reserves its complete work and growth budget, mutates
only the authenticated tree, recursively verifies the result, and returns a
new stage capability plus a canonical receipt. Any failure after allocation or
mutation begins poisons and terminally consumes the production session.

The current textual import and detached lowering services remain test and
migration bridges. They cannot be called by `ProductionCompilation`, and
removing their final production callers is part of #140/#178 rather than a
second compiler route.

Each pass performs this transaction:

1. Validate the complete input graph and canonical binding.
2. Reserve bounded work, diagnostics, graph growth, and nesting.
3. Apply one independently specified transformation deterministically.
4. Validate the complete output graph and declared analysis preservation.
5. Emit a receipt binding pass, input, output, resource use, and diagnostics.
6. Poison the session after a failure that may have partially mutated state.

The initial pass order is deliberately conservative:

1. unreachable control-flow removal and branch simplification;
2. eligible local-storage promotion to SSA;
3. constant propagation and folding;
4. dead value, operation, argument, helper, and symbol elimination;
5. equivalent pure-computation reuse;
6. bounded aggregate decomposition and helper integration;
7. loop normalization and explicitly bounded unrolling;
8. address-space refinement and memory-effect analysis;
9. uniformity, divergence, barrier, and synchronization validation;
10. ABI preparation and target-independent call lowering.

The implemented Wave 1 route assigns this work to three bounded stages rather
than pretending the whole list is one Pliron pass manager. Ranked recipe
construction performs unreachable-block pruning, CFG/guard normalization,
explicit memory typing, and checked index folding. Target-neutral lowering
performs scalar/fragment SSA promotion, aggregate ABI decomposition, intrinsic
lowering, and GPU/tensor/MFMA legalization. After formal memory admission and
target binding, the fixed V2 Pliron optimizer runs SCCP, CFG simplification,
select canonicalization, DCE, local pure CSE, DCE, and CFG cleanup. General loop
unrolling, alias-driven memory optimization, global CSE, scheduling, and cost
models remain disabled in that historical Wave 1 route pending their legality
and coordinate-map contracts.

The separately implemented checked native-V12 path has since added dominance
CSE in Policy3, bounded private-memory forwarding in Policy4/5, and checked
continuations. It is not the default/protected route described above. The
[checked middle-end architecture](pliron-optimizing-middle-end-v1.md) records
the actual fixed schedules, independent checks, and remaining activation gates.

No optimization is required for semantic correctness. A pass may reject or
leave code unchanged, but it may not select an old compiler route.

## Proof and verification

Verification does not select compiler implementation. The current MIR-to-KIR
receipt binds the exact semantic MIR identity to the production KIR wire
version, digest, and byte length. It retains complete block, statement,
terminator, synthetic-operation, and parameter correspondence plus the exact
semantic induction report. The formal-memory receipt independently binds the
same versioned KIR identity to the complete canonical obligation receipt and
its structural witness.

For gfx942, the independent proof-input validator strictly decodes canonical
KIR V8, checks complete contiguous operation coverage, replays the induction
analysis deterministically, and requires each admitted induction certificate
to name exactly one checked KIR addition in its retained source span. Hostile
span reassignment, report mutation, parameter rebinding, synthetic-trap drift,
and independently well-formed KIR identity substitution all fail closed. A
workload-specific Verus proof may discharge obligations for that module, but it
cannot replace MIR or Kernel IR or provide artifact authority.

The #106 General GEMM proof is therefore the first substantial producer of a
generic MIR-to-KIR correspondence receipt. The #174 consumer retains that
receipt with the same MIR owner. Later kernels use the same receipt type and
relation vocabulary with different proved obligations.

Source proof, compiler transformation validation, LLVM/ISA correspondence,
machine inspection, hardware observation, and runtime authority remain
separate evidence classes.

### Guarded memory obligations

Formal memory analysis distinguishes launch-envelope accesses from slice-bounded
accesses. A proved guarded access retains its actual predicate, slice, pointer,
element width and invocation range, plus symbolic bounds and alias obligations.
Selecting zero for an inactive index does not make that index globally equal to
the active index or require the slice to cover the entire launch.

Production admission partitions guarded reads using a freshly derived report
borrowed from the same live semantic KIR owner. Core-proved reads and reads still
requiring ranked structural proof must exhaust the actual body-order census.
The consumer rejects unconsumed, duplicate or out-of-order report rows,
foreign-owner reports, and missing or extra pending reason locations. It does
not interpret the absence of a ranked-proof reason as proof by itself.
Unsupported effects and other outstanding obligations retain their own gates.

The inert receipt facade preserves legacy V1 bytes and adds guarded V3/policy2
encoding for the symbolic representation. Singleton V4 admission evidence pairs
outer policy1 only with legacy V1, and outer policy2 only with guarded V3 and an
exact positive Bits64 structural witness. Legacy nested V2 is not admitted by
either pairing. Multiroot payload association remains an inert, exact-root
check, not singleton admission or live proof authority. Decoding, canonical
reencoding and matching identities do not authenticate compiler origin, prove
LLVM or machine refinement, or grant runtime authority.

## AMDGPU and finalization

One AMDGPU lowering owner centralizes exact target identity, features, wave
policy, address spaces, device libraries, code-object policy, resource bounds,
kernel metadata, calling conventions, and module flags. Textual LLVM is a
bounded Worker transport and inspection form, not a semantic identity boundary.

The production finalizer returns a generic move-only inspected-artifact owner.
It retains and freshly revalidates the exact compiler graph/handoff, Worker,
finalized bytes, descriptor, ELF, metadata, target, and ISA observations. The
successful Worker exchange now also retains exact linked-LLVM,
optimized-LLVM, generated-object, ordered native-input, path-independent LLD
invocation, and final-HSACO identities. Rust independently reconstructs that
record's identity, request-derived object order, linker policy, and output
identity; strict Worker V3 bootstrap and durable replay require every measured
stage to agree. The current General GEMM owner chain
under #173 is a qualification oracle for this generic boundary. Its three
late-machine axes must not become a second GEMM-only authority path. Exact
derivation custody is not formal LLVM-to-machine semantic preservation.

Generated host interfaces derive from the canonical Kernel IR ABI and are
checked against the inspected descriptor. They still grant no launch authority;
the runtime separately validates allocation, lifetime, launch geometry, and
device compatibility.

## Selector retirement

Cargo has no selector. It rejects both the obsolete pipeline and qualification
oracle environments rather than interpreting absence as a route choice.
Versioned `V1`/`V2`/`V3` suffixes identify frozen records and protocols, not
selectable implementations. Production build inputs use only
`FE2O3_PRODUCTION_BUILD_CONFIG_V1` with the
`fe2o3-production-build-config-v1` schema. Worker V2 config,
expected-identity, envelope, and source-debug controls are recognized only for
fail-closed rejection.

The Cargo qualification feature and executable branches are deleted. Keeping
a route behind `cfg(feature)` would be isolation, not convergence. Default and
all-feature tests compile the same production route; `cfg(test)` cannot
activate alternate Cargo compiler behavior. Backend-only differential oracles
remain temporary work under the backend deletion ledger below.

### Variant deletion ledger

| Area | State | Required deletion |
|---|---|---|
| Host launch surface | Complete | The private generated KFD invocation is the sole production direction. Worker V2 host admission, workload launch adapters, production raw-HIP loading/packing/launch, and compatibility aliases are deleted; separately named legacy HSA and unsafe-HIP surfaces are non-production qualification coverage. |
| Cargo production tests | Complete | Default tests compile `PreparedProductionBuildConfig`, `ManagedProductionBuild`, Worker V3 application handoff, and the fixed device-then-host plan. |
| Cargo qualification graph | Complete | The feature, simulation command, Worker V2 build/restart modules, S09 routing, workload parsers, fixture binaries, vertical tests, and Worker V2 bundle dependency are deleted. Worker V3 application fault coverage uses a compiler-neutral test feature. |
| Backend qualification graph | In progress | Default and all-feature backend libraries are feature-invariant, selector-free, and enter only `ProductionCompilation`. Exact-profile and Worker V2 modules are restricted to feature-enabled unit-test fixtures and cannot be selected by a rustc invocation. Preserve the minimal canonical differential fixtures, then physically delete the remaining modules and backend feature. |
| Finalizer and Worker execution | Open | Move shared protocol, request, admission, finalization, and executor mechanics to workload-neutral owners; migrate V3 callers; delete V2 executable APIs and workers. |
| Artifact restart compatibility | Open | Retain only versioned records needed for canonical decode, explicit rejection, or migration; delete V2 publication/recovery actions once V3 differential coverage owns their hostile cases. |
| Pliron and workload workers | Open | Replace Worker V2 executable bridges with production handoff fixtures or offline comparisons, then delete the worker crates and profile entry points. |
| Versioned schemas | Retained by design | V1/V2/V3 remains on frozen bytes, receipts, domains, and protocol records only. A suffix must not imply a selectable implementation. |

Migration follows these rules:

1. Add no workload-specific production implementation or selector.
2. Move exact-profile evidence into inert differential fixtures or offline
   tools, then delete the executable entry points from production crates.
3. Migrate a semantic slice only after ordinary attributed Rust passes the
   production transaction and differential tests match its existing oracle.
4. Once a slice migrates, preserve only the smallest authority-free fixture
   needed for differential coverage and delete the old implementation.
5. For a kernel-containing crate, unsupported production behavior is terminal.
   `legacy-v1` and exact-profile selectors are never fallbacks.
6. Host-only Rust code may continue through rustc LLVM; that is not a second
   device compiler implementation.
7. Keep non-authoritative comparisons only in offline qualification tooling. The
   compiler API has no implementation selector, and exact-profile qualification
   oracles retire as their differential coverage migrates.

Production became the sole unselected compiler transaction after the first scalar slice
completed its compile, host-interface, artifact, and hardware gates. It has no
selector. An incomplete production transaction now fails closed instead of silently
entering legacy codegen. Backend workload oracles are absent from default, all-feature, and test builds. `FE2O3_QUALIFICATION_ORACLE_V1` is rejected as backend configuration. Unselected host-only dependency units omit fe2o3's managed rustc
arguments and backend descriptor so rustc uses its built-in LLVM backend
directly.

The 2026-08-20 compiler review made this distinction structural. Production has no qualification table or corresponding
variant or selector. The backend has one protected publication call, Cargo has
one production intake without a schema selector, and production recovery is a
separate state machine from V1/V2 qualification recovery. Frontend-record validation does not weaken the boundary: `ProductionCompilation` receives only the move-only production closure and has no oracle helper. See
`compiler-convergence-review-2026-08-20.md` for the deletion inventory and
remaining complexity bounds.

## Migration order

The vertical slices migrate through the same transaction in this order:

1. fill and vector arithmetic;
2. scalar arithmetic, branches, and structured control flow;
3. loops, helpers, cross-crate generic and const-generic calls;
4. multiple kernels in one final crate;
5. global, private, and workgroup memory;
6. barriers, one wave operation, and scoped atomics;
7. scalar GEMM and parameterized tiled GEMM;
8. reductions, softmax, attention, and MoE.

For each slice, the old implementation becomes a differential oracle. Tests compare
canonical MIR, Kernel IR, ABI, artifact structure, numerical results, canaries,
synchronization behavior, and terminal cleanup before its selector is removed.

## Active issue alignment

| Issue | Role in the one production pipeline |
|---|---|
| #140 | Owner-authenticated graph handles and sealed transformation execution |
| #174 | Workload-neutral same-session MIR owner; General GEMM is its first demanding consumer |
| #106 | First mechanically checked producer of the generic MIR-to-KIR correspondence |
| #145 | Typed general AMDGPU to LLVM construction, not artifact authority |
| #146 | Pinned upstream LLVM and in-process LLD Worker consumer |
| #147 | Differential and hostile qualification for the LLVM/Worker boundary |
| #173 | General GEMM oracle for retained compiler/Worker/finalizer ownership and late-machine binding |
| #175 | Production transaction integration, migration order, and selector retirement |
| #176 | One workload-neutral rustc semantic MIR importer for both entry paths |
| #177 | Canonical semantic MIR to general Kernel IR lowering |
| #178 | Owner-authenticated deterministic middle-end transformations |
| #179 | Generic retained finalization and inspected AMDHSA artifact owner |
| #180 | Typed host-interface generation from canonical KIR and inspected ABI |
| #181 | Differential migration and exact-profile selector retirement |

## Parallel implementation lanes

Work remains parallel only at frozen ownership boundaries:

| Lane | Primary write ownership | Exit criterion |
|---|---|---|
| Session safety | `fe2o3-pliron`, owner-handle tests | #140 sealed pass execution with poisoning and receipts |
| Rust import | `fe2o3-mir-model`, `dialect-mir`, rustc importer module | both rustc entry paths return the same generic MIR owner |
| MIR to Kernel IR | `fe2o3-lower-mir-kernel`, correspondence tests | general `KernelModule` for the first scalar/control-flow slice |
| Kernel/GPU passes | dialect and lowering services | deterministic checked pass sequence over owner handles |
| AMDGPU/LLVM | `fe2o3-amdgcn-model`, production target lowering | complete typed target contract and canonical handoff |
| Worker/finalizer | Worker handoff and `fe2o3-hsaco-finalize` | generic retained inspected-artifact owner |
| Host/runtime | generated host and protected runtime adapters | ABI/descriptor agreement and one-shot checked launch |
| Migration/oracles | integration tests, scripts, evidence docs | each old selector removed after differential hardware gates |

Shared root manifests, exports, selectors, and the production transaction are
owned by the integrator. Lane changes merge only after their canonical records
and hostile fixtures are frozen, preventing parallel work from creating new
routes.

## Critical milestones

The current checkpoint has completed the feature-invariant backend entry and
the first source-authentic workgroup vertical slice. Scalar GEMM reaches
deterministic gfx942 LLVM with checked induction custody. Dynamic tiled GEMM and
attention currently stop earlier because their uniform loop bounds are not yet
admitted as exact total unsigned index expressions. The ordinary attributed
WG64 `i32` LDS reduction continues through the compiler-bound handoff, measured
upstream LLVM target APIs, in-process LLD, and inspected COV6 HSACO. Its exact
256-byte LDS and launch-resource contract survives every compiler and artifact
stage. The same route also requalifies the scoped atomic kernel. Neither path
uses COMGR, a shell linker, or a workload-profile selector, and neither
currently grants load or launch authority.

1. **Compiler middle end, bounded operational:** one importer carries the LDS
   slice through semantic MIR, ranked PLIRON, general Kernel IR, composed
   memory checks, and deterministic AMDGPU LLVM. General pass coverage remains.
2. **First production code-object slice, complete:** attributed LDS-reduction
   Rust reaches reproducible inspected gfx942 HSACO through only the production
   compiler and finalizer transaction.
3. **Safety semantics, in progress:** bounded references, LDS, barriers, and
   scoped atomics use the same transaction with hostile tests; general race,
   alias, convergence, and address-space proofs remain.
4. **Rust and verification:** current gfx942 receipts bind exact semantic MIR,
   canonical KIR V8, complete operation spans, formal obligations, and replayed
   checked-induction anchors. They now also bind neutral and target KIR
   identities, target profile, kernel ID, and exact pre-descriptor LLVM. The
   independent verifier reconstructs target KIR, reruns deterministic AMDGPU
   lowering and layout binding, and requires byte equality. The sole Worker
   continuation now records and independently replays exact linked and
   optimized LLVM, generated object, ordered native inputs, canonical
   in-process LLD policy, and final HSACO. Rust recomputes the request-derived
   and final-output relations and requires the worker-measured stages to match
   across replay. Singleton and multi-root roster Worker V3 admission
   independently decode the target-binding, AMDHSA-layout, and
   semantic-to-LLVM records; check every associated capsule receipt; own exact
   KIR-to-LLVM replay; cross-bind final LLVM to handoff and finalizer state; and
   match COV6 plus exact per-root workgroup facts to the admitted descriptor
   roster and physical symbols. Formal KIR-to-LLVM semantic preservation and
   LLVM-to-machine refinement remain open; no
   profile-selected semantic replacement or association-only lowering variant
   remains.
5. **Parameterized GEMM:** ordinary attributed Rust GEMM reaches inspected
   HSACO through the production transaction; #173 remains only an oracle.
 6. **Worker V3/KFD execution:** the source-bound artifact enters the sole
    application/verifier graph and pure-Rust KFD packet submission path.
7. **Selector convergence:** all exact-profile production selectors are gone,
   default kernel compilation uses the one transaction, and unsupported code
   fails without fallback.

No milestone changes a parity row until its protected evidence policy and
hardware gates independently qualify that row.
