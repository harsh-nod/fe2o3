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

## Ordinary mixed Rust GEMM CPU regression

A separately named ordinary Rust example, `gfx950_mixed_fp4_fp8_gemm_rust`,
is selected by the `kernel-mixed-fp4-fp8-gemm` feature in
`examples/gfx950_low_precision`. It retains sixteen M16×N16×K128 tiles,
four workgroups of 256, typed low-nibble FP4 A loading, split-K FP8 B loading,
and four disjoint output stores per lane. No new device API or simulator cap
is introduced. The existing four pure-format features and their historical
lowering claims remain separate.

The Linux integration target
`production_mixed_fp4_fp8_source_simulation_v1` contains ordinary controls and
an ignored genuine exporter test. That test invokes the existing
`fe2o3-export-sim --crate fe2o3_gfx950_low_precision --target gfx950
--bundle-version 6 --output <fresh>/kernel.fe2sim --target-dir <fresh> --
--manifest-path examples/gfx950_low_precision/Cargo.toml
--features kernel-mixed-fp4-fp8-gemm --lib --offline`.
Its fresh paths, matching toolchain/loader environment, source/dependency
bindings and complete child-process containment must be supplied by a bounded
qualification owner. This document does not authorize or attest an execution.

The test consumes the exact exported Bundle V6 / KIR V11 through the existing
strict CLI admission. It compares all 4,096 F32 outputs, four tail canaries and
unchanged A/B buffers against a scalar-coordinate f64 host reference using
independent format decoders. The chosen finite inputs have exact eighth-grid
products and |partial sum| ≤ 12,288, making the final f32 conversion exact.
Wrong expected canaries, genuine late FP4 negative zero, FP8 NaN/off-quarter
inputs and damaged bundle bytes are negative controls; no KIR is constructed,
repaired or substituted. Original CLI limits remain 2^27 steps, 256 MiB
resident memory and 65,536 memory-access records. The normally ignored source
test was explicitly selected under the bounded owner described below; failures
remain failures, not accepted fallbacks.
The final merged-source mixed qualification on 2026-10-08 passed all 13
ordinary controls and the explicitly selected genuine test (45.06 seconds).
It ran on public base
`e1e62154d5e87eeae4f461c6846c6786d5aaca27` plus the seven source/test/guide
leaves, before the final receipt documentation. The normal receipt is 76,155
bytes, SHA-256
`3d4ff4fd4cd9044533407e887a067c6914bdb45c9854d2ef0ea9f48037d7322c`;
the run finished at 13:37:53 UTC. The actual same-module KIR V11 is 43,126 bytes,
`90932d2975183d69398759b911702db0d4f9782784eb26205f5682fc1c711036`,
and contains one mixed matrix operation. Its result is CPU simulation only.

The documented public host route also passed on that exact merged source:
`bash scripts/quickstart.sh source-check examples/gfx950_low_precision/Cargo.toml`.
It checks the package through the normal frontend and runs all host targets:
9 library tests and 3 source-contract integration tests passed.
Receipt: 99,191 bytes,
`045b7687b7e9f4b09ddcc3958d44d38521cc89aae87126a7060d63c19bbffb16`;
finished at 13:31:24 UTC. No internal macro bypass was used.

The unchanged pure-FP8 source regression then passed on the same merged
source: 11 ordinary controls (including the three added shared reference
tests) and its genuine test (43.47 seconds). Receipt: 83,619 bytes,
`8bdc5bf97c1514a19bd6a5065ef20c70f14c5e87d35e9c9142dc9937cbda436c`;
finished at 14:04:37 UTC under the fresh R89 window with unchanged limits.
Its original feature, kernel body, inputs, two typed-domain refusals,
canary mismatch and damaged-bundle control were not replaced by the mixed case.
The pure-FP8 KIR V11 remains 42,407 bytes,
`1dc71f554f2399f3a056cc4d9809f193b5e964c13efd4863fc8c1e1aef828c46`.
Its source census matches the host and mixed runs below exactly.

The merged host and mixed runs retained identical before/after source census:
15,415 files / 222,198,746 bytes,
`4be88999e91f8520815c04482a64a4a245eb0c7e790a476642aab02eba126fc5`,
with selected tool/input pins unchanged. The receipts do not authenticate
every transitive tool or grant compiler, proof, artifact, load or launch
authority. Neither run accessed a GPU.

Earlier mixed and FP8 runs on base `2f119c4` also passed (13+1 and 11+1),
but are historical, not merged-source evidence. Two host attempts failed:
plain Cargo correctly rejected device-only kernel macros; the first public
source-check then passed 9 library tests but failed one stale integration
assertion that expected dimension literals in the source file. The correction
checks the compiled public constants and preserves all kernel safety and
export-count assertions. Those failed receipts remain failures:
`7c7b540ca999d54ccb92bcbfc558411ec2f360160ca3f3780146bcfe89e496d2`
and
`72236057fd955ccb54068fe2ddc4d1d018a3b7cbd99ac60da53f897b970bae88`.

This new GEMM does **not** establish that the existing
`gfx950_moe_expert_rank_fp4_fp8_v1` example works in the simulator.
That complete ordinary source also computes SiLU through `exp_f32`;
the current closed scalar simulator contract refuses Exp before execution.
The MoE source is unchanged. No exp approximation, branch-based waiver,
gfx942 counterpart, hardware correctness or full M3/M4 completion is claimed.
