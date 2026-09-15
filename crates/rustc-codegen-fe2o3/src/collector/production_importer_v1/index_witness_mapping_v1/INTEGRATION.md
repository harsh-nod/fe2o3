# Scoped Workgroup Index Checkpoint

## Status

WG production and test hooks are MOUNTED after the parent lifted the freeze.
The complete `patches/01` through `patches/07` batch was applied against current
file contents. No pipeline or ranked-projector changes are included. Shared
dirty files were not rewritten from snapshots.

Completed locally: the actual registered source fixture compiled with pinned
nightly-2026-04-03 rustc, exporter flags including debuginfo2, complete cached
AMD device/core metadata, repo cwd, private output, and exact derived crate
binding/build observation. Draft candidates passed exact-context application
and in-memory rustfmt syntax checks. After mounting, pinned rustfmt parsed the
actual changed files and children without rewriting them; scoped git diff
whitespace checks passed. These are NOT callback, import, ownership,
lowering, execution, or refinement test results. Actual rowsoft acceptance and
all mounted central tests remain pending. No Cargo, SSH, network, protected
runtime receipt, or launch was performed by this worker.

## Exact Operations

```text
WorkgroupMemoryIndexV2 { workgroup_reference, workgroup, option, witness }
WorkgroupMemoryIndexIntoDisjoint { input_witness, output_witness }
```

V2 issuance distinguishes source shared reference, owned pointee, Option return,
and ThreadIndex payload. All four identities are distinct. MIR checks the exact
shared-borrow ABI, pointee, Option payload, and transparent unsigned64 witness.
KIR returns the payload, not the Option. Existing guarded Option handling stays.

Conversion consumes one existing witness by value. MIR checks the same raw
unsigned64 representation and distinct witness identities. KIR requires exact
source type, root provenance, workgroup brand, epoch, and role. Its result changes
only nominal witness type. There is no memory effect, epoch transition, rank
reissuance or global disjointness assertion. Safety obligations remain explicit.

- MIR V22, outer ExecutionCapability tag73: inner26 for V2, inner27 for conversion.
- KIR execution: inner27 for V2, inner28 for conversion. Inert envelopes retain
  revision1; actual source occurrences use existing revision5.
- Legacy producer tag18, fields, bytes and meaning are unchanged.
- Lagrange outer intrinsic tag80 and Ram KIR execution tag25 are unchanged.

Issuance implements flattened workgroup rank `(z * size_y + y) * size_x + x`,
not localX alone. Physical and symbolic consumers require nonzero static
geometry with product fitting unsigned64. Conversion copies the existing rank
without invocation intrinsics. Physical copies keep original SSA result IDs.

## Importer and Custody

`index_witness_contract_v1` classifies only authenticated ThreadIndex and
DisjointIndex types into the existing invocation family or exact trusted
WorkgroupMemoryIndexSpace1D plus parsed WorkgroupMemoryBrand. It introduces no
global mapping variant. `thread_into_disjoint_contract_v1` requires identical
instantiated Rust Space and Brand and correct input/output witness kinds.

`conversion_requires_root_v1` selects original caller/root authentication by
those types, not diagnostic names. `validate_conversion_carriage_v1` reconstructs
the complete operation from original instance, source identity, ABI and actual
root and compares it with the canonical callable. Source joins grant no authority.

Separately, `disjoint_slice_get_mut_contract_v1` requires Some/Some legacy
classifications followed by equality before layout projection. Both-None cannot
pass. DisjointSlice has no receiver Brand parameter; none is invented here.

`BorrowedWorkgroupPlanV1::index_receiver` uses the existing replay-checked loan
resolver and checks actual call/signature, expansion/root owner, live shared
loan, exact Workgroup issuer/provenance and receiver SSA value.
`WorkgroupIndexTransportV1` transports an existing typed witness. Restore and
transport check exact expected KIR types, including root, issuance, target,
launch, brand and epoch. Raw scalars cannot become capabilities.

