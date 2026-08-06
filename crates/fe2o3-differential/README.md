# fe2o3-differential

`fe2o3-differential` is standalone infrastructure for reproducible differential
testing. It generates a bounded scalar kernel-expression subset from an exact
seed, provides wrapping `i32` CPU reference semantics, encodes cases in a
canonical V1 format, reports output mismatches, and reduces a case while a
caller-provided mismatch predicate remains true.

The expression subset includes constants, the one-dimensional global ID,
same-index input loads, unary negation and bitwise not, wrapping arithmetic,
bitwise operations, comparisons, and lazy `select` control flow. Cases are
limited to 4 inputs, 256 work-items, 127 expression nodes, 12 levels of depth,
and 16 KiB of canonical bytes.

This crate does not execute a GPU, invoke a compiler, authenticate artifacts,
or establish CUDA Oxide parity. A passing generated corpus is not correctness
or safety evidence. Integration must bind canonical cases to exact compiler,
runtime, device, and artifact identities and must preserve both successful and
failing execution evidence.

The reducer is deterministic for a deterministic predicate. It reaches a local
minimum over its documented expression, launch-shape, input-buffer, and scalar
value transformations; it does not claim a globally minimal reproducer.
