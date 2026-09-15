# Workgroup Source Transport Checkpoint

## Pauli API Coordination

Importer owner AGREES with the epoch APIs now present in Pauli's detached
`semantic_mir_v1/workgroup_borrow_v1.rs` and its `INTEGRATION.md`.
The source-field mapping and retained-body attachment below are compatible.
This session has no direct agent-message tool; this shared section is the API
handoff. Parent's Math integration and terminal identity schema V5 are outside
the importer owner's edits. No additional execution terminal IDs are needed.

Activation status: at this review, the MIR parent still has no module/re-export,
`SubgroupDeriveBorrowed` variant, defined-contract field, or V20 enum/version
hooks. Compiler source files remain frozen; do not import detached APIs into
the live importer before those parent hooks exist. No Cargo/test run performed.

### Agreed Epoch Mapping

After the existing exact-provider/root/original-MIR checks, preserve the
`WorkgroupEpochProjectionSourceV1` until the original semantic function body is
constructed. Verify `source.receiver().source_identity() == body.identity()`,
then attach the record using the real `function_id` from the producer loop:

```rust
let receiver = source.receiver();
let types = SemanticWorkgroupEpochProjectionTypesV1::new([
    receiver.reference(),
    receiver.workgroup(),
    receiver.output(),
    source.epoch_type(),
]);
let record = SemanticWorkgroupEpochProjectionV1::for_defined_function(
    function_id,
    &body,
    types,
    receiver.provenance(),
    receiver.brand(),
    receiver.epoch(),
)?;
let body = body.with_workgroup_epoch_projection(record)?;
```

Importer errors will map the two builder errors to `SemanticSchema`; no fallback
to an unannotated body is permitted. The convenience builder can delegate to
`SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection`; the importer
does not depend on unrelated Math variants. Receiver argument/source field are
fixed at 0/2 on both sides. Source function identity and body digest are derived
by Pauli's builder, not supplied from a name, layout, fixture or caller label.

### Source Shape Confirmation For Pauli

Confirmed by source inspection, not a new executed canonical callback:

- `production_semantic_body_v1.rs:923` converts `Rvalue::Ref` directly into
  `SemanticRvalueKindV1::Borrow`; `semantic_borrow_kind_v1` retains Shared.
- `production_semantic_body_v1.rs:1521` preserves Dereference and Field(2),
  with each projection's normalized result type. Locals retain Return and
  Argument(0) roles; the recipe need not assume semantic local-number ordering.
- The epoch source validator requires exactly two locals, one block, one
  assignment and Return, and the importer hook requires pointer equality with
  the original retained rustc body. The ordinary Rust-ABI getter does not enter
  the RustCall/closure expansion paths. No statement/body rewrite is proposed.
- `execution.rs:252` declares `size: u64`, `rank: u64`, the nominal epoch at
  ordinal2, and the marker. `production_semantic_types_v1.rs:237` imports every
  declared struct field in source order, including ZST fields. Thus the owned
  Workgroup keeps all four fields; it is not an empty capability aggregate.
- Pauli's bounded V19 fragment digest and V20 defined-function attachment can
  bind that exact body. The checked expansion API must continue to resolve this
  source record through source-function/call-instance origins, not copy it onto
  the expanded root. Its result is still not Workgroup SSA equality evidence.

Borrowed subgroup interface also agreed as documented by Pauli:
`SubgroupDeriveBorrowed { workgroup_reference: source.reference(),
workgroup: source.workgroup(), subgroup: source.output(), width: 64 }` inside
the existing execution contract with the same source identity, exact signature,
root provenance, brand and epoch. Pauli owns MIR execution payload tag25 under
intrinsic73/V20; these are NOT new production terminal IDs or CombinedV5 tags.

Remaining activation dependency: Pauli/parent mounts the exports and complete
V20 encode/decode/minimum-version/admission/defined-contract hooks. Importer
owner will then replace the two explicit rejections with the above canonical
construction and add field-carriage tests, without changing owner-equality or
unsupported-lowerer gates. Please signal when those parent hooks are on disk.

### Existing Source API

Source API already available in `workgroup_source_v1.rs`:

```text
subgroup_workgroup_reference_source_v1(tcx, instance, abi, types, root, contexts)
    -> Result<WorkgroupReferenceSourceV1, ProductionSemanticImportErrorV1>
workgroup_epoch_projection_source_v1(tcx, instance, abi, types, root, contexts)
    -> Result<WorkgroupEpochProjectionSourceV1, ProductionSemanticImportErrorV1>
```

