# General tiled GEMM protected hardware harness

This standalone package contains the deterministic, guarded MI300X case matrix
and independent CPU-oracle comparison for the issue #138 general tiled GEMM.
It covers the reference and A-only-vectorized schedules independently.

There is intentionally no executable entry point yet. The public protected
authority and argument types have private fields and no constructors. They
record the exact future join: compiler binding, schedule proof, artifact,
publication, application, observed `gfx942:xnack-` device, runtime, eleven-slot
ABI, 64x1x1 workgroup, 2D tiled grid, and 1024-byte LDS allocation. No raw
HSACO, path, native handle, generic launch, or caller-supplied authority bridge
is available.

Run the independent preparation and oracle checks with:

```text
cargo test --manifest-path examples/tiled_gemm_general_v1/hardware-harness/Cargo.toml
```

Passing these tests is not GPU execution evidence. Protected hardware evidence
remains blocked until the compiler emits an exact source-bound artifact and the
runtime consumes the verifier-owned final admission.
