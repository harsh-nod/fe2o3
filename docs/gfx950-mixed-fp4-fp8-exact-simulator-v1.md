# Exact-domain mixed FP4 × FP8 simulation

The canonical CPU simulator admits the existing gfx950 identity-scale
FP4 E2M1 A × OCP E4M3 B matrix contract. This is a bounded numerical
implementation, not a native GPU result or completion of M4. The device API,
KIR schema, target lowering, pure FP4/FP8 codecs and simulator limits are
unchanged.

## Supported operation

The operation is `ScaledMultiplyAccumulate` with the existing
`fp4_e2m1_f32_m16n16k128_wave64()` profile and
`gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64()`
layout: 64 active lanes at a uniform subgroup rendezvous. The existing gfx950
lowering selects formats 4/0 and fixes the scale controls to zero. This models
that existing identity-scale contract, not an interpretation of E8M0 byte zero
as a scale of one. Reversed operands, arbitrary scale controls and missing or
different layouts are not admitted by this implementation.

A is an FP4 E2M1 value from ±{0, 0.5, 1, 1.5, 2, 3, 4, 6}, excluding negative
zero. B is finite OCP E4M3, exactly quarter-integral, with magnitude at most 16,
excluding negative zero. C is positive zero or an exactly sixteenth-integral
normal F32 with magnitude at most 2^18. Valid format encodings outside this
closed simulator domain are refused, not rounded or clamped.

For `a = 2*A`, `b = 4*B` and `c = 16*C`:

```text
Q = c + sum(k = 0..127, 2*a[k]*b[k])
D = Q / 16
|Q| <= 4,194,304 + 128*2*12*64 = 4,390,912 < 2^23
```

All products and possible partial sums are exactly representable F32
sixteenths. Existing integer codecs therefore produce exact bits without a
floating-point rounding-order claim. Cancellation produces positive zero.

## Packed coordinates and failure behavior

A and B use different mappings. For matrix coordinates row, column, k:

```text
A lane = row + 16*(k/32),             nibble = k%32
B lane = column + 16*((k%64)/16),     byte = k%16 + 16*(k/64)
D lane = column + 16*(row/4),         component = row%4
```

Each operand has eight U32 words per lane. A uses four packed-nibble words and
requires four zero padding words; all eight B words are meaningful. The
executor validates all 64 lanes and computes all 256 results before binding the
first result. A late input-domain or padding refusal therefore performs no
matrix completion or subsequent store. This does not promise transactional
rollback for unrelated later event-sink or lifecycle failures.

Partial waves, divergent participation and mismatched matrix sites retain
their existing refusal behavior. Ordinary uninitialized-memory and bounds
checks remain active. The executor prepays 40 work units per arrival and
`333952 + 66*N` at resolution for N scheduled participants, with checked
arithmetic and reserved actual-size scratch storage. No work, resident-memory,
access-history, stack-depth or scheduling cap is raised.

## CPU qualification and limits

On base `01cf5996671499bd7bf71c9feb5fb5d840fa2c06` plus nine
implementation/test leaves, this command passed on mi350-2:

```sh
cargo test --locked --offline -p fe2o3-kir-sim -p fe2o3-kir-sim-cli -j2 -- --test-threads=2
```

There were 813 passed, zero failed and zero ignored tests across 45 result
groups. This includes six new private controls, fifteen canonical mixed
controls, the unchanged 128-KiB small-stack/deep-call regression, and existing
pure-format and CLI tests. The independent dense scalar oracle tests all 128 K
positions and all 256 outputs, signed/fractional inputs, exact extremes,
two-wave/workgroup layouts, seed/replay equality, input preservation and output
canaries. Negative controls cover every operand, late malformed padding,
convergence, ordinary memory errors, wrong layouts and exact/one-short resource
boundaries. Two-wave fixtures retain 16 MiB resident storage, 128 participants,
2,000,000 steps and 8,192 history records; a complete 256-decision schedule
passes and 255 refuses.

The normal receipt is 35,226 bytes, SHA-256
`9a65017181293ba12a0164232d9770c67c405b293f2745ae8f1f1c739415e156`.
Stdout SHA-256 is
`a74ab44973d2f91296d83e6dc07e774696910d73ab5199b7b35dc17109ab00f7`.
Before/after source census agrees: 15,413 files / 222,150,586 bytes,
`edfd89c53bb67ac03713e75e43f4cc5ca661a93da1fc968599e6a39497026ea2`.
Selected tool/input hashes are unchanged. The run finished at
2026-10-08 11:28:59 UTC; this documentation was added afterward.

These tests use inert canonical graphs, not an exported ordinary Rust mixed
kernel. No gfx942 counterpart, either target's hardware numerical result,
KFD dispatch, source execution authentication, performance prediction or full
race-history coverage follows. Required dual-target source-to-GPU correctness
remains open. The pure FP8 source-to-CPU result has its separate scope and
receipts in the [implementation status](assembly-authoring-implementation-status.md).
