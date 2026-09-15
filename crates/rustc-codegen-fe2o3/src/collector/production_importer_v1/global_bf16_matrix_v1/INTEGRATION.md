# Global BF16 Matrix Read Checkpoint

## State

AMD27f now rejects before the layout diagnostic: Bern's defined matrix
constructor hook requires `MatrixCapability<SubgroupBrand<Width, Root, Epoch>>`
where the existing BF16 source uses the actual deprecated
`KernelContext::matrix() -> MatrixCapability<KernelCapabilityBrand<...>>` route.
There is no subgroup epoch in that type. The fixture still retains its real
policy wrapper and checked Global constructors, and still requires full import
success; it has not been rewritten to avoid this incompatibility. A focused
pre-import assertion/diagnostic now preserves and prints the exact root-branded
matrix/policy bind signature. Bern owns the constructor classification fix:
do not synthesize InitialEpoch, collapse subgroup/root brands, or make the raw
BF16 read confer numerical policy authority. Existing Global root, read-only
allocation, lane, fragment and shared-loan checks remain required.

MIR27f diagnostic test baselines used Primitive fields for unit instead of the
required empty Arbitrary fields. The owned helper now uses the existing exact
unit layout pattern; the old malformed unit is explicitly a negative. Both
positive assertions and all original failure-location negatives remain, with
no production validator change. Parent rerun pending.

AMD25: the owned scratch manifest fix clears macro setup. Nominal A/B probes
remain 2/2 PASS, but full-import and SSA callbacks now reach complete semantic
MIR admission and fail with `InvalidTypeLayout`. The printed A/B terminal
signatures alone do not localize that rejection. No layout acceptance rule has
been changed based on those signatures.

After exporter26 PASS/freeze release, a diagnostic checkpoint is mounted:
`semantic_mir_v1/layout_diagnostics_v1.rs` reuses the existing per-type validator
and checks request-wide layout identity consistency without admitting anything.
A test-only hook before complete BF16 request admission reports the failing
canonical type ID, actual rustc type/layout, same-identity conflicts, and its
indexed aggregate/variant field offsets and child types. The original admission
call and error remain unchanged. Its four focused MIR tests cover localization,
identity conflict, misaligned fields, work bounds, and diagnostic-success not
being admission. No Cargo or tests run by this worker; parent rerun needed.

The finite next step is `global_bf16_full_import` or `global_bf16_source_ssa`
with the rebuilt test binary and actual AMD metadata, preserving the
`BF16 canonical layout failure` block. If type/layout diagnostics pass instead,
the failure is outside this diagnostic's type/layout-identity subset and must
be localized in complete request validation, not waived. The separate mounted
typed-terminal ABI callbacks remain `typed_matrix_terminal_abis_gfx942/gfx950`.

Mixed24 parent sweep: the previous Global BF16 load preflight failure cleared
in five kernels. All 47 kernels still fail, with stable binary hashes; this is
not a whole-kernel correctness or hardware qualification result. The isolated
four-callback harness repair below is mounted but still needs a central rerun.

The next two terminal ABI fixes are mounted in `../typed_matrix_terminal_v1.rs`:
`WaveLaneCurrent` accepts only the reviewed Wave64/UnbrandedCapability source
type; `F32MatrixAccumulatorIntoValues` accepts the real four-type-argument
accumulator and consumes its exact source type by value. The latter is a
projection for any Brand in the reviewed Rust method, not a new issuer or a
claim that two brands are interchangeable. Exact complete source type IDs
(including kernel/subgroup/epoch) remain in the ABI and operation. Source
carriage replays these checks. No new wire tag, producer binding, native-wave
waiver, matrix policy or memory proof is introduced. The old multi-operand
matrix helpers are deliberately unchanged.

The new actual-AMD tests reuse the repaired isolated manifest/metadata harness:
`typed_terminal_tests::typed_matrix_terminal_abis_gfx942` and
`typed_terminal_tests::typed_matrix_terminal_abis_gfx950` under
`collector::production_importer_v1::global_bf16_matrix_v1::tests`.
They cover real registered source import, retained terminal calls, exact
root/brand/epoch type substitutions after inert admission, ownership mismatch,
wrong profile/distribution/width/result and counterfeit definitions.
Run with existing AMD metadata and `--ignored --nocapture --test-threads=1`.
Only rustfmt and `git diff --check` ran here; central compilation/tests pending.
The importer module/dispatch/replay hooks and test-harness stage are all mounted;
no additional parent hook is needed for these two operations. Ranked Global
memory hooks remain unmounted and require parent approval as before.