Option/Result uses existing unique dominating source custody for scoped payloads.
The bounded structural classifier only selects storage treatment. Exact known
variant payloads and existing definition/dominance/move/owner checks remain
mandatory. No arbitrary enum authority or private scalar reconstruction is added.

Ranked projection does not create global PreserveMapping edges for these ops.
Original expanded call occurrences and checked execution-source carriage retain
them through the ordinary ranked roster path, exercised by the new full-lowering
callback. No receipt is manufactured and no projector exemption is added.

After original KIR verification, simulator/AMD block parameters lower only the
WorkgroupMemoryIndex role to INDEX, preserving edge IDs. Other capability block
parameters and function capability ABIs retain their existing rejection rules.
Final symbolic conversion accepts WorkgroupIndex only, never a plain scalar.

## Applied Mount Order

Dependencies: Lagrange shared V22 infrastructure and Ram borrowed-workgroup /
source-occurrence infrastructure. Both were present at the last draft review.
The complete patch set below has been applied. Do not reapply the archived
drafts. Central compile/test is the next gate:

1. `01-mir.patch`: enum, ABI, obligations, references, V22 codec and tests.
2. `02-kir.patch`: enum/codec, signatures, exact verification and tests.
3. `03-importer.patch`: classification, original-root join, V2 producer,
   conversion, Some/Some guard and exact carriage.
4. `04-lowering.patch`: KIR projection, payload correction and checked receiver.
5. `05-consumers.patch`: simulation, AMD, target requirements, uniformity, symbols.
6. `06-ssa-transport.patch`: existing typed witness and exact enum custody.
7. `07-kir-catalog.patch`: existing catalog and exhaustive wire-header tests.

Patch03 added `tcx` to the root resolver and all current callers, including a
one-line signature adaptation in Lagrange's `global_bf16_matrix_v1::validate_carriage`.
No nominal checks in that child were changed. `send_input` was unavailable in
this worker session, so the exact coordination message was sent through the
parent. Patch04 extends shared resolver dispatch without replacing files.
`FILES.md` lists exact new children and mounted shared-file targets.

## Central Test Hooks

Compiler filter:
`collector::production_importer_v1::index_witness_mapping_v1::compiler_tests`.
Ignored callbacks use `FE2O3_CORE_TRY_DEVICE_RMETA`, `FE2O3_CORE_TRY_HOST_DEPS`,
`FE2O3_CORE_TRY_AMDGPU_CORE`, `FE2O3_CORE_TRY_AMDGPU_BUILTINS`. They require
complete metadata and build no dependencies, using repo cwd/private output and
the exact metadata-derived observation/registration.

Classifier cases exercise genuine rowsoft conversion/store/publish calls,
unchanged global/shifted mappings, wrong Space/Brand/kernel/epoch/witness kind,
and matching/mismatching/unsupported DisjointSlice calls. Registered cases:
`actual_registered_workgroup_index_import_v22_amdgpu` and
`actual_registered_workgroup_index_ranked_lowering_amdgpu`.
They check actual source ABI/root reconstruction, missing/wrong roots,
Option-as-payload rejection, ordinary SSA/ranked ownership, source occurrences,
and KIR scope/result substitutions. They have not run centrally yet.

Library filters: `workgroup_memory_index` (MIR/KIR/sim/AMD/verifier),
`workgroup_index_transport_tests` (lowerer), plus existing KIR execution catalog.
Coverage includes codec/version identity, exact custody, wrong root/target/launch/
epoch/brand/operand, flat 1-D/3-D rank, conversion without reissuance, phi transport
and invalid geometry. Inert tests do not substitute for real import/lowering.
Any downstream memory or proof-runtime blocker must remain explicit/fail-closed.

## Queued SSA Triage

After this batch: mixed20 MoEtop2, original function20 bb19 statement0 local106,
unsupported projected move. The parent suspects a dynamic Index destination
guard despite initialized owned array; exact source diagnostics are still
needed before attribution. Recompute-prefix reports 2,097,158 versus unchanged
2,097,152 limit. Do not infer required headroom from the first rejected allocation.
Partial-move state files remain frozen and are not part of this WG batch.
