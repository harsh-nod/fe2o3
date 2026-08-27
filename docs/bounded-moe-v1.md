# Bounded MoE V1 evidence

Implementation checkpoint: `1281f97487adfd4af32687b7705ba46e5c11152b`.

This checkpoint adds three narrow pieces around the fixed `T8/E4/K2/C4`
top-2 router and expert schedule. They are deliberately separate evidence
classes:

1. a private same-session rustc structural diagnostic for the router source;
2. an inert Verus proof of one exact expert compact-plan arithmetic model; and
3. a host-observed routing-to-expert consistency and upload bridge.

They do not compose into source-to-machine refinement, authenticated router
provenance, artifact authority, dispatch authority, or expert GPU execution.
The CUDA-Oxide parity status remains `0 Complete / 97 Partial / 0 Missing / 12
N/A` in aggregate.

## Private rustc structural record

The `collected-moe-top2-v1` rustc admission retains the source text already
loaded in rustc's `SourceFile`; it does not reopen the source pathname. After
the parent admission checks source identity, compiler semantics, trusted
definitions, the exact root instance, the rustc-derived `FnAbi`, the complete
imported portable-MIR module, and the selected KIR/profile, it constructs an
opaque `ValidatedMoeTop2AuthorityV1`. The private structural producer consumes
that token by value together with an opaque same-session input witness.

The checked `FnAbi` identity commits the Rust calling-convention discriminator,
variadic flag, fixed and actual argument counts, unwind flag, ignored-return
mode discriminator, return size and ABI alignment, and every argument's layout
size/alignment, pair-mode discriminator, and both components' checked regular
attributes, extension, pointee size, and optional pointee alignment. The
readable diagnostic projection is intentionally smaller than that complete
identity: it shows the checked header, result, and eight pair-mode arguments.

The portable-MIR diagnostic is computed over the full imported module. It binds
the portable semantic identity and records counts for functions, roots,
helpers, blocks, statements, terminators, CFG edges, external imports, root
arguments and locals, assignments, calls, indexed places, repeated values, and
observed binary operators. These are whole-module diagnostics, not a proof that
MIR values or effects simulate the KIR routing state machine.

### Canonical KIR/profile table

The private classifier encodes all current `MoeTop2KernelIrV1` and
`MoeTop2ProfileV1` fields, including nested arrays, descriptors, resources, and
policy records, in these 31 ordered aggregate entries. `KIR`, `Profile`, `ABI`,
`Effects`, and `Routing` name the domain-separated projections that include an
entry. An aggregate entry can encode several leaf fields.

| # | Canonical entry | Projections | Canonical content |
|--:|:--|:--|:--|
| 1 | `kir.module-id` | KIR | module identifier |
| 2 | `kir.function-id` | KIR | function identifier |
| 3 | `kir.kernel-id` | KIR, ABI | kernel identifier |
| 4 | `kir.arguments` | KIR, ABI, Effects | ordered role, shape, scalar, offset, size, and alignment records |
| 5 | `kir.shape` | KIR, Routing | tokens, experts, experts per token, capacity, logits, and routes |
| 6 | `kir.layout` | KIR, Routing | layout discriminator |
| 7 | `kir.finite-input` | KIR, Effects, Routing | finite-input policy discriminator |
| 8 | `kir.tie-break` | KIR, Routing | tie-break discriminator |
| 9 | `kir.overflow` | KIR, Routing | overflow policy discriminator |
| 10 | `kir.routing-steps` | KIR, Routing | ordered routing-step discriminators |
| 11 | `kir.packing` | KIR, Routing | all packing predicates and drop sentinel |
| 12 | `kir.ownership` | KIR, Effects | ownership policy, lanes, output lengths, and write predicates |
| 13 | `profile.source-sha256` | Profile | exact source digest |
| 14 | `profile.namespace` | Profile | source namespace |
| 15 | `profile.target` | Profile | target-capability encoding |
| 16 | `profile.code-object-version` | Profile | profile code-object version |
| 17 | `profile.wave-width` | Profile | wave width in lanes |
| 18 | `profile.workgroup-size` | Profile | required workgroup dimensions |
| 19 | `profile.grid` | Profile | grid dimensions |
| 20 | `profile.correspondence` | Profile | correspondence discriminator |
| 21 | `profile.descriptor.logical-name` | Profile, ABI | logical kernel name |
| 22 | `profile.descriptor.export-name` | Profile, ABI | exported kernel name |
| 23 | `profile.descriptor.symbol` | Profile, ABI | descriptor symbol |
| 24 | `profile.descriptor.code-object-version` | Profile, ABI | descriptor code-object version |
| 25 | `profile.descriptor.explicit-kernarg-bytes` | Profile, ABI | explicit kernarg size |
| 26 | `profile.descriptor.complete-kernarg-bytes` | Profile, ABI | complete kernarg size |
| 27 | `profile.descriptor.workgroup-size` | Profile, ABI | descriptor workgroup dimensions |
| 28 | `profile.descriptor.wave-width` | Profile, ABI | descriptor wave width |
| 29 | `profile.descriptor.static-lds-bytes` | Profile | static LDS size |
| 30 | `profile.descriptor.required-dynamic-lds-bytes` | Profile | required dynamic LDS size |
| 31 | `profile.descriptor.maximum-dynamic-lds-bytes` | Profile | maximum dynamic LDS size |