Central 23c: both actual-AMD nominal probes PASS. The combined core log also
records all six BF16 lowerer component tests PASS and all five MIR BF16 tests
PASS. The four new full-import/SSA callbacks stop before collection because
the kernel macro cannot find the isolated child's Cargo.toml. The mounted
harness fix creates that manifest in owned scratch, pointing at the real
repository device crate; observation/registration derivation is unchanged.
These four callbacks still require a central rerun. No post-collection failure
or full source-path success has yet been observed for this fixture.

After exporter24 the parent lifted the source-hook freeze; the sweep has now
finished with stable hashes and input snapshot7 remains untouched. No Cargo/dependency rebuild,
frozen-artifact change, memory-proof acceptance hook or ranked-root change was
made by this worker. Pauli's new source memory API is still a detached,
unpredicated/nonvolatile/straight-line subset; it cannot yet discharge the
four guarded volatile BF16 events. The independent CPU request child remains
unmounted pending that common proof integration, with no duplicate proof graph.

Checkpoint 22b nominal probes reached rustc but failed the first nominal check.
The fixture omitted the managed invocation's Cargo metadata build observation;
provider authentication requires it even for ADT recognition. The harness now
uses an isolated test child with an observation derived from its actual
`-Cmetadata` argument, preserving the parent's `-Zunstable-options` fix. No
provider/source authentication checks changed. Actual cached AMD metadata is
still required. Central 23c subsequently verified both corrected nominal probes.

Four additional opt-in callbacks in `tests/import_tests.rs` exercise one real
registered A/B kernel through production collection and canonical V22 import,
then separately through replayed SSA, on gfx942/gfx950. They retain checked
constructors and Result forwarding, independent Global binds, both consumed
read terminals, and negative source-carriage checks for matrix/global brands
and A/B role substitution. These are positive assertions at each boundary,
not a waiver for unsupported paths. They have not been compiled or run here.
The harness derives the registration binding from its own crate/metadata;
it never constructs a production root or receipt by hand.

Ranked Global mapping remains explicitly unsupported: an independently
authenticated Global-origin memory reference is still required. No legacy
slice-origin alias, final-expression self-reference, or missing-write-contract
bypass was added. Pascal's live `tcx` argument in the shared root resolver is
also present at the BF16 child call site.

Mounted for the next central compiler build after checkpoint 21b. The schema,
decoder, nominal importer, root selection, complete source-carriage replay,
SSA classification, typed transport, live Global-loan resolver, custody-aware
enum planner and shared lane-load emitter are wired into their production
call sites. The missing Ord/PartialOrd derives and enum-planner exhaustive arm
are fixed. Parent reported MIR21 240/240 PASS, including all five BF16 tests;
the latest SSA/lowerer changes still need central compilation and tests.
No Cargo, source probe or test binary was run by this worker.

This is not full mixed-kernel support: the common graph has no proof for
cyclic loan generations, and the current Result custody cannot represent
different active variants at one SSA merge. The new live resolver reports
`global BF16 Result needs variant-sensitive payload SSA custody` when it
encounters an inactive payload on an incoming edge. The enum planner requires
a unique source, and restoration still requires strict source-block
dominance. A general extension needs per-variant incoming payload validity
and ownership, not a placeholder Global on Err edges or a dominance waiver.
Those are exact remaining invariants, not permission to mint new issuers.

The mounted wire allocation is MIR V22, top-level compiler intrinsic tag 80.
Pascal's workgroup-index conversion is a separate ExecutionOp, not tag 80.
Coordinate the shared V22 declaration/entry points once, without moving old
tags. Compiler terminal-expansion identity tags 194/195 are proposed for A/B;
both importer and semantic-plan identity encoders now use that same pair.
There is no new KIR operation, KIR source-wire change, or change to Ram's
borrowed tag 25 / Pauli's Math operation.

## Children

All paths are relative to the repository root.

- `crates/fe2o3-mir-model/src/semantic_mir_v1/global_bf16_matrix_v1.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1/canonical_decode/global_bf16_matrix_v1.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1/global_bf16_matrix_decode_tests.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/global_bf16_matrix_v1.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/global_bf16_matrix_v1/tests.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/global_bf16_matrix_v1/tests/import_tests.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/global_bf16_matrix_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/global_bf16_matrix_01/transport.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/global_bf16_matrix_01/live.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/global_bf16_matrix_01_tests.rs`
- `crates/fe2o3-pliron/src/production/semantic_ssa/adapter/global_bf16_borrows_v1.rs`

## Contract And Invariants

`SemanticGlobalBf16MatrixLoadV1` is an inert source contract, not authority or
numerical proof. It retains six actual semantic type IDs, A/B role,
Bf16F32M16N16K16 / Tile16x16 / wave64, row-major storage, distinct matrix/global
brand identities, root memory provenance, and the exact source callable ID.
The nominal importer checks the actual reviewed terminal DefId and device
ADTs. It preserves subgroup epoch in the full matrix brand and requires the
lane and fragment to have that same brand. Matrix and Global roots must match;
the matrix brand is not substituted for the Global brand.

