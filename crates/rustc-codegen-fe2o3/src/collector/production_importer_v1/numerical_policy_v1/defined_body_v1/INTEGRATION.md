# Original Source Math Attachment

Checkpoint: scoped adapter and both importer hooks are mounted. The two
cached-provider source tests and registered-source Math full-import assertions
are implemented. Rustfmt checked; no Cargo or test execution by this worker.
Parent owns central compilation; Pauli owns Math SSA following MIR21 integration.
The canonical-index correction below is now implemented, awaiting central tests.
Existing Current guard/tests remain frozen.
The Math full-import harness retains the parent's repo cwd and scratch out-dir.

## Mounted Hooks

Both hooks below are now present, using Pauli's exposed MIR21 API:

```rust
// collector/production_importer_v1/numerical_policy_v1.rs
pub(super) mod defined_body_v1;

// collector/production_importer_v1.rs, construct_complete_request_v1:
// immediately AFTER the complete functions loop, BEFORE new_with_callables
numerical_policy_v1::defined_body_v1::attach_math_defined_contracts_v1(
    tcx, plan, &types, &mut functions, &callables, kernel_contexts,
)?;
```

Do not move the call inside the function loop: a getter may precede its bridge
in canonical function order. Do not replace functions with intrinsic callables.
The adapter takes completed rosters and the actual retained preflight owner;
no consumer contract, FP operation or consumer-derived type table is an input.

## Constructor API For Pauli

Important source-integration finding: preflight sorts locals and blocks by
identity (`rustc_semantic_plan_v1.rs`, local sort near 2555 and block sort near
2601). Its raw_to_semantic mappings are not generally identity maps. The
detached Math validators initially assumed return local0/argument local1 and
entry block0/exit block1. The actual source sweep confirmed both mismatches.
They now resolve unique locals by exact Return/Argument/Temporary role and type,
entry through `body.entry()`, and exit through the exact CallReturn target.
Counts, statements, destinations, edges and distinctness remain closed. Eight
`defined_math_permutation` tests cover sorted identities, all local orderings,
getter/bridge block orderings, full V21 roundtrip, and retained rejection checks.
Central tests and real source full-import are pending. The source adapter
preserves canonical ordering; it never reorders bodies to make a predicate pass.
No payload field or constructor signature changed.

The adapter uses the existing detached API, without requesting new fields:

```text
SemanticKernelMathDeriveV1::for_defined_function(
  getter_id, functions, callables, types,
  SemanticKernelMathDeriveTypesV1::new([
    context_reference, context, branded_math, unbranded_math]),
  provenance, kernel_brand)

SemanticPolicyMathBindV1::for_defined_function(
  bind_id, functions, callables, types,
  SemanticPolicyMathBindTypesV1::new([
    math_reference, math, policy_reference, capability, bound]),
  provenance, strict_policy, kernel_brand)

SemanticDefinedCapabilityContractV1::KernelMathDerive(record)
SemanticDefinedCapabilityContractV1::PolicyMathBind(record)
FunctionDecl::with_defined_capability_contract(contract)
```

Getter/bridge/Current IDs are cross-checked against original source instances
and retained edge recipes after construction. MIR21/tag1/tag2 is selected by
the model's minimum-version logic; the source child does not hardcode or rename
wire versions. Preserve MIR20 epoch and MIR19 consumer bytes.

## Validation And Custody

1. Recognize only the exact reviewed KernelContext::math helper and existing
   PolicyMathBind marker. Unmarked lookalikes, consumers, Current and the bridge
   receive no attachment. Getter and bridge additionally require the reviewed
   safe external-helper authentication and retained original MIR.
2. Normalize actual signatures. Match getter context axes to its output's
   KernelCapabilityBrand and bridge instantiation. Require CurrentTarget and
   RegisteredLaunch. Bind reuses the existing strict-policy/source validator,
   then checks its entire original aggregate body and copied shared references.
