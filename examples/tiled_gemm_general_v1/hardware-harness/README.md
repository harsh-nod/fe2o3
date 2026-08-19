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

The only constructible execution entry point is explicitly unsafe. Its safety
contract requires rustc-codegen to retain the two non-Clone final
qualifications, one per schedule, throughout the call and to borrow every
structural input and finalized byte directly from those same owners. The
executor privately constructs a transient per-case authority, binds each
symbolic artifact to a checked concrete launch-time plan, canonical KIR, and
runtime ABI snapshot, and consumes that authority during synchronous dispatch.
It returns typed observations but no publication, load, or replay authority.

Run the independent preparation and oracle checks with:

```text
cargo test --manifest-path examples/tiled_gemm_general_v1/hardware-harness/Cargo.toml
```

Passing these tests is not GPU execution evidence. A real run additionally
requires finalized artifacts and the live rustc-owned three-party
qualifications. The full 14-case matrix must be invoked through that unsafe
boundary on MI300X.