The retained view shape is `(&Global<u16, ReadOnly, GlobalBrand>, usize,
usize, usize, usize, ZST, ZST, ZST)`. Source-order layout, immutable thin
reference, usize/u16 types and inhabited markers are checked. Source ownership
is shared/shared/by-value/by-value. The read records only a memory access;
admission still requires the separately authenticated Global bind. A read
cannot supply its own allocation binding.

Lowering consumes the actual retained Global value and compares its full KIR
context to the current authenticated root, including the root identity rather
than only marker/target/launch axes. The four index calculations retain
checked arithmetic, logical row/column bounds and the same Global's physical
length. Each guarded load uses that same GlobalCapabilityIndex, a zero u16
fallback, and a bitcast to BF16. Reads retain Global::load volatility. No
floating arithmetic, rounding relaxation, policy receipt or memory-equivalence
claim is introduced.

The proposed view transport carries one typed GlobalCapability and four U64
values. Reconstructing its descriptor from an existing typed SSA edge is not
GlobalCapabilityBind and does not establish a live borrow by itself. No raw
slice/pointer or free scalar may stand in for that capability.

## Hook Reference

The following hooks are mounted. The later source-custody discussion records
the original integration constraints; only the closed, unique-source subset
is admitted, and unsupported merges/cyclic loans still reject.

1. In `fe2o3-mir-model/src/semantic_mir_v1.rs`, declare/reexport the contract
   child, add `GlobalBf16MatrixLoad { contract: SemanticGlobalBf16MatrixLoadV1 }`,
   and coordinate V22 in the version constant/enum/conversions. The child has
   `admit_exact_v22`; do not also define that method in Pascal's child.
2. Add exact arms to `compiler_intrinsic_source_identity_matches`,
   `record_intrinsic_capability_claims`, the intrinsic branch of
   `validate_non_body_callable_abi`, and
   `enqueue_compiler_intrinsic_type_references`. Delegate respectively to
   `contract.source_identity()`, `record_claim`, `abi_matches`, and
   `contract.types().all()`. Do not put the read in the no-claim fallback.
3. `minimum_wire_version` selects V22 for this variant. The intrinsic encoder
   rejects versions below V22, writes tag 80 and calls the child `encode`.
   Legacy operations keep their old minimum versions and payload bytes;
   existing limits are unchanged.
4. In `canonical_decode.rs`, mount the decoder child, accept V22 in the
   production version policy, gate the maximum intrinsic tag at 80 only from
   V22, and decode tag 80 as `GlobalBf16MatrixLoad`. Mount the test include in
   its existing tests module. The child supplies `decode_exact_v22_canonical`.
5. In `production_semantic_terminal_v1.rs`, add A/B expansion variants and
   both trusted-item mapping directions for the existing reviewed device
   items. In `production_semantic_body_v1.rs`, both expansions have arity 4.
   The Rust method names are `load_m16k16` and `load_k16n16`, despite the
   diagnostic-item names ending in `load_zero_filled_v1`.
6. Mount the importer child. In `terminal_operation_v1`, delegate the new
   expansions to `global_bf16_matrix_v1::operation` with the actual instance,
   role, ABI/types, Rust inputs/output, authenticated root/contexts and source
   identity. Reserve the same 194/195 expansion identity tags in both this
   importer and `rustc_semantic_plan_v1.rs` after checking for concurrent use.
7. In the lowerer parent, include `global_bf16_matrix_01.rs`; the new intrinsic
   dispatch calls `lower_global_bf16_matrix_load_v1` with the actual callable
   source identity. Include `global_bf16_matrix_01_tests.rs` in resource tests.
8. Add the internal `SemanticPromotedBindingV1::GlobalBf16MatrixView {
   contract: SemanticGlobalBf16MatrixLoadV1 }`. Its types/values/restoration
   arms delegate to the three `global_bf16_view_*_v1` helpers, retaining the
   normal `binding_from_transport` exact-type check before restoration.
   `current_wave` is None for the view. Register both the view descriptor and
   the existing MatrixFragment descriptor from the read contract using the
   conflict-rejecting `insert_compiler_issued_ssa_binding_v1` helper.
9. Preserve the legacy `lower_bf16_matrix_load` arity, five-component slice
   checks and u16 check. Replace only its common lane-load tail with
   `lower_bf16_matrix_load_components_v1`, passing
   `Bf16MatrixStorageV1::Slice { value: bits, slice: slice.clone() }` and the
   four cloned coordinates. This is the shared generic emitter, not a second
   bespoke matrix lowering. Legacy slice reads remain nonvolatile.
