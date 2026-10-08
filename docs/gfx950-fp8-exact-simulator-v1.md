# Exact-domain gfx950 FP8 scaled-MFMA simulation

This exact-domain implementation is CPU-qualified in the existing canonical
simulator. It is not a native execution result or completion of M4. The existing device FP8 packers, exact
OCP-format LLVM lowering and low-precision reference kernels remain unchanged.
Their prior source and hardware evidence is not recreated or promoted by this
simulator implementation.

## Closed descriptor and numerical domain

The only new admitted operation is the existing
`ScaledMultiplyAccumulate` profile
`fp8_e4m3_f32_m16n16k128_wave64()` with the exact declared
`gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64()` tensor contract,
64 active lanes, uniform subgroup convergence and the existing 64-bit simulator
target. It is OCP E4M3 (bias 7), not FNUZ or E5M2. The operation has no arbitrary
scale operands: the existing gfx950 lowering fixes both format selectors and
all scale controls to zero. This implementation models that existing identity-scale
meaning; it does not interpret an E8M0 byte zero as a scale value of one.

A and B are finite, exactly quarter-integral OCP values with absolute value
at most 16. C is positive zero or an exactly sixteenth-integral normal F32
with absolute value at most 2^18. Negative zero, NaN, off-grid values and larger
magnitudes are refused, not rounded or silently clamped. Other valid OCP
encodings outside this subset remain unsupported simulator inputs; the format
itself is not declared invalid.

Using integer units `a = 4*A`, `b = 4*B`, and `c = 16*C`, compute

```text
Q = c + sum(k = 0..127, a[k] * b[k])
D = Q / 16
|Q| <= 4,194,304 + 128 * 64 * 64 = 4,718,592 < 2^23
```

Every product and every possible partial sum is an exactly representable F32
sixteenth. Integer accumulation and final bit construction therefore avoid a
rounding-order claim. Exact cancellation produces positive zero. This is not
a general floating-point MFMA model.

## All eight packed words and split K

Each lane supplies eight U32 words for each operand: four bytes per word and
all 32 bytes meaningful. Unlike FP4, the high four words are not padding.

For lane `L`, group `g = L / 16`, and byte component `t`:

```text
k = 16*g + t                  if t < 16
k = 64 + 16*g + (t - 16)     otherwise
A coordinate = [L % 16, k]
B coordinate = [k, L % 16]
D component d coordinate = [4*g + d, L % 16]
```

The executor uses the inverse `g = (k % 64) / 16`,
`t = (k % 16) + 16*(k / 64)`. This is the existing device packer contract,
not FP4's contiguous 32-element K interval. The independent test fixture packs
row-major matrices and computes a dense scalar oracle without calling the
simulator decoder or lane lookup.

## Execution, refusal and accounting

The existing cooperative rendezvous requires a full physical wave at the same
matrix site. Partial waves, divergent participation and mismatched sites do
not receive synthetic zero lanes. Mixed FP4/FP8 remains unsupported. BF16 and
FP4 numerical paths remain unchanged.

All 4,352 input components are screened in deterministic lane, operand and
component order. Every input is validated and all 256 numerical results are
calculated before the first lane result is bound. Thus a late domain failure
causes no matrix completion or following store. Subsequent inherited event-sink
or lane-lifecycle failures are not transactional rollback; earlier successful
completions are not undone.

The existing prepaid logical accounting remains in force:

- 40 arrival work units per participant.
- `268160 + 66*N` resolution work units for N scheduled participants:
  64*68 screening, 256*128*8 MAC units, 256*4 result units, 64*5 completion,
  320 scratch initialization and bounded rendezvous/member scans.
- Fixed scratch/input/move-overlap payload is reserved before execution using
  actual Rust type sizes, without a boxed wave or a larger shared value type.

No simulator cap, default, access-history count, schedule mode or byte-accounting
guard is increased or discounted. Two-wave tests retain 128 participants,
the 16 MiB resident limit and 8,192 access-history records. Their exact complete
schedule is 256 decisions; 255 must refuse. Numeric result checks do not claim
complete race-history assessment when the bounded history is incomplete.

## Validation coverage

Four private numerical controls cover every one of the 256 byte encodings,
OCP/FNUZ distinction, exact descriptor admission, accumulator boundary neighbors
and independent product/result bit encoding. Two executor controls cover
overflow-safe work and actual inline/scratch type layout.

Fifteen canonical integration controls exercise a dense signed/fractional
oracle, all 128 reduction positions and all 256 outputs, exact extrema and
cancellation, all eight packed words, two separate workgroup/wave layouts,
seeded/replay equality, typed late input failures in every operand/word,
ordinary initialization/bounds errors, partial/divergent/mismatched waves,
prepaid work refusal, exact/one-short step and resident limits, layout omission,
32-bit target refusal and retained mixed-format refusal. Four output canaries
and unmodified input bytes remain checked.

The existing FP4 mixed/FP8 compatibility test changes only the former
FP8-unsupported expectation to exact-profile preflight admission. Its mixed
refusal and all FP4 numerical checks remain.

The private and canonical controls passed in the complete simulator suite,
and the canonical CLI compatibility suite passed separately. The exact CPU
checkpoint is recorded below.
No native/GPU launch, source-atomic lowering, arbitrary scaling, performance
budget or whole M4 exit is established by this patch.

## 2026-10-08 CPU qualification

Implementation commit `40ce99c1b9f73801da185029a4ba5772fb5db4cb` is published
to `main` in both compiler repositories. Its nine source leaves were tested
unchanged on `mi350-2` atop `f4cb21ba50a0b4413e745aaa1cf11c9483ff41d5`.
The pre-commit source census was 15,380 files / 221,568,223 bytes, SHA-256
`768c827c876fa5e3fa9fd89932053f6643899fbe03eca896451a6dcca1e6ef4f`.
This later documentation update is separate from that measured source.

- Complete `fe2o3-kir-sim` suite: 607 passed across 33 groups, zero failures
  or ignored tests; includes all 21 new FP8 controls and existing BF16/FP4 tests.
- `fe2o3-kir-sim-cli --test canonical_v12`: nine passed, zero failures
  or ignored tests.
- Existing resource limits were unchanged; exact and one-short work/resident
  boundaries and the two-wave 256/255 schedule controls passed.

The normal full-suite receipt is 30,783 bytes, SHA-256
`d10ed0e6f888a62a5a590575c8cdd8bfb81c407c895570085da3477549e6be68`;
the CLI receipt is 30,220 bytes, SHA-256
`f4cc4211ae2d330b50b8adf2817d5e0f40f5461790af0b9f2bfa77b1e92dbf03`.
Both record matching source and selected inputs before and after execution.

The numerical fixtures are inert canonical graphs. These results do not yet
qualify the real Rust FP8 kernel through source export, native gfx950 execution,
arbitrary FP8 inputs or scaling, or the complete M4 milestone.
