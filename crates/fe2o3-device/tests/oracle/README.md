# gfx942 FNUZ oracle

`fp8_gfx942_oracle.hip` records the native `gfx942` behavior used by the
`fe2o3-device` FNUZ value contract. It runs conversions and packed constructors
on the GPU; ROCm's host fallback is not treated as an oracle.

The checked golden records:

- the exact gfx942, ROCm, HIP, and AMD Clang identities;
- all 256 E4M3-FNUZ and all 256 E5M2-FNUZ widening results;
- narrowing of every widened encoding;
- both signs of every midpoint and the adjacent `f32` values around it;
- zero, infinity, NaN payload, f32 extent, overflow, and deep-underflow cases;
- packed x4 lane order, special values, and one-lane mutations; and
- SHA-256 identities of the oracle and generator.

From the repository root on a machine with a visible gfx942 GPU:

```text
scripts/fp8-gfx942-oracle.sh --check
scripts/fp8-gfx942-oracle.sh --stdout
scripts/fp8-gfx942-oracle.sh --write
```

`--check` recompiles the committed oracle, runs it, and compares the complete
output and toolchain metadata with the checked golden. `--write` is reserved
for an intentional contract update followed by review of the textual diff.
The script locates the repository and ROCm tools dynamically; generated data
contains no filesystem paths. Generic CI does not need ROCm or a GPU because
the Rust integration test consumes the checked golden directly.