10. Every exhaustive promoted-binding match, especially enum payload planning,
    must handle the new descriptor without falling through to raw pointer
    flattening. The custody-dependent hooks below are required before an
    actual source-path claim. Ranked projection must reject this operation
    specifically until it has an exact Global-origin mapping; it cannot reuse
    the legacy slice origin by discarding allocation or brand information.

## Real Source-Custody Dependencies

Mounted resolver: `global_bf16_live_v1::verify` revalidates the real owner,
selects its exact expanded root/body/SSA plan and traces every new read back
to an actual matching read-only Global bind. It follows immutable reference
copies/reborrows and exact aggregate/known-variant projections using
`CapabilitySsaGraphV1`, which checks the original owner's loan at the consumer.
Same-typed foreign issuers and missing definitions do not qualify. The
address-transparency classifier reuses the existing bounded fork-aware
classifier; it accepts only the exact Global matrix capture, metadata field,
and actual A/B terminal signatures. There is no separate graph solver.

The enum planner does not allocate private slots for the capability-bearing
view. It requires existing unique-source custody, validates the retained typed
Global/coordinate binding at storage time, then uses the unchanged strict
dominance restoration. General tagged Result SSA remains the gap above.

### Captured Global Borrow

`tensor.rs`'s `GlobalBf16MfmaMatrix::checked` calls `bits.len()` and captures
the same `&Global` in field 0, then returns `Result<Self, ...>`. The legacy
adapter currently poisons references captured by an aggregate and rejects
consumed alias chains. Merely adding matrix receiver/lane terminal acceptance
does not admit this path. The fork-aware Workgroup classifier still calls the
same aggregate-escape invalidator. Pauli's Context terminal roster extension
does not by itself cover Global capture or the ordinary expanded `len` body.

Reuse the shared `CapabilitySsaGraphV1` mechanics and actual replayed owner,
not a new graph solver or an ambient type whitelist. A scoped resolver must
trace the actual Global bind through immutable copies/reborrows into the exact
matrix field and prove the original owner's loan live through every read.
Unknown capture/call, mutable/raw exposure, owner move/overwrite/StorageDead,
ambiguous incoming versions and cyclic storage generations must reject.
Only after that relation is present may the adapter make those exact sites
address-transparent. A contract or terminal roster entry alone is not that
relation.

The lowerer already has `global_capability_physical_field_v1` and
`slice_metadata_carrier_v1` for Global::len's exact retained physical field.
Reuse them: they retain the actual Global capability value and authenticated
root/bind, rather than exposing an ordinary slice.

### Result Payload Custody

`plan_enum_payload_storage_v1` must not spill the new Global-bearing view as
an ordinary aggregate. Reuse `plan_unique_enum_payload_sources_v1`,
`enum_payload_compile_time_custody` and strict source-block dominance on
restoration. The existing generic custody predicate already recurses through
retained Aggregate/Global bindings. A new planner arm can require unique
source custody, but must not invent a source for copied/merged Result values.
Trace actual expanded constructor/Result forwarding before deciding whether
the current unique-source relation suffices; unresolved aliases remain a
specific rejection. Validate the new view's typed Global/coordinates when
retaining it, not only its non-storable shape.

## Central Verification Plan

There are 5 MIR tests, 6 lowerer component tests, 1 source expansion-map test,
2 ignored actual-AMD nominal probes, 2 full-import probes and 2 SSA probes.
None have been executed by this worker.
The probes require existing actual device/core/builtins metadata and host
dependencies via the existing FE2O3_CORE_TRY_* fixture variables; they never
build dependencies or manufacture device source evidence. They check real
terminal/type identities and real checked A/B constructors followed by `?`
and the read terminal, not full canonical import. The new live-graph component
test covers an actual bind/capture flow, owner StorageDead and a substituted
non-Global producer. Component scaffolds do not claim source authentication.

Run codec tests for A/B, every truncation/suffix, old-version rejection,
legacy byte preservation, wrong layouts/roles/source IDs, ABI ownership and
read-without-bind. Run component tests for actual same-allocation guards,
volatile zero fallback, exact BF16 bitcasts, foreign bound/allocation/root
(including equal marker/target/launch with a different root), source identity,
plain-slice substitution and non-current lanes.

After the two custody integrations, add and run actual source collection ->
expanded SSA -> lowerer cases for checked A/B construction, legitimate Result
forwarding, repeated helper calls and retained subgroup brands. Negative cases
must cover foreign Global, changed epoch/role, mutable/raw escape, dead or
overwritten owner, and ambiguous/non-dominating Result payload. Then rerun the
five real mixed kernels. The existing typed-read memory-equivalence and final
write-contract gaps stay unresolved; no free-variable substitution, frame
waiver, or changed expected PASS is part of this work.
