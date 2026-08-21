# dialect-kernel

`dialect-kernel` owns target-neutral Pliron kernel semantics. Alongside its
structured-algorithm root, it defines a bounded ranked-memory vocabulary:

- `kernel.ranked_view<width, writable, shape>` represents rank 1 through 8;
  zero shape entries are runtime dimensions and nonzero entries are static.
- `kernel.index_constant` and `kernel.dim` produce unsigned index values.
- `kernel.access` describes a read or write with exactly one index per
  dimension.
- `kernel.index_lt_br`, `kernel.br`, and `kernel.return` form the closed CFG
  vocabulary used by target-neutral safety analysis.

Every type and operation has an MLIR-style local `Verify` implementation.
Local verification rejects malformed ranks, types, operand counts, dynamic
extent bindings, dimension selectors, CFG payloads, and writes through
read-only views. The whole-function `kernel-memory-bounds-v1` stage lives in
`fe2o3-kernel-analysis`: it proves static bounds directly and intersects
`index < extent` facts across all incoming control-flow paths for dynamic
shapes. An unproved relation is a terminal pre-lowering compile-time error with
the exact view, dimension, index, and extent in the diagnostic.

The vocabulary and analysis do not contain GEMM names, tiles, schedules, or
target details. The same pass covers vectors, images, tensors, volumes, and
future fixed-contract kernels.

The shell does not lower operations, choose schedules or compilers, describe a
hardware target, produce artifacts, or grant proof, publication, load, tuning,
or launch authority. A clean bounds report is a descriptive rejection-gate
result, not source correspondence or runtime allocation authentication. Its
Pliron values and printed syntax are not durable fe2o3 identities.

Its production registration adapter depends only on
`fe2o3-pliron-owner-core`; ownership of the full Pliron session remains in
`fe2o3-pliron`.
