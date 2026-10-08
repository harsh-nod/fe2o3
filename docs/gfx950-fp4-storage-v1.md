# OCP E2M1 FP4 representation helpers

This addition supplies a missing low-precision input-preparation helper for
#280 M4. It does not complete M4 or qualify an MFMA kernel.

## Numerical contract

The primary definition is
[OCP Microscaling Formats (MX), version 1.0, section 5.3.3](https://www.opencompute.org/documents/ocp-microscaling-formats-mx-v1-0-spec-final-pdf)
(September 2023, printed pages 12–13). The reviewed PDF is 812,323 bytes,
SHA-256 `d195d6a36dd4a0c89064af0c479bcaad5c0fe29d63f628502ea6d7c4b4279421`.

E2M1 has one sign bit, two exponent bits with bias one and one fraction bit.
Its eight positive magnitudes are 0, 0.5, 1, 1.5, 2, 3, 4 and 6. Both zero
signs exist; 0.5 is subnormal. There are no infinity or NaN encodings.

`Fp4E2M1Ocp::try_from_bits` rejects any set bit outside the low nibble.
`try_from_f32_rne_saturating` names the policy explicitly: nearest-even
rounding, sign-preserving saturation of finite overflow and infinities, and
NaN refusal. Underflow preserves the input sign on zero. NaN conversion is
implementation-defined in the specification; refusal is this helper's chosen
policy, not a claim about AMD hardware conversion. Widening preserves every
element exactly, including the sign of zero.

Conversion inspects f32 bits and at most seven exact midpoint constants. It uses
no floating arithmetic or ambient rounding/denormal mode. No host FMA, MFMA
accumulation, E8M0 block scaling or matrix numerical theorem is implemented.

## Author-facing use

```rust
use fe2o3_device::{Fp4E2M1Ocp, Fp4E2M1Ocpx8, Gfx950Fp4MfmaAMatrix};

let values = [0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 6.0, -6.0]
    .map(|value| Fp4E2M1Ocp::try_from_f32_rne_saturating(value).unwrap());
let packed = Fp4E2M1Ocpx8::from_array(values);
let source_bytes = packed.to_unpacked_bytes();
let _view = Gfx950Fp4MfmaAMatrix::row_major(&source_bytes, 0, 1, 8, 8).unwrap();
```

This constructs a checked host-side view, not a compiler-issued wave token or
an executable kernel. The existing gfx950 matrix view consumes one byte for
each logical element; it does its own hardware-fragment packing later. Passing
the four dense bytes of `packed.to_bits()` where eight logical elements are
required is wrong and is covered by a view-extent negative test.

`Fp4E2M1Ocpx8` packs eight nibbles into a u32 by bit position, lowest indexed
element first, independent of host byte order. An element index outside 0..8
returns None. `try_from_unpacked_bytes` validates all bytes instead of
silently masking upper bits. The storage element index is not a GPU lane.

Scalar and packed equality are numeric: positive and negative zero compare
equal. Use `to_bits()` for exact encoding comparisons. Signed zeros survive
packing and unpacking even though their numeric values compare equal.

## Integration and no_std boundary

The implementation uses only core, fixed-size arrays and integer operations.
There is no allocator, unsafe block, dependency addition, public device marker,
schema allocation, LdsElement implementation, device-ABI admission or source
provider change. The only shared-root delta is a module registration and three
representation exports in fe2o3-device/src/lib.rs.

The helpers support host input preparation and Rust constant evaluation.
Runtime calls inside an authored device kernel still need the ordinary
Rust/MIR admission path. This patch does not guarantee that an arbitrary
helper body is admitted, folded, or replaced by a native FP4 instruction.
The existing authenticated gfx950 source/matrix APIs remain responsible for
their original wave/layout/resource and lowering requirements.

FP4 storage has no architecture capability by itself. The focused target
control reads the existing catalog: gfx950 admits its reviewed FP4 metadata;
gfx942 does not acquire it because this value type exists. This is not a new
target selector or hardware result. Existing gfx942 FNUZ values are unchanged.

## Acceptance status and required qualification

Thirteen focused integration tests are authored for layout and const use,
all sixteen encodings, every invalid upper-byte encoding, all seven rounding
midpoints and adjacent f32 values of both signs, signed-zero/subnormal behavior,
infinities/NaNs, a 4,608-value independent numerical corpus, classification,
packing round trips and bit isolation, existing matrix-view storage shape and
the target capability distinction. The independent reference selects nearest
mathematical values by f64 distance; it does not reuse implementation tables.

CPU qualification on 2026-10-07 used the pinned nightly-2026-04-03 toolchain
on mi350-2. The no_std library check, all 13 focused FP4 tests, all 7 FP8
regressions and the FP4 rustdoc passed. The full fe2o3-device library/integration
suite then passed 179 harness cases (including its UI subcases), and all 32
rustdocs passed. Implementation and test files were rustfmt-formatted.

The full device run used source base `1736eff451d445f1f194abe145af242cf51ec322`
plus the four-file FP4 slice and an independently tracked simulator candidate.
Its retained receipt SHA-256 is
`8a393f1b8e2e1b89304fee1e15a5ddd971da56d2e85084e141283c08c45dfe72`.
The publication base adds only unrelated AMDGCN checked-load experiment files;
all device inputs are unchanged. These are CPU/API results, not native matrix
or production-source qualification. Reproducible focused commands:

```text
cargo check --locked --offline -p fe2o3-device --lib
cargo test --locked --offline -p fe2o3-device --test fp4_api
cargo test --locked --offline -p fe2o3-device --test fp8_api
cargo test --locked --offline -p fe2o3-device --doc fp4
```

These are target shapes, not authorization to reuse expired runners or a
complete tool/target/env/resource binding. The full device suite is recorded above; broader canonical-source
regressions and existing source-provider identity checks remain separate.

The ordinary genuine gfx950 source-to-matrix route already exists, including
the FP4/FP8 GEMM and attention examples and their historical four-kernel
hardware qualification in `examples/gfx950_low_precision/README.md`.
Those exact historical profiles are not new results of these representation
tests. The closed identity-scale FP4 numerical simulator is described in
[gfx950-fp4-exact-simulator-v1.md](gfx950-fp4-exact-simulator-v1.md); it does
not generalize to arbitrary scales or mixed/FP8 accumulation.

Remaining M4 evidence must connect applicable numerical, per-target negative,
emitted layout/register/resource and currentness checks to the intended
common artifact/generated-host route. Historical external-tool/HSA runs
and private BF16 engineering observations keep their original scope.
No tutorial, production, formal, native or hardware milestone is closed
by the representation tests or the inert simulator fixtures.
