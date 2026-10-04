# Production semantic CPU conformance V3 and V4

The V3 conformance path tests the semantic simulator through the same
authority-free artifact boundary used by production debugging:

```text
ordinary #[kernel(typed)] Rust
  -> production semantic MIR
  -> production KIR V8/V9
  -> exact same-module KIR V10
  -> Bundle V5 strict admission
  -> deterministic CPU simulation
  -> exact byte and initialization comparison
```

The runner never compiles source and accepts no loose KIR, request identity,
target, or limit substitution. `AdmittedSimulationBundleInputV5` owns the
strictly captured bundle/request pair. Before execution, V3 rechecks the bundle
content and subject identities, source-lineage receipt identities and lengths,
kernel ABI, and KIR V10 digest and length. The report repeats the exact bundle,
subject, KIR, and request identities used for the observation.

The additive V4 boundary is the current route:

```text
ordinary #[kernel(typed)] Rust
  -> production semantic MIR and mixed SSA/memory lowering
  -> exact canonical KIR V11
  -> Bundle V6 strict admission
  -> deterministic CPU simulation
  -> exact byte and initialization comparison
```

`run_production_semantic_conformance_v4` accepts only an
`AdmittedSimulationBundleInputV6`. It revalidates the complete Bundle V6,
content and subject identities, source lineage, ABI, and exact KIR V11 identity
before execution. KIR V11 includes the explicit same-pointee,
same-address-space `ReadWrite`-to-`ReadOnly` pointer restriction used by
retained shared borrows; that operation preserves pointer provenance but does
not prove Rust aliasing or lifetime rules. The V3 Bundle V5/KIR V10 API above
remains a frozen compatibility route.

The ignored compiler integration suite is the production qualification entry:

```text
cargo test -p rustc-codegen-fe2o3 \
  --test production_semantic_conformance_v3 -- \
  --ignored --test-threads=1
```

It uses deterministic generated integer cases for every frontend-retained
signed and unsigned width through 64 bits. The signed and unsigned fixtures use
the same high-bit inputs so their CPU oracle exercises signed predicate
semantics, not only storage width. Separate f32/f64 tables cover subnormals,
round-to-nearest-ties-to-even, infinities, NaN quieting/payload retention, and
signed zero. A checked `DisjointSlice` output case covers the admitted dynamic
bounds path and exact scalar/buffer layout.

Failure tests reject duplicate and oversized expectations, wrong scalar types,
missing output ordinals, corrupt bundle bytes, and requests naming a kernel
from another artifact. They also preserve typed unavailability for current
ordinary producer gaps: `i128`/`u128`, f16/bf16, core atomic RMW projection,
pointer distance, volatile access, copy-nonoverlap, and recursive aggregate
Bundle V5 input. Ordinary `u32` switch lowering is covered as an exact
production conformance case.

The same ignored integration target additionally compiles nested loops,
loop-carried locals, and a multiway `match` through the production mixed-SSA
pipeline, admits the resulting Bundle V6, and compares exact output across all
64 lanes. General Rust reference helpers still require provenance-keyed
address-space specialization before a private retained borrow can cross a
helper ABI; this conformance boundary does not claim that support, all-Rust
lowering, or formal verification.

This evidence is CPU simulation only. It makes no performance prediction and
does not authenticate compiler execution, a GPU result, or KFD load/launch.
