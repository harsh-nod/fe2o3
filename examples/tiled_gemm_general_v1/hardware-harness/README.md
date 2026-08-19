# General tiled GEMM protected hardware harness

This standalone package contains the deterministic, guarded MI300X case matrix
and independent CPU-oracle comparison for the issue #138 general tiled GEMM.
It covers the reference and A-only-vectorized schedules independently.

The package now contains a generated, checked HSA dispatch adapter for the
11-logical/14-physical ABI. It packs the exact 80-byte explicit prefix, retains
guarded A/B/C allocations through synchronous completion, initializes the
256-byte COV6 implicit suffix through the reviewed HSA adapter, and checks the
executable, kernel, geometry, ABI, initialization, and dispatch observations.
The adapter accepts no raw HSACO, path, native handle, generic launch, packed
byte, or caller-supplied authority bridge.

There is intentionally no constructible execution entry point at this
checkpoint. The protected authority has private fields and no constructor. It
must eventually come from a same-process rustc-codegen join that consumes the
opaque frontend correspondence, verifier proof/numerical evidence, and
finalizer machine inspection, then binds the symbolic artifact to a checked
concrete launch-time plan, canonical KIR, and runtime ABI snapshot. Argument
construction compares the exact `GeneralGemmSymbolicCompilationUnitV1` and
`GeneralGemmCheckedLaunchInstantiationV1` accessors against that unavailable
authority before retaining any HSA allocation. The legacy concrete compiler
unit is model evidence, not that production artifact authority.

Run the independent preparation and oracle checks with:

```text
cargo test --manifest-path examples/tiled_gemm_general_v1/hardware-harness/Cargo.toml
```

Passing these tests is not GPU execution evidence. Protected hardware evidence
remains blocked on the symbolic lowering/finalization route and the one-shot
rustc-owned three-party join. The full 14-case matrix has not run on MI300X.
