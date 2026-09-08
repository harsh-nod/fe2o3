# Dynamic strided GEMM

This M4 example is one target-neutral, attributed Rust kernel for

```text
C = alpha * A * B + beta * C
```

`M`, `N`, `K`, `lda`, `ldb`, `ldc`, `alpha`, and `beta` are runtime values.
Each 64-invocation workgroup owns one 16x16 output tile. Checked matrix loads
zero-fill M/N/K tails, and checked disjoint stores leave output padding
unchanged. The numerical contract requires exact BF16-to-FP32 widening,
depth-ordered FP32 products and additions, positive-zero K tails, and separate
FP32 alpha/beta products before the final addition. Simulation and hardware
qualification therefore require bitwise agreement with the Rust oracle.

The generic kernel contains no processor or backend schedule selection. Exact
target binding occurs in separate runners:

```bash
./run-gfx942.sh
./run-gfx950.sh
```

Set `FE2O3_EXAMPLE_COMPILE_ONLY=1` to stop after a compiler-produced Bundle V8
has passed the local KIR V13 contract, hostile-mutation, and deterministic
simulation gates and the separately extracted LLVM has been linked into
target-matched HSACO. Bundle and LLVM extraction must carry the same
compiler-issued crate binding. Failed runs leave no persistent Bundle or HSACO,
and temporary build artifacts are removed on shell exit.

A genuine compiler bundle can also exercise the deterministic simulator:

```bash
FE2O3_M4_BUNDLE_V8=target/fe2o3-gfx950/tiled_gemm_general_v1.bundle-v8 \
FE2O3_M4_EXPECTED_TARGET=gfx950:xnack- \
cargo test --features bundle-v8-simulator --test production_v13 \
  bundle_v8_matches_the_deterministic_cpu_oracle \
  -- --ignored --exact
```

## Current production boundary

M4 is not production-complete. Four shared APIs still prevent the ordinary
kernel from using the complete compiler-issued hierarchy end to end:

- matrix views accept raw slices rather than branded read-only `Global` views;
- `#[kernel(typed)]` cannot bind an exclusive/disjoint read-modify-write
  `Global` needed by `beta * C`;
- runtime K loops cannot carry the changing `WorkgroupCapability` epoch type;
- the source matrix API cannot bind an exact numerical-policy capability.

The source uses real `KernelContext` invocation, private-memory, and subgroup
APIs. Its deprecated context-branded matrix and dynamic LDS pipeline calls are
the two remaining compatibility terminals; compile-fail boundary fixtures pin
their missing typed replacements. Source/UI/reference tests are not production
receipts. The Bundle V8 gate rejects missing layout roles, K-phase control
flow, barrier families, resource limits, launch shape, target binding, or
strict numerical requirements, but cannot run until shared W6 emits the exact
finalized graph.

The library forbids unsafe code. The host runner's external-HSACO load and
launch remain an explicitly unsafe qualification path and grant no protected
publication, load, or launch authority.
