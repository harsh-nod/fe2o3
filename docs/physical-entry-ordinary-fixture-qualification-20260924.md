# Physical-entry transport qualification — 2026-09-24

The standalone [ordinary-shell fixture](../tools/fe2o3-llvm-link-worker/tests/physical-entry-abi/README.md)
passed its four native compilation/inspection cases on `mi350`. This is a
test-only transport result, not a source-produced assembly kernel, protected
artifact, GPU execution result, or M2 milestone exit.

## Observed machine contract

| Body | Optimization | Native entry | LLVM bytes | HSACO bytes |
| --- | --- | --- | --- | --- |
| Scalar fill | O0 and O3 | 21 instructions, 1 block, 116 bytes | 2,105 | 5,568 |
| Uniform selector | O0 and O3 | 25 instructions, 4 blocks, 132 bytes | 2,267 | 5,632 |

The scalar fill stores argument `a`; it is not a two-buffer copy. The selector
chooses `a` when the selector is zero and `b` otherwise. Both use exact
gfx942:xnack-, Wave64, COV6, workgroup [64,1,1] and maximum grid [2,1,1].

Every decoded instruction and the complete entry extent matched the independent
fixture contract. There was no compiler-added prologue or tail. The assembly
body supplied kernarg loads, indexing/address arithmetic, bounds masking, the
store, waits, EXEC restoration and termination. The ordinary LLVM function
still supplied the compiler-owned descriptor and metadata; no descriptor or
linked bytes were patched.

Both optimization levels produced identical bytes within each body:

- Fill HSACO SHA-256: `9ffc4c1968a80499d0addce910a126b968d9256d8565de2e95cd1ec453fbaf6e`.
- Selector HSACO SHA-256: `999390dfa07744c5b698835d8e98e415f2e34c7888c66d02a98e3083d9ec28a3`.

The descriptor reported RSRC2=132, properties=8 and preload=0, matching
kernarg s[0:1], workgroup-X s2 and local-X v0. Its capacities were 32 SGPRs and
16 VGPRs, with no spills, AGPRs, LDS, private allocation or dynamic stack.
The metadata retained six explicit arguments (32 bytes) and thirteen
COV6 hidden arguments (288 total kernarg bytes).

Each successful case also rejected its instruction/descriptor/metadata
mutations: 32 for fill and 34 for selector. Across four runs this is 132
mutation refusal observations, not 132 unique invariants. Eight additional
ordinary-worker input refusals covered wrong target and missing launch metadata.

## Reproduction and retained evidence

Use the fixture README's standalone CMake and four invocation commands with
the existing worker and the same pinned SDK configuration. The tested LLVM
revision was `f58b06dce1f9c15707c5f808fd002e18c2accf7e`; the reviewed ROCm
7.2.1 SDK build identity was
`rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540`.
The strict C++ build used warnings as errors.

Retained task evidence (SHA-256):

| Record | Digest |
| --- | --- |
| Build receipt | `a8402abf8a3b4d81a23581b3c2649238474a95aec48c02a5c737a48c08867317` |
| Fill O0 receipt | `cfe27588c2cbbe2bf10a224223f55f5dc9855809eec03878caec737fc4b52671` |
| Remaining three cases receipt | `2906cf2a7fc4d7a9a04f6799270811d3d2f7c15ba8b9ed8f356d2af32c8bcf04` |
| Four-case summary | `4f5c0e8a185ec2b822df805b488e363ed2a5231db123851c92b759640befc251` |

The matrix ran at compiler HEAD
`76fe660d9ef27961ec764c5cec1b7b79e6321a33` with the uncommitted fixture
included in source census
`0c7d973dc91a9cb07ce611f1227e409a5539f2245d1dc8ba74d391468d43deee`
(7,258 files, 108,069,207 bytes). The build and first case used earlier
recorded source censuses; their receipts retain those separate identities.

## Remaining integration

A passing hand-written LLVM fixture does not establish source authentication,
canonical SSA/CFG ownership, mandatory ranked/formal checking, simulation,
native functional refinement, production finalization or hardware execution.
Those must be connected through a separately declared physical-entry source
profile. Existing complete-body V19 still has compiler-owned setup and tail;
this fixture does not silently change that contract.
