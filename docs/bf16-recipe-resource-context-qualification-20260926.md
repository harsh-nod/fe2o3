# Original-ledger recipe context and borrowed checked references (2026-09-26)

This checkpoint is a prerequisite for completing source-bound helper-kernel
lowering. It does not admit that helper profile to normal compilation, create
checked-reference authority, bypass verification, or qualify hardware execution.
The accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implementation

The ordinary checked-reference continuation now borrows its three prepared
inputs through a shared helper. The existing owned wrapper delegates to that
helper without cloning the graph or changing its origin propagation, local
definition counts, store/atomic/discriminant handling, FIFO order, or refusal
semantics. This is a reuse boundary, not a new origin constructor.

A concrete nominal recipe-resource context borrows actual canonical facts and
the original preparation ledger. It checks the exact rich-function association,
rejects masked facts, and queries actual entry-block materialization before
calling its continuation. Resource operations and facts queries alternate on
that same ledger; no replacement budget is used for successful preparation.

The context reserves storage but never refunds it. Its caller owns the physical
payload and accepted-credit counter across both nested source/facts scopes.
Payloads and bounded errors or panic payloads must be dropped before the caller
releases only its own credits. Separately reserved callback surplus survives.
A sticky resource denial, ledger substitution, floor erosion, or counter
regression refuses rather than repairing or resetting accounting. A propagated
resource error can therefore be superseded by the accounting postflight;
non-resource errors are forwarded only while custody remains valid.

The genuine test hook enters through the real owner's entry-resource floor,
actual rich-source preparation, actual canonical facts, and the concrete
context. It observes a paid materialization bitmap, not an assertion mask or
memory certificate. Every reached run prepays 33,280 logical work before
constructing the fixed 8,192-byte captured payload; separate conservative
control precharges are retained. No resource cap was raised.

## Controls and actual-source evidence

The regression gate passed 331 model tests and 2,572 backend tests (189 ignored),
plus backend/extractor build and the whitespace check. Fifteen new tests cover
the context/borrowed-reference boundary and the fixed genuine-hook envelope.
The first regression request failed to compile because two test `assert_eq!`
macros demanded `Debug` on an intentionally opaque ledger identity. Equality-only
assertions corrected those tests without exposing the identity or altering the
production API. The failed request and its streams remain retained.

Fresh R16 completed five Rust sessions: Identity, Swap01, wrong launch, callback
error and callback panic. The two positive source variants retain 36 positive
CPU numerical cases and 32 request refusals; their two normal helper-compilation
refusals are unchanged. Error, panic and wrong-launch sessions remain separate
controls, not extra successful source variants.

Actual context observations report 19 source blocks, 19 materialized blocks,
61,883 owned logical storage credits and one context callback. Each affected
stderr contains four ordered retained-effects/context/assertion triples at
(16,17,18), (44,45,46), (47,48,49), and (50,51,52), followed by one retained-only
row at line 53. The last row belongs to the one-short-storage probe; its later
failing allocation remains uninstrumented. Wrong launch has no such rows.
The 19-block fixture has zero assertions, so this is not positive actual
assertion coverage or an independent allocation-contract payload oracle.

The hook also exercises same-owner surplus, error and panic cleanup, foreign
semantic-function and missing-function refusals, foreign correspondence, and
masked-facts rejection before the context callback. The separate floor-minus-one,
zero-work and no-extra-storage probes are negative-only ledgers whose full caps
are prepaid on the original ledger. They do not establish an inner-context
success on replacement budgets.

R15 and R16 dependency directories were independently enumerated: 459 regular
files and 352,670,356 bytes each, with all 918 file hashes checked. All six
fixture/device source pins in the R16 report match the compiler checkout. The
comparator separately joins seven current implementation/test inputs to the
completed gate. Numeric comparisons use exact
JSON number lexemes, not floating-point round trips.

## Normal compilation and lossless comparison

The fresh normal suite completed 36 composition sessions and two direct-BF16
sessions. Composition retained seven finite checked owners, seven public LLVM
outputs, seven inert handoffs, seven dynamic-source refusals and eight
invalid-source refusals, including seven same-source joins and 21 descriptor
mutation controls. The separate direct-BF16 cases retain the normal handoff and
wrong-launch refusal; they do not admit the helper profile described here.

