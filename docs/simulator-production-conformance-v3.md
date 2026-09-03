# Production semantic CPU conformance V3

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
ordinary producer gaps: `i128`/`u128`, f16/bf16, switch fallback traps, core
atomic RMW projection, pointer distance, volatile access, copy-nonoverlap, and
recursive aggregate Bundle V5 input.

This evidence is CPU simulation only. It makes no performance prediction and
does not authenticate compiler execution, a GPU result, or KFD load/launch.
