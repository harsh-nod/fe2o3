# dialect-kernel

`dialect-kernel` owns target-neutral Pliron kernel semantics. Alongside its
structured-algorithm root, it defines a bounded ranked-memory vocabulary:

- `kernel.ranked_view<width, writable, shape>` represents rank 1 through 8;
  zero shape entries are runtime dimensions and nonzero entries are static.
- `kernel.index_constant` and `kernel.dim` produce unsigned index values.
- `kernel.access` describes a read or write with exactly one index per
  dimension. Atomic forms additionally retain explicit ordering and scope.
- `kernel.index_lt_br`, `kernel.br`, and `kernel.return` form the closed CFG
  vocabulary used by target-neutral safety analysis.
- `kernel.require_finite_fold`, `kernel.require_finite_recurrence`, and
  `kernel.require_permutation_gather` retain bounded workload-neutral semantic
  contracts. Each names its finite domain, step bound, evaluation order,
  exact numerical policy, typed expression witnesses, and required output
  ownership theorem.

Every type and operation has an MLIR-style local `Verify` implementation.
Local verification rejects malformed ranks, types, operand counts, dynamic
extent bindings, dimension selectors, CFG payloads, and writes through
read-only views. The whole-function `kernel-memory-bounds-v1` stage lives in
`fe2o3-kernel-analysis`: it proves static bounds directly and intersects
`index < extent` facts across all incoming control-flow paths for dynamic
shapes. An unproved relation is a terminal pre-lowering compile-time error with
the exact view, dimension, index, and extent in the diagnostic.

The whole-function `kernel-atomic-legality-v1` stage rejects missing or invalid
atomic ordering/scope contracts before race analysis may treat atomic effects
as compatible. A valid contract is still incomplete without a matching bounded
target-capability context, and system scope additionally requires authenticated
coherent-allocation provenance.
The current aggregate read-modify-write effect does not encode
compare-exchange failure ordering; source projection must leave
compare-exchange incomplete until that exact operation contract is represented.

The vocabulary and analysis do not contain GEMM names, tiles, schedules, or
target details. The same pass covers vectors, images, tensors, volumes, and
future fixed-contract kernels.

Collective coverage and value semantics remain separate theorems. In
particular, `CollectiveContributions` proves exactly-once atomic participation,
not the reduction operator, order, identity, or final value. The semantic
refinement stage requires a separately proved MIR equality before any finite
collective contract is clean.

The shell does not lower operations, choose schedules or compilers, describe a
hardware target, produce artifacts, or grant proof, publication, load, tuning,
or launch authority. A clean bounds report is a descriptive rejection-gate
result, not source correspondence or runtime allocation authentication. Its
Pliron values and printed syntax are not durable fe2o3 identities.

Its production registration adapter depends only on
`fe2o3-pliron-owner-core`; ownership of the full Pliron session remains in
`fe2o3-pliron`.
