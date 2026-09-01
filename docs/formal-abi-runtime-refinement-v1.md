# Formal ABI/runtime refinement V1

## Boundary

V1 covers one production-used profile: the generated Worker V3
`vecadd(&[f32], &[f32], DisjointSlice<f32>)` COV6 preparation path. It joins
four independently admitted inputs before returning a typed prepared
invocation:

- the canonical descriptor and generated host packing plan;
- physical kernel metadata and launch/resource limits;
- verifier-bound kernel, generated-host, Rust layout, and Rust effect
  identities; and
- capability-derived allocation regions, effects, alias admission, and launch
  geometry.

The executable admission is
`fe2o3_runtime_model::admit_formal_vecadd_runtime_preparation_v1`. The exact
Verus theorem is `vecadd_runtime_preparation_refines_v1` in
`crates/fe2o3-runtime-model/verus/formal_abi_runtime_v1.rs`.

## Established relation

For an admitted input, the theorem establishes:

- six 8-byte fat-slice components at offsets `0, 8, 16, 24, 32, 40`;
- a 48-byte explicit prefix, 256-byte COV6 suffix, 304-byte physical kernarg,
  descriptor alignment 8, and HSA runtime alignment 16;
- exact 256x1x1 workgroups and `ceil(output_len / 256)` workgroup counts within
  the retained source and per-axis physical limits, with zero dynamic group and
  private-segment requirements;
- exact kernel/host/layout/effect identities and ordered read/read/write
  argument effects;
- nonempty output bounded by both inputs, checked four-byte element extents,
  and no overlap between a writable output region and either input region; and
- the ownership transition from `Loaded` with caller-held resources to
  `Prepared` with all three regions retained by the one-shot invocation.

The host stores the non-cloneable evidence in
`GeneratedWorkerV3PreparedInvocationV1` beside the arguments, alias admission,
and in-flight registration. Non-vecadd kernels keep the prior checks and carry
no V1 formal-profile evidence.

## Explicit exclusions and TCB

The theorem ends at runtime preparation. It does not establish LLVM lowering,
optimization, code generation, linker correctness, code-object semantics,
driver or HSA behavior, AQL publication, firmware execution, GPU memory-model
facts, completion truth, functional output, liveness, or performance.

The remaining V1 contracts/TCB are:

- the pinned Verus `0.2026.08.09.92f466f` release closure and Z3;
- correspondence between the executable Rust validator/projection and the
  formal relation;
- the unsafe compiler-generated argument implementation and reviewed HSA
  adapter contracts;
- truth of authenticated descriptor, physical metadata, verifier identities,
  allocation provenance, currentness, and alias observations; and
- all LLVM, linker, runtime, kernel-driver, firmware, and device execution.

The evidence API reports these exclusions directly and grants no load, launch,
completion, or proof authority.