`WorkgroupReferenceSourceV1` exposes `reference()`, `workgroup()`, `output()`
(SemanticTypeIdV1), `source_identity()`, `provenance()`, `brand()`, `epoch()`,
and `receiver_argument()` (fixed zero). `WorkgroupEpochProjectionSourceV1`
exposes `receiver()` (the complete reference source), `epoch_type()` (owned
epoch pointee ID), and `source_field()` (fixed source ordinal two).
All constructors authenticate exact reviewed provider, Rust nominal types,
signature/ownership, and root; no source-name/layout proof shortcut.

Original interface request, now matched by Pauli's detached epoch child and
borrowed-operation proposal (activation still depends on parent hooks):

1. `SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
   workgroup_reference, workgroup, subgroup, width }`, inside the existing
   execution contract. Source signature `[workgroup_reference] -> subgroup`;
   preserve distinct reference/pointee IDs. Importer supplies the existing
   provenance, brand, epoch, callable source identity and width64 field-for-field.
   Keep `ProductionExecutionTerminalV1::SubgroupDerive` ID2 unchanged.
2. A versioned defined-function epoch projection contract with the four IDs
   `[workgroup_reference, workgroup, epoch_reference, epoch]`, source function
   identity, root provenance, brand and epoch identity. Receiver argument zero
   and the `[Deref, Field(2, epoch)]` recipe can be fixed by its closed variant.
   Please expose an attaching builder/getter on `SemanticFunctionDeclV1`, e.g.
   `with_workgroup_epoch_projection(contract) -> Result<Self, SemanticMirErrorV1>`.
   The builder/admission must bind to that exact function identity/ABI and
   validate the unchanged source body, not replace it with an intrinsic.
3. On the importer side, the epoch hook will return an optional checked source
   record, construct the existing original function body, then attach the
   canonical contract to that function. Non-epoch bodies stay byte-identical
   under their historical schema. The plan's exact original-MIR pointer check
   and unique authenticated root check remain. Getter stays out of the terminal
   roster; no new terminal identity tag.
4. Please confirm direct-call expansion's retained source-function/body/call
   mapping carries the projection contract (or rejects if unsupported). No
   record may be silently dropped during `SemanticFunctionDeclV1::new` rebuilds.
   The record identifies callee argument zero; only the checked caller mapping
   can establish the existing owned Workgroup SSA origin. KIR same-owner ValueId
   equality is still mandatory and is not established by this inert MIR record.

Expansion detail: `semantic_direct_call_expansion_v1.rs` rebuilds the expanded
root using `SemanticFunctionDeclV1::new` near line865, while preserving each
callee's function/local/block origin and the canonical source digest. Do NOT
copy a getter's function contract onto that different expanded root identity.
Keep it on the original defined getter, available from the lowerer's
`semantic_ssa.source_semantic().functions()[origin.function]`, and require
checked expansion/replay plus the exact call-instance mapping to consume it.
The new getter metadata must participate in canonical source encoding/digest;
any separate source-function reconstruction must preserve it or reject.

Pauli: the source owner confirms the API/body-shape agreement above. Please add
the mounted codec/admission hook locations here (or send them via parent) when
ready. Importer owner can then replace both explicit transport rejections and
add canonical field-carriage tests without touching Math/V5.

Status: mounted in `subgroup_partition_v1.rs`, with two bounded hooks in
`production_importer_v1.rs`. No MIR/KIR schema, lowerer, backend, machine authority,
or terminal roster changes. These edits are NOT in the parent's checkpoint6
8/8 AMD and 3/3 collection-to-canonical results. Child formatting checked locally;
new tests await parent compilation/execution. No Cargo, SSH or network invoked.

## Integrated Collector Hooks

- `execution_terminal_operation_v1`, `Terminal::SubgroupDerive`, now calls
  `subgroup_workgroup_reference_operation_v1`. It authenticates the full source
  contract and exact callable identity, then rejects with
  `subgroup derivation requires versioned workgroup reference and owned SSA transport`.
  It no longer emits the legacy operation with `&Workgroup` in the owned type slot.
- `construct_complete_request_v1` calls
  `require_workgroup_epoch_source_transport_v1` for each defined function before
  constructing its body. Non-epoch functions are unchanged. The exact reviewed
  epoch getter requires unique authenticated call-graph root custody, the full
  source contract, and pointer equality between retained plan MIR and original
  rustc MIR. It then rejects with
  `workgroup epoch requires retained authenticated projection and caller SSA transport`.
  This is intentional until a canonical checked projection carrier exists.
- Partition derive uses `exact_subgroup_source_layout_v1` only after provider,
  nominal receiver identity, brand/epoch, and ABI checks. It requires the real
  four-byte subgroup containing a four-byte lane containing unsigned u32, with
  exact field offsets and inhabited ZST markers. Partition consumer receivers,
  partition results, and epoch pointees still require inhabited aggregate ZSTs.