3. Select exactly one authenticated root through retained direct-call edges.
   Match the real receiver brand's kernel identity to that root; cross-check
   the canonical root function identity, export binding and root rosters.
   A function shared across roots rejects even when nominal types happen to
   match. No context, epoch or policy authority is manufactured.
4. On programs containing a candidate, reconstruct canonical types and ABIs
   once with the existing converters. Require exact payload equality with the
   completed rosters, not merely unchanged hash labels. Check all function and
   terminal identity axes against their actual rustc instances. Index original
   instances through canonical identities; require exact Instance equality.
5. Require retained plan MIR to be the original tcx.instance_mir body for each
   attributed getter, bridge and Bind. Check getter -> bridge and bridge ->
   Current against the complete source recipe tables. Run the original body
   predicates and canonical record constructors. Cross-check local/block
   identities, source metadata, types, roles and entry against their retained
   canonical mappings, without assuming raw MIR indices survive sorting.
   Current also passes the
   frozen exact MathCurrent source/ABI guard.
6. Build all modified function clones first; any source, root, roster, ABI,
   recipe, record or conflicting-attachment error leaves the caller's entire
   function slice unchanged. Then apply only the validated Math attachments.
   Existing unrelated policy, epoch, reference and Darwin edits are preserved.

The extra converter pass occurs once and only when a getter/Bind candidate
exists. Root reachability reuses the existing authenticated-root routine and
one retained edge vector; no new root metadata or source occurrence table is
introduced. The bridge stays unannotated and cannot independently issue Math.

## Tests And Remaining Work

Two ignored tests are mounted with the adapter; neither builds dependencies:

```text
defined_math_original_source_host
defined_math_original_source_amdgpu
```

Host uses FE2O3_POLICY_DEVICE_RLIB. AMD uses FE2O3_CORE_TRY_DEVICE_RMETA,
FE2O3_CORE_TRY_HOST_DEPS, FE2O3_CORE_TRY_AMDGPU_CORE and
FE2O3_CORE_TRY_AMDGPU_BUILTINS. They fork with their exact metadata observation,
keep cwd at the repo for reviewed source paths, and direct rustc output to a
fresh test scratch directory. They inspect real getter/bridge/Bind source,
reject lookalikes and consumer-as-constructor cases, test generic/brand/target/
launch substitutions, require original bodies, and reject erased calls or
swapped/moved reference fields. A different kernel monomorphization stays a
different source identity, not a falsely authenticated same-root value.

The existing registered full-import tests now require V21:

```text
policy_math_all13_full_import_gfx942_v21
policy_math_all13_full_import_gfx950_v21
```

Their `math/import_tests/defined_sources.rs` child checks exact original getter,
bridge, Current and Bind identities, ABI/reference edges and retained source
MIR, all 13 consumers, idempotent attachment, exact V21 roundtrip, V20 rejection,
and replay-checked defined-call occurrences. It also rejects wrong-body,
root/type/callable roster and coherent hostile nominal substitutions without
partial attachment. Tests resolve canonical places by role and entry/edge,
never raw ordinal. The stable fixture metadata discriminator remains v19;
it is not the document wire version. Central execution is still required,
after rebuilding with the canonical-index correction. The 14 detached MIR tests cover
typed body/codec mutations independently; they are not full-import evidence.

Later SSA lowering must consume replay-checked defined-capability occurrences
and actual dominating KernelContext/Math/policy owners, tracking reference
copies, moves, reborrows, frame lifetimes and generation invalidation. This
adapter leaves all numerical/target proofs and downstream rejections intact.

## KIR Coordination

No KIR edits are part of this source checkpoint. Lagrange owns the optional
replay-checked source-occurrence carrier and payload revision 5, including the
lowerer source adapter. Revision 5 must not be consumed for another extension.
Ram now owns operation tag 25 (borrowed subgroup, None occurrence using payload
revision 3). Existing tag 26 (Math, payload revision 4) remains separate from
occurrence encoding. No KIR work is included in the canonical Math correction.
Existing accepted partition payload bytes are not relabelled.