The classifier rejects name, order, removal, duplication, membership, and value
drift. The final record also binds the rustc-loaded source, complete checked
`FnAbi` identity and projection, full imported-MIR diagnostic, compiler
semantics, trusted definitions, root instance, and completed source authority
from the same admission. The record remains private, diagnostic, and inert. It
is not MIR-to-KIR semantic refinement and grants no Worker V2, LLVM, artifact,
load, launch, runtime, GPU, memory-safety, or race-freedom authority.

## Exact compact-plan proof

The compact-plan proof covers only `E4/C4/routes16/width16/tile256`. Given
zero-based monotone expert offsets whose four counts are each at most four, it
proves source ranges remain inside their expert tiles, destination ranges remain
inside the compact tile, nonempty destination ranges are pairwise disjoint and
ordered, their union is exactly the accepted prefix, and every unused tail
element is zero.

The pinned runner requires `19` verified Verus obligations and rejects exactly
seven named negative mutations. A Rust differential test checks every one of
the `5^4 = 625` valid expert-count vectors. The expected proof values are
copyable inert pins. They are not joined to the rustc structural record, host
code, runtime copies, machine addresses, an authenticated proof receipt, or a
GPU artifact.

## Retired host routes

The former MoE V1/V2 host bridges, generated adapters, denial token, exact
top-2 lifecycle, and workload-specific HSA launcher were qualification
alternatives. They never granted production artifact, load, launch, or
execution authority and duplicated the generic Worker V3 application path.
They have been removed.

The ordinary Rust MoE kernels, rustc collection and structural diagnostics,
canonical KIR/profile data, compact-plan verifier model, Verus obligations,
negative mutations, and independent source/oracle tests remain. They are
compiler and proof evidence, not a second runtime pipeline.

Any executable MoE integration must now publish a normal Worker V3 descriptor
and use the same application handoff, descriptor recovery, HSA
load/resolve/dispatch/unload lifecycle, generated argument packing, alias
admission, physical-resource validation, and dynamic-LDS handling as every
other kernel. Workload-specific host lifecycle or HSA adapter APIs must not be
reintroduced.

Direct Cargo compilation of the standalone example is not an evidence lane:
its typed router dependency requires the per-crate binding issued by the
fe2o3 wrapper. The rustc-codegen integration test below exercises the retained
compiler boundary.

## Reproduce the retained checks

Run from the repository root with the pinned Rust toolchain:

```sh
python3 scripts/test-bounded-moe-docs.py
cargo test --locked -p fe2o3-verifier --test moe_expert_compact_plan_v1
VERUS=/absolute/path/to/pinned/verus \
  ./scripts/test-moe-expert-compact-plan-verus.sh
cargo test --locked -p fe2o3-host \
  --test production_application_handoff_ui
```

These checks establish only the retained source, proof, and production-route-absence claims above. No MoE hardware execution or
source-to-machine refinement claim follows from them.