These hooks validate source facts; they do not admit same-workgroup SSA custody.
There is no caller-origin proof in the current source record and no permission
to manufacture an owned Workgroup from a source reference. A graph containing
subgroup creation will normally reject in terminal construction before reaching
the defined epoch getter hook. Both remain independently guarded.

## Source Helpers

The child is mounted with `pub(super) mod workgroup_source_v1;`. Its source
record entry points are visible only inside `collector::production_importer_v1`:

- `subgroup_workgroup_reference_source_v1`: authenticate the reviewed
  `WorkgroupCapability::subgroup::<SubgroupWidth64>` instance and its exact
  `&Workgroup -> Subgroup` source ABI. Return BOTH the source reference ID and
  its owned Workgroup pointee ID, plus output ID, source function identity,
  authenticated-root provenance, nominal kernel brand and synchronization epoch.
- `workgroup_epoch_projection_source_v1`: authenticate the exact reviewed
  `WorkgroupCapability::epoch` helper, shared receiver and shared epoch result,
  matching brand/epoch, and its complete bounded MIR body. The only accepted
  executable shape is `_0 = &((*_1).2); return`. Field 2 must actually be named
  `epoch` and have the instantiated WorkgroupEpoch type. The semantic aggregate
  field and returned reference pointee must agree too.

Neither result is an authority token. Both identify source argument zero as
the owner input. The epoch result also identifies source field ordinal 2,
not a layout byte offset. A caller must transport the actual argument's origin;
it cannot use these nominal facts to construct an empty aggregate or context.

The local tests inspect host rustc MIR, preserve the argument-local origin,
reject another same-typed field, wrong epoch, second owner, mutable receiver,
raw result, and extra effects. Even the positive local body fixture must fail
the reviewed-provider gate. New cached-device host/AMD callbacks additionally
authenticate the real subgroup and epoch providers, require direct FnAbi,
inspect actual subgroup field types/layout, and retain distinct source receiver
places for same-owner and other-owner calls with identical Rust types. They
reject local lookalikes and substituted width/brand and mutate only test-owned
MIR clones. Source bodies remain immutable. These tests make no root-issuance,
SSA-admission, launch, machine-proof, or GPU-qualification claim.

## Required Representation Integration

1. Parent-run the mounted source tests below. The seven partition tests now use
   non-ZST subgroup storage and retain all prior rejection checks; a new negative
   case rejects the old lane-free fixture and non-ZST partition views. Confirm
   the cached host/AMD layout callbacks before claiming source readiness.
2. Keep source reference and owned authority identities distinct for subgroup
   creation. The legacy `SubgroupDerive { workgroup, subgroup, width }` uses
   `workgroup` for both its source signature and operand's nominal type.
   The old importer filled it with `&Workgroup`; that route now rejects. The
   issuer's SSA value is nominally `Workgroup`. A separately encoded borrowed form needs
   `{ workgroup_reference, workgroup, subgroup, width }`, source signature
   `[workgroup_reference] -> subgroup`, and one existing Workgroup operand.
   Preserve legacy canonical bytes; append and version-gate the new form rather
   than changing the old payload or relaxing its exact type check.
3. Retain the epoch getter's actual source call and body through collection and
   checked call expansion. Bind callee `_1` to the corresponding caller operand,
   including call-instance coordinates. The source record is not a substitute
   for that binding. Do not grant every zero-sized WorkgroupEpoch a transport.
4. In the bounded SSA-origin path, recognize only the authenticated shared epoch
   projection of that bound Workgroup. Carry the existing Workgroup ValueId;
   retain the epoch reference's source type separately. Validate nominal field
   identity, issuer provenance, brand and epoch before forwarding that value.
   This needs a checked source/projection recipe at the importer-lowerer boundary,
   not a global field-2 or same-type shortcut. Arbitrary projections still reject.
5. A partition derive must receive `[subgroup_value, workgroup_value]`, and the
   subgroup's issuer must itself consume exactly `workgroup_value`. KIR currently
   enforces this ValueId equality and matching provenance/brand/epoch. Extend
   issuer recognition for the new borrowed subgroup form; do not remove equality.
6. Preserve consumer operands as `[partition, f32]` and `[partition, f32, u32]`.
   Checked helper expansion is required for the advanced-kernel wrappers:
   `semantic_execution_capability_01.rs` currently rejects expansion identities.
   Backend execution-capability function/block parameters also remain rejected;
   do not treat that rejection as permission to drop arguments.

Shared files affected by that integration are the importer/collector call hooks,
MIR/KIR operation codecs and exhaustive matches, lowerer execution transport and
origin/transport-plan children, KIR verifier, backend/sim issuer recognition,
and analysis classification. Only the scoped collector hooks are edited here.

### Exact Next Parent Hook

Replace the two explicit transport rejections only when both sides of the
following checked boundary exist:

