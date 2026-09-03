# Target-neutral workgroup scan V1

`fe2o3_device::WorkgroupCollectives` exposes two compiler terminals for
ordinary attributed Rust kernels:

- `inclusive_scan_sum` returns the sum through the caller's linear work-item rank.
- `exclusive_scan_sum` returns positive zero at rank zero and the sum through
  the preceding rank elsewhere.

Both calls consume one compiler-issued
`DynamicLds<T, LdsUninitialized>` value. The affine LDS value cannot be reused,
replaced by a raw pointer, or passed with a different element type. The
compiler also requires one exact `[N, 1, 1]` launch contract, with any `N` in
`1..=256`, and uniform workgroup participation. This includes odd extents and
partial final waves such as 3, 65, and 255 lanes.

The sealed element set is `u32`, `i32`, and `f32`. Integer additions use the
existing finite-width KIR semantics. `f32` uses the fixed Hillis-Steele
association and the existing strict scalar-add contract. Exclusive rank zero
uses exact positive `0.0_f32`. Wider integers, `f64`, relaxed/fast floating
point and target-specific numerical substitutions are rejected; the CPU
simulator does not approximate them.

## Compiler contract

Scan is an additive semantic MIR V10 operation. V9 remains closed for the
existing target-neutral reduction. No new KIR operation is needed: production
lowering expands scan into ordinary KIR V8/V10 local-rank, typed LDS,
comparison, select, add, and acquire-release workgroup-barrier operations.
The expansion uses guarded subtraction, so inactive ranks never rely on
unsigned index underflow.

The ranked projection records every generated LDS access and barrier in exact
order. For `N` lanes it records `3 * ceil(log2(N)) + 2` memory effects and
`2 * ceil(log2(N)) + 2` barriers. A domain-separated recipe identity binds the
semantic function, producer/consumer blocks, exact LDS/storage/element types,
extent, scalar type, and inclusive/exclusive mode. The translation gate then
replays the full KIR expansion, including index guards, pointer bases,
arithmetic operand order, positive-zero identity, input/output SSA custody,
and the absence of deleted, duplicated, reordered, or trailing operations.
Reduction recipe identities retain their prior V1 encoding.

## CPU and debugger evidence

The target-neutral KIR is executable without a GPU. Focused tests run 3, 65,
and 255 lanes in both scan modes for every supported scalar type under
canonical and seeded cooperative schedules, then replay the exact seeded
record. The output bits must match the corresponding prefix oracle in every
mode.

Debugger records expose each logical invocation's global, workgroup, and local
coordinates; exact KIR function/block/operation sites; typed LDS reads and
writes; barrier arrival/release phases and participant counts; and the seeded
schedule identity plus decision ordinal. A changed input cannot reuse a
recorded schedule because the replay binding includes the exact request.
Wrong workgroup geometry fails at preflight, and divergent participants return
the typed workgroup-barrier diagnostic.

The ordinary source examples also pass through the production Bundle V5 path,
not a handwritten simulator copy. The compiler derives each kernel's semantic
ABI/storage correspondence, emits production KIR V8 plus exact same-module KIR
V10, and content-binds the debug source map. One bounded gate exports all 18
feature-isolated entries across the six type/mode families, executes every
result row through direct CPU simulation and the runtime backend, records a
complete Trace V2, and opens the debugger's retained workgroup, wave, and lane
views. The 65-lane cases also inspect lane 64 in their one-lane final Wave64.
The 255-lane debugger transcript reaches its published hard retention ceiling:
the protocol must report the inexact `resource_exhaustion` stop and still
permits inspection of the bounded retained prefix. Smaller examples complete
their debugger transcripts exactly.

Run the 3-lane ordinary source example without GPU access:

```bash
./scripts/quickstart.sh simulate-source \
  --crate fe2o3_workgroup_sync_v1 \
  --request examples/workgroup_sync_v1/scan-u32-request.json \
  --bundle-version 5 \
  --output /tmp/scan-u32.fe2sim \
  -- --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --no-default-features --features lds-scan-u32-kernel --lib
```

Bundle V1 remains the quickstart default for compatibility. Select Bundle V5
when the consumer needs compiler-derived semantic MIR, storage correspondence,
and source-map members in addition to executable KIR.

This is deterministic CPU execution and compiler evidence. It is not GPU
execution, hardware validation, performance prediction, or proof that all
possible schedules were explored.

## Ordinary Rust examples

- [`kernel_scan_u32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_u32.rs)
  computes 3-, 65-, and 255-lane inclusive `u32` scans.
- [`kernel_scan_u32_exclusive.rs`](../examples/workgroup_sync_v1/src/kernel_scan_u32_exclusive.rs)
  computes 3-, 65-, and 255-lane exclusive `u32` scans.
- [`kernel_scan_i32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_i32.rs)
  computes 3-, 65-, and 255-lane exclusive `i32` scans.
- [`kernel_scan_i32_inclusive.rs`](../examples/workgroup_sync_v1/src/kernel_scan_i32_inclusive.rs)
  computes 3-, 65-, and 255-lane inclusive `i32` scans.
- [`kernel_scan_f32.rs`](../examples/workgroup_sync_v1/src/kernel_scan_f32.rs)
  computes 3-, 65-, and 255-lane inclusive `f32` scans.
- [`kernel_scan_f32_exclusive.rs`](../examples/workgroup_sync_v1/src/kernel_scan_f32_exclusive.rs)
  computes 3-, 65-, and 255-lane exclusive `f32` scans.

The existing target driver compiles one compatibility entry from each of the
six source families through semantic MIR, ranked PLIRON, verified KIR, and both
gfx942/gfx950 LLVM bindings. The Bundle V5 CPU driver separately qualifies all
18 feature-isolated entries through gfx942 source export, simulation, runtime,
Trace V2, and bounded debugger inspection. Neither grants artifact, load,
launch, hardware, or performance authority.
