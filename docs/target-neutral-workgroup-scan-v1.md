# Target-neutral workgroup scan V1

`fe2o3_device::WorkgroupCollectives` exposes two compiler terminals for
ordinary attributed Rust kernels:

- `inclusive_scan_sum` returns the sum through the caller's linear work-item rank.
- `exclusive_scan_sum` returns positive zero at rank zero and the sum through
  the preceding rank elsewhere.

Both calls consume one compiler-issued
`DynamicLds<T, LdsUninitialized>` value. The affine LDS value cannot be reused,
replaced by a raw pointer, or passed with a different element type. The
compiler also requires one exact `[N, 1, 1]` launch contract, with a
power-of-two `N` in `1..=256`, and uniform workgroup participation.

The sealed element set is `u32`, `i32`, and `f32`. Integer additions use the
existing finite-width KIR semantics. `f32` uses the fixed Hillis-Steele
association and the existing strict scalar-add contract. Exclusive rank zero
uses exact positive `0.0_f32`. Wider integers, `f64`, relaxed/fast floating
point, non-power-of-two groups, and target-specific numerical substitutions
are rejected; the CPU simulator does not approximate them.

## Compiler contract

Scan is an additive semantic MIR V10 operation. V9 remains closed for the
existing target-neutral reduction. No new KIR operation is needed: production
lowering expands scan into ordinary KIR V8/V10 local-rank, typed LDS,
comparison, select, add, and acquire-release workgroup-barrier operations.
The expansion uses guarded subtraction, so inactive ranks never rely on
unsigned index underflow.

The ranked projection records every generated LDS access and barrier in exact
order. For `N` lanes it records `3 * log2(N) + 2` memory effects and
`2 * log2(N) + 2` barriers. A domain-separated recipe identity binds the
semantic function, producer/consumer blocks, exact LDS/storage/element types,
extent, scalar type, and inclusive/exclusive mode. The translation gate then
replays the full KIR expansion, including index guards, pointer bases,
arithmetic operand order, positive-zero identity, input/output SSA custody,
and the absence of deleted, duplicated, reordered, or trailing operations.
Reduction recipe identities retain their prior V1 encoding.

## CPU and debugger evidence

The target-neutral KIR is executable without a GPU. Focused tests run both
scan modes for every supported scalar type under canonical and seeded
cooperative schedules, then replay the exact seeded record. The output bits
must match the corresponding prefix oracle in every mode.

Debugger records expose each logical invocation's global, workgroup, and local
coordinates; exact KIR function/block/operation sites; typed LDS reads and
writes; barrier arrival/release phases and participant counts; and the seeded
schedule identity plus decision ordinal. A changed input cannot reuse a
recorded schedule because the replay binding includes the exact request.
Wrong workgroup geometry fails at preflight, and divergent participants return
the typed workgroup-barrier diagnostic.

This is deterministic CPU execution and compiler evidence. It is not GPU
execution, hardware validation, performance prediction, or proof that all
possible schedules were explored.

## Ordinary Rust examples

- [`kernel_scan_u32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_u32.rs)
  computes an inclusive `u32` scan.
- [`kernel_scan_i32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_i32.rs)
  computes an exclusive `i32` scan.
- [`kernel_scan_f32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_f32.rs)
  computes an inclusive `f32` scan.

The ignored production driver compiles all three sources through semantic MIR,
ranked PLIRON, verified KIR, and both gfx942/gfx950 LLVM bindings. It checks
compiler output only and grants no artifact, load, launch, hardware, or
performance authority.