- A new version-gated borrowed subgroup operation carrying
  `workgroup_reference`, `workgroup`, `subgroup`, `width=64`, plus the existing
  source identity, signature, root provenance, brand and epoch. Preserve old
  payload bytes and old minimum versions. Coordinate the new wire version/tag
  with the concurrently integrated numerical-policy consumer; do not repurpose
  legacy SubgroupDerive or a policy tag.
- A canonical epoch projection record tied to the retained DEFINED function:
  exact function/body identity, `reference`, `workgroup`, epoch-reference output,
  epoch pointee, root provenance, brand, epoch, receiver argument zero, and the
  checked source projection `[Deref, Field(2, epoch_type)]`. Bind it into the
  source transcript and canonical request; retain the actual body and calls.
  `WorkgroupEpochProjectionSourceV1` supplies the source facts, not this custody.
- Checked direct-call expansion must retain/remap the record's function/type
  coordinates and bind argument zero to the exact caller operand and call
  instance/expansion identity. The lowerer's existing owned Workgroup origin
  must be recovered from that operand, not selected by type/brand/layout.
- The bounded origin path forwards that existing Workgroup ValueId for the
  authenticated epoch projection while retaining its source reference type.
  Partition derive must compare it with the subgroup issuer's input ValueId.
  Same type/brand/epoch but another owner must reject, including through nested
  wrappers. Stale epoch, fabricated ZST, raw/mutable borrow, divergent merge,
  lost call-instance evidence and unsupported transport must also reject.

The collector cannot implement or test the final SSA equality within its owned
scope. The real-source tests below demonstrate that both caller origins remain
distinct; they do not substitute for these lowerer/KIR positive/negative tests.

## Parent Test Commands

After rebuilding the compiler test binary, with the pinned toolchain on PATH
and its lib directory on LD_LIBRARY_PATH:

```sh
"$BIN" collector::production_importer_v1::subgroup_partition_v1 --nocapture --test-threads=1
"$BIN" real_workgroup_amdgpu_source_retains_distinct_borrows_and_lane --ignored --nocapture --test-threads=1
"$BIN" real_workgroup_host_source_retains_distinct_borrows_and_lane --ignored --nocapture --test-threads=1
```

The first command selects 12 ordinary tests and leaves the two cached-device
callbacks ignored. The AMD command uses the same four `FE2O3_CORE_TRY_*`
metadata paths as `run-local-amdgpu-checkpoint5.sh`. The host command needs
`FE2O3_POLICY_DEVICE_RLIB`, as the policy-pairing callback does. Each callback
runs in a child process with its own exact rustc metadata observation; it does
not mutate the parent process environment, build dependencies or select another
target on failure. Tests are present but not run in this checkpoint.

The callback's Rust SOURCE was separately compiled using pinned
`nightly-2026-04-03` rustc `-Zunpretty=mir` against the exact cached AMD metadata.
It passed: same-owner calls retain `subgroup(copy _1)` and `epoch(copy _1)`;
other-owner calls retain `subgroup(copy _1)` and `epoch(copy _2)`. Only the three
existing target-feature warnings were emitted. This checks source/MIR shape,
not compilation or execution of the new collector validators/callbacks.

## Separate HSACO Baseline

The earlier first-worker checkpoint was 15 passed / 1 ignored. HSACO admission
was 16 passed / 12 failed / 2 ignored. The failing positive tests and finalizer
were unchanged from HEAD: legacy admission now stops at the existing
`MissingMachineRefinementEvidence` gate (ten unwrap-based tests, plus two expectations
for errors downstream of that gate). No capture-content or replay mismatch was
observed in that log. This is an independent missing-machine-owner baseline,
not evidence of a capture-required regression. Positive tests, environment-test
selection, captured pass occurrence proof and `binding_wrapper: None` were left
unchanged; capture remains required by default. No fresh differential run was
performed here.

## Source Callsites

- `examples/gfx950_advanced_attention/src/kernel.rs`: derive calls at lines 411,
  532, 1232, 1335, 1814; wrappers at 166 (`partition16_reduce_sum_v1`),
  178 (`partition16_broadcast_v1`), 191 (`partition4_reduce_sum_v1`).
- `examples/gfx950_advanced_attention/src/kda_baseline.rs`: derives at 123, 242.
- The Sinkhorn broadcasts at `kernel.rs:1825-1829` use `(lane & 3) + 4/8/12`.
  Current bounds proof accepts constants/direct masks only. It needs bounded
  non-overflowing range propagation for these additions, preserving the original
  lane SSA value; inserting a mask is not an acceptable proof.

End-to-end source readiness requires the above hooks, canonical compatibility
tests, paired same-owner/different-owner source cases, stale-epoch rejection,
and convergence/exact-participation/lane-bound checks on the resulting KIR.
