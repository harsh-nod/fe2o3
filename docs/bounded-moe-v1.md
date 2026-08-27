# Bounded MoE V1 evidence

This document retains one narrow example proof around the fixed
`T8/E4/K2/C4` top-2 router and expert schedule: an inert Verus proof of
one exact expert compact-plan arithmetic model. The former private rustc
classifier and workload-specific host routes have been removed.

The retained proof does not compose into source-to-machine refinement,
authenticated router provenance, artifact authority, dispatch authority, or
expert GPU execution.

## Exact compact-plan proof

The compact-plan proof covers only `E4/C4/routes16/width16/tile256`. Given
zero-based monotone expert offsets whose four counts are each at most four, it
proves source ranges remain inside their expert tiles, destination ranges remain
inside the compact tile, nonempty destination ranges are pairwise disjoint and
ordered, their union is exactly the accepted prefix, and every unused tail
element is zero.

The pinned example runner requires `19` verified Verus obligations and rejects
exactly seven named negative mutations. The expected proof values are
copyable inert pins. They are not joined to the compiler pipeline, host code, runtime copies, machine addresses, an authenticated proof receipt, or a
GPU artifact.

## Retired host routes

The former MoE V1/V2 host bridges, generated adapters, denial token, exact
top-2 lifecycle, and workload-specific HSA launcher were qualification
alternatives. They never granted production artifact, load, launch, or
execution authority and duplicated the generic Worker V3 application path.
They have been removed.

The ordinary Rust MoE kernels, standalone compact-plan Verus model, obligations,
negative mutations, and independent source/oracle tests remain. They are
compiler and proof evidence, not a second runtime pipeline.

Any executable MoE integration must now publish a normal Worker V3 descriptor
and use the same application handoff, descriptor recovery, HSA
load/resolve/dispatch/unload lifecycle, generated argument packing, alias
admission, physical-resource validation, and dynamic-LDS handling as every
other kernel. Workload-specific host lifecycle or HSA adapter APIs must not be
reintroduced.

## Reproduce the retained checks

Run from the repository root with the pinned Rust toolchain:

```sh
python3 scripts/test-bounded-moe-docs.py
VERUS=/absolute/path/to/pinned/verus \
  ./scripts/test-moe-expert-compact-plan-verus.sh
cargo test --locked -p fe2o3-host \
  --test production_application_handoff_ui
```

These checks establish only the retained source, proof, and
production-route-absence claims above. No MoE hardware execution or
source-to-machine refinement claim follows from them.
