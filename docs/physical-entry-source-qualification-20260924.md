# Source-authored physical-entry qualification — 2026-09-24

The experimental V20 profile now connects an actual Rust-authored physical body
to a checked canonical SSA/CFG owner, the existing CPU simulator, and unchanged
LLVM consumed by the ordinary native worker. This is a bounded diagnostic
continuation, **not** normal production admission or a completed milestone.

See [the reproducible source command](physical-entry-source-v20.md) and
[the independent native checker](../tools/fe2o3-llvm-link-worker/tests/physical-entry-abi/README.md).

## Authored control and checks

The author controls the complete entry instruction body and physical registers:
kernarg loads, index/address arithmetic, carry, the uniform selector CFG,
bounds comparison, EXEC save/mask, store, waits, EXEC restoration and termination.
The source signature remains five typed arguments; the ordinary LLVM entry
carries six native slots. There is no handwritten descriptor or object-byte patch.

The closed profile is gfx942:xnack-, Wave64, COV6, workgroup [64,1,1],
maximum grid [2,1,1], one block or a four-block diamond, at most 64 native
instructions and 73 source marker occurrences. It does not admit global input
loads, helpers, loops, LDS, barriers, atomics, matrix instructions or arbitrary
assembly text.

Source authentication checks actual rustc instances, signatures, provider bytes,
argument transport and marker occurrences. The move-only materialized owner
retains semantic SSA, source launch, canonical graph and source correspondence.
Replay reconstructs the graph. Canonical verification checks exact register
definitions at CFG merges, pending LGKM loads, pointer-half/carry origins,
bounds/EXEC/store relations, restoration and terminal control flow.

These checks catch the tested wrong launch, replaced input argument, undefined
register at a merge, missing load wait and mismatched pointer carry. They do
not establish general memory safety, race freedom, latency behavior, host
pointer validity, or equivalence of arbitrary author edits.

## Observed source and simulation results

On mi350, the actual-source ladder passed 16 isolated compiler sessions:
eight fixtures through both live observation and the diagnostic driver.
Three positives (scalar fill, uniform selector and a v8-to-v22 register edit)
passed 576 CPU cases in the existing simulator. Five malformed sources each
failed twice at their precise boundary, without diagnostic output.

The CPU matrix includes both selector outcomes, 64/128-thread grids, lengths
0/1/63/64/65/127/128/129, extreme scalar values, nonzero view offsets, canaries
and inactive-lane behavior. Symbolic pointer components and carry are not
invented numeric GPU addresses. Inactive stores do not fetch their operands.

Separately, all eight public Cargo/wrapper commands passed their expected
outcomes: three diagnostic exports and five named source refusals. The public
command itself runs neither the CPU simulator nor the native worker.

The live ladder and public Cargo invocation have distinct source identities
and canonical digests. Their three positive LLVM bodies match byte-for-byte;
this does not make their source owners interchangeable.

## Independent native compilation and inspection

The actual ladder's diagnostic LLVM and same-owner sidecar were separately
joined to the source observations, then passed unchanged to the pinned ordinary
worker at O0 and O3. The independent decoder checked every instruction,
implicit state, complete entry extent, register requirements, descriptor and
all 19 metadata arguments (six explicit plus thirteen hidden).

| Source | Native instructions / blocks | Entry bytes | HSACO bytes | Required / allocated VGPRs |
| --- | --- | --- | --- | --- |
| Scalar fill | 21 / 1 | 116 | 5,400 | 9 / 16 |
| Selector | 25 / 4 | 132 | 5,504 | 9 / 16 |
| Register edit | 25 / 4 | 132 | 5,520 | 23 / 24 |

Each row passed both optimization levels with identical HSACO bytes within
that row. All used 20 required / 32 allocated SGPRs, 32 explicit kernarg bytes,
288 total COV6 kernarg bytes aligned to 8, and no LDS or private allocation.
No compiler-added prologue or tail was observed.

Across the six runs, 246 native mutation refusals, 66 sidecar refusals and
12 ordinary-worker input refusals passed. These are repeated observations,
not that many independent invariants. Mutated metadata payload digests are
retained; two metadata control kinds do not carry the separate fresh-payload
flag used by instruction/descriptor controls.

This path still represents the authored body as one side-effecting inline-asm
statement followed by unreachable in ordinary LLVM IR. LLVM emits the kernel
descriptor and metadata. We are not bypassing LLVM, reconstructing source from
assembly, or claiming native functional execution from machine-code inspection.

## Retained evidence and limits

The source/public/native gates used HEAD
ed69ea8c845a5d13adbaceedf1b6b3321217d6ed plus the reviewed V20 candidate,
source census 0a6f3e5f756debe7cbade2c801e71c3acf3bf2377215090c079df274fde0e4ff
(7,335 files, 108,820,967 bytes). The later broad regression used census
aeb58793ab3d5118019d87ec6da0efc3440842d9850c875d58c79bb16682ab63
after exact legacy storage golden updates.

| Retained task receipt | SHA-256 |
| --- | --- |
| Source ladder r3 | 5f7d5f0e54e3427a0540c78bd25d55e3c18fd88ef2fdc585ae2d81a416f2f773 |
| Public source matrix r1 | e3da961be4878dd37a80a57870ade9a71e037eb2df8b1248fdd5a401d11fd4db |
| Source native matrix r1 | 08ac6cba88764585d44ce41d166a23f22489b8ac5b3bc7dfdc8f12318f1e1892 |
| Broad regression r6 | ab6b0cb137ec0cd6a3bcc0440ba96ec407c0e27385eddccc7bd99480fbfb2943 |

The broad run passed 6,605 tests across 135 suites; 158 explicitly ignored tests
were not silently counted as passed. It covered IR/MIR/model/simulator/CLI
all-targets plus debugger/protocol/lowerer/backend libraries. The unchanged
256 KiB simulator stack test also passed after outlining existing hot arms;
no stack threshold or accounting check was relaxed. V20 enlarged the IR
operation type, so three legacy storage goldens were remeasured; exact
one-byte-short failure and caller-ledger assertions remain enabled.

Provider identity pins were independently recomputed over the unchanged
framing algorithm and full 33-file closure. Old pins were replaced, not
retained as alternate admissions. The diagnostic provider source was unchanged.
Added source/canonical/emission accounting shares one cumulative ledger;
rustc planner accounts and output I/O bounds remain separate, not whole-RSS claims.

The native observer still uses inert worker identity fields, with an explicit
external same-source join. It exports no compiler custody. Normal ranked/formal
descriptor continuation, protected finalization, host obligations and GPU
execution remain pending. Raw V20 debugger loading and hardware register capture
refuse explicitly. The six accepted umbrella exits remain M1, V1, V2, U1, U2
and U3; this increment does not close #280, #281 or #282.