Root compared all 38 observation bodies with the preceding ordinary-recipe
checkpoint using exact numeric lexemes. They are unchanged. All 45 report-bound
artifact hashes were rechecked, along with seven additional worker LLVM files;
all 52 files are byte-identical to the preceding checkpoint.

The R15-to-R16 comparison passed 40 controls. Its six previous core/parser/test
files are byte-identical; the original 138 explicit comparison paths remain.
Exactly 132 values changed: 96 cumulative-work fields, 25 generation paths,
six dependency digests and five artifact digests. Numerical results, u64 masks,
refusal payloads, normal/authority flags, storage and peaks remain exact.
The logical phase/reverification peak stays 1,632,943,151.

Uniform cumulative-work increases are 25,910,038 for Identity/error/panic and
25,910,326 for Swap01. Source-explicit charges total 25,766,162, including eight
original-ledger run admissions, seven control precharges, three fully prepaid
negative probes, bitmap operations, context frames and the real masked-table
scan. The remaining 143,876/144,164 are aggregate consistency inferences, not
instrumented per-function or equal per-run costs. Repeated diagnostic rows from
the existing prepaid boundary probes do not multiply original-ledger work.

The 61,883 credits are a logical envelope: 40,960 header, 19 bitmap, and an
inferred 20,904 generic frame contribution. This is not an allocator/RSS
measurement or independent machine-stack bound.

## Retained evidence

Paths are relative to the retained MI350 task root
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| R23 `compiler-root-recipe-resource-seams-regression-r2/receipt.json` | 76326 | `3fa052ec8810bdaf7538308557562ecb0890f92cc4cd884c5b45f0ea4454606d` |
| R24 `compiler-root-recipe-resource-genuine-r1/receipt.json` | 325029 | `e0053cf712f41136216d6943c558c6b90a4813718cfd55cfcf04841979dcc50a` |
| R24 `compiler-root-recipe-resource-normal-ladders-r1/receipt.json` | 133374 | `19be3eb0156e0048a261a4ee729dae5b41e4efa06dbc16a4341bf6326d6c94ab` |
| R24 `compiler-root-recipe-resource-lossless-comparison-r1/receipt.json` | 87938 | `c455a568003ab74c4e263a5cc7983fd62db40dc401a52ee61e13878bb354a4e3` |
| `phase28-drafts/bf16-n3-root-recipe-seams-integration-r24-r1/FINAL-QUALIFICATION-READBACK.json` | 16379 | `6c337de8f5fa6e5ea6d363af8e4066b5b153024093b457bd33eb9f4e3a48d2cd` |
| `phase28-bf16-call-source-cpu-actual-r16/observation.json` | 281064 | `6f664b280dad3483ab997f74ca963db1f11a12211e4397a7a9afa64c0feb71ad` |
| `phase28-bf16-call-source-cpu-actual-r16/dependency-files.json` | 85886 | `1a0e0ab24e2bddb8229d457502b6d7e5960a03e1b4930c065e09f84e038207e0` |

Gate paths use `logs/phase28-resume-r23-` or `logs/phase28-resume-r24-`
before the labels above. Successful source/input/tool before-and-after snapshots
match. These are sampled command observations, not transitive build attestations.
Backend warnings remain; no whole-backend strict-Clippy result is claimed.

## Still required

This context does not yet construct the authenticated source-edge graph or strict
checked-reference origins. Completing the nominal recipe still requires those
inputs, runtime index/extent/Option guards, the retained Final table feeding the
shared intrinsic continuation, exact tensor insertion, metered CFG and assertion
overlay, actual source-to-ranked emission positions, and output-write expressions.
Normal admission and formal/target/LLVM continuation must be qualified after those
connections exist. Neither the context bitmap nor matching source indices can
substitute for them.

See the [earlier shared ordinary recipe checkpoint](bf16-shared-root-recipe-qualification-20260926.md)
and [retained Final effects checkpoint](bf16-helper-retained-effects-qualification-20260926.md)
for their separate evidence and limits.
