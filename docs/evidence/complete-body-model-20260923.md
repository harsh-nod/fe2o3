# Bounded complete-body structural model — 2026-09-23

This is an implemented, tested **inert planning model**, not complete-kernel
source admission or #280 M2 acceptance. Current accepted original exits remain
M1/V1/V2/U1/U2/U3 (6/18).

## Implemented boundary

`fe2o3-amdgcn-model::Gfx942CompleteBodyPlanV1::check` copies a bounded typed
plan into fixed storage after checking:

- At most eight labeled blocks and sixteen total existing typed arithmetic
  instructions, with at least one instruction.
- Internal strictly forward jumps and uniform-selector-zero branches; unique
  labels, distinct branch successors, reachable blocks and valid terminal paths.
- Three initialized input roles; scratch/output must be defined before every
  read. Definition state at a merge is the **intersection** of predecessor
  states, not the union. Each guarded store/end path requires defined output.
- Five distinct explicit VGPR roles within v8..v63. v0..v7 are reserved for
  the proposed compiler-owned ABI/index/address boundary.
- Exact declared target, launch, implicit state, resources and fixed tail
  contract. The profile is gfx942:xnack- / wave64 with required and maximum
  workgroup dimensions 64x1x1.

Dead, repeated and self-writes remain part of the plan. This checker is not an
optimization pass. It rejects backward/self/missing/aliased targets,
unreachable blocks, undefined merge values, invalid roles and altered boundary
declarations. It introduces no heap allocation, recursion, emitter, executor,
source marker, serializer, canonical wire version or artifact owner.

The checker prepays 512 units of structural work and respects a pre-existing
budget floor. This is a bounded logical work contract, not a process RSS bound.
Any future retained source/compiler owner must separately account for its own
storage and expanded canonical graph.

## Provisional ABI, not generated-code evidence

The declared shape has a writable u32 slice, three uniform u32 data inputs and
a uniform u32 selector. Its proposed explicit argument offsets are
0/8/16/20/24/28, totaling 32 bytes with alignment eight; normal AMDHSA hidden
arguments still belong to the descriptor path. The selector's s22 reservation
is **new and unqualified**. Checked declarations do not prove a generated
prologue, argument descriptor, clobber set or store/wait/EXEC-restoration tail.

The compiler remains responsible for ABI/index setup and the fixed guarded
store/end tail. The author controls the admitted arithmetic, role placement
and forward control flow. Arbitrary memory, LDS, synchronization, runtime
helpers, recursive calls and naked entry-point ABI ownership are not enabled.

## Qualification

The pinned nightly, locked/offline MI350 CPU gate passed:

- 97 model unit tests, including 14 new complete-body tests.
- Six existing compile-fail documentation tests.
- Library/test Clippy with warnings denied.

The new tests exercise two- and three-predecessor initialization intersections,
independent terminal paths, exact block/instruction bounds, preserved dead and
self writes, every implicit-resource field, all reserved VGPR roles, and work
budget failure/floor behavior. No source callback, native object or GPU was
executed by this gate.

Retained runner receipt:
`logs/phase28-resume-r4-compiler-complete-body-model-r1/receipt.json`,
20,765 bytes, SHA-256
`a0815068d9c20b35b5cf40ad05403a45a7aff973d1fecb3117045d7be5133422`.
Its source census is 6,754 files / 102,029,504 bytes, SHA-256
`05125bfe0cf32edbe3981575b675ca245e84eda0dcef39ad0c5fa352e7f7cd51`,
at parent `60ff2707ad313726dda663e5b96823a994b5d15f` plus the four
model implementation paths. Documentation added afterward is not attributed
to that test snapshot.

## Next compiler integration

Actual source admission must derive the body from the authenticated Rust
Instance and current semantic/launch owners, produce real blocks, edges,
SSA definitions and merge arguments in the **same canonical executable graph**,
and independently check exact instruction/role/source occurrence preservation.
A second executable CFG blob or diagnostic-plan-to-production conversion is
not an acceptable substitute.

The checked output then needs its own supported fixed continuation, actual-L
descriptor/target lowering and the common Worker/link/inspection mechanism,
with fresh applicable obligations and generated ABI checks. The existing
source-local-order artifact path demonstrates that useful source-derived
LLVM/descriptor preparation can retain an explicit protected-publication refusal.
Full protected finalizer, generated-host and hardware qualification belong to
M6; they are not blanket prerequisites for the ordinary M2 source slice.
Existing V12 owners must not be relabeled to admit this new profile. Source
materialization and whole-body/mixed-source qualification remain required for
M2. Schema numbers
and shared registration are being coordinated in
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5803283453);
that request is not an allocation or approval.

## Fixed packing and transactional builder

A separate inert numeric codec now lives in `fe2o3-kernel-ir`, with the original
model type paths re-exported for compatibility. It represents up to eight blocks
and sixteen existing typed instruction descriptors in four block words and four
instruction words. All reserved bits, unused targets and inactive slots must be
zero; counts and descriptor grammar are checked without truncation. Numeric
packing is independent of host byte order.

The append-only fixed-array builder checks every bound before mutation. Failed
appends leave it unchanged. Empty individual blocks and cross-block role reads
remain representable: this codec deliberately does not duplicate CFG or
initialization analysis. The model's packed adapter prepays 64 decode/projection
work units and then invokes the unchanged 512-unit structural checker.
Retained storage and a future source/canonical expansion need their own accounting.

Qualification on parent `413ba987b8878f53e13127fdb53f9e1495ab93b3` plus this
packing change passed **1,470 kernel-IR/model tests**, with 22 ignored, including
12 new codec/builder tests and six composition tests. Exact literal encodings,
both endian round trips, padding, transactional errors, maximum sizes, unchanged
model refusals and nonzero work-budget floors are covered.

Strict all-target Clippy for both packages **failed** with nine existing
diagnostics in five unchanged local-frame files. Their bytes were compared
against the parent. No lint suppression was added. A separate all-target
Clippy run without warnings-denied, selected formatting and whitespace checks
passed; it does not make the strict gate green.

Retained receipts:

- Tests and strict-lint failure:
  `logs/phase28-resume-r4-compiler-complete-body-packing-r1/receipt.json`,
  13,557 bytes, SHA-256
  `1a86c3411ab69213abbd630a098c87ae48e8b4fd68bb82417d41434695edc788`.
- Non-strict lint, formatting and whitespace:
  `logs/phase28-resume-r4-compiler-complete-body-packing-lint-r2/receipt.json`,
  21,274 bytes, SHA-256
  `689fa60f503352bca947485af05c54f189426896ddc054a8f71670bdde1392bf`.

Both use 6,765 source files / 102,124,979 bytes, SHA-256
`a1b14cd491a950bd2ed5be36952f804560fb405a4f8b846f563a67f73735a903`.
No executable schema, trusted source marker, actual source compilation or GPU
execution is added by packing. It is not M2 acceptance.
