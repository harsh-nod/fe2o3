# Device const complete-body data — 2026-09-23

The device SDK now exposes no_std, allocation-free const packing and a
transactional builder for bounded complete-body metadata: eight blocks,
sixteen total arithmetic instructions, body-local labels and typed terminators.
It preserves the existing MOV/ADD/SUB/AND/OR/XOR descriptor grammar.

This is an M2 foundation, not whole-body source admission. No new trusted
marker, semantic/KIR version, identity domain, ABI, canonical executable CFG,
LLVM emitter or production/protected publication path is registered. The
separate compiler model still owns CFG/all-path initialization/resource checks;
a valid packed value is not a semantic or execution authority.

## Exact provider refresh

Both accepted full provider materializations were recomputed over the unchanged
canonical manifest and all 31 regular SDK source files (423,135 raw source bytes):

- Canonical: `570816af9f221a87dfe87d7be9257da3812835de665707db968e1dd36b56e952`.
- Cargo-vendor: `c90b0fbce11611eaba245ad8d5456f55628600406972f58db6882c8a6f6e6f92`.

The canonical manifest remains 398 bytes; the reviewed Cargo-generated vendor
manifest remains 1,922 bytes. No build script, device dependency or new SDK test
target is added. The normal-dependency parity fixture is a separate nested
workspace. Old closures are not retained as acceptance fallbacks; current
provider identity and old terminal/domain rules remain otherwise unchanged.

## Package and provider qualification

Pinned-nightly, offline/locked SSH MI350 qualification passed:

- All 10 normal-consumer parity tests and strict all-target consumer Clippy.
- All 197 device unit/integration/documentation tests. Compile-fail fixture
  cases inside the harnesses are not added to that top-level count.
- All 1,761 backend library tests; 117 separately ignored tests remain ignored.
- Backend library/exporter/extractor, inspector/simulator and diagnostic LLVM
  lowerer builds, affected Rust formatting and whitespace checks.

Parity exhaustively checks all 65,536 descriptor words and count pairs, exact
error variants/fields/precedence, all 260 legal descriptors in every instruction
slot, block slots/targets/padding, endianness-independent numeric packing,
transactional overflow and genuine const evaluation. The SDK does not depend
on kernel-ir; the host fixture compares both ordinary dependencies.

Provider regressions include exact canonical/vendor materialization identities,
actual Cargo target-roster checks, manifest/source mutation refusal and stale
compiled-source-hash refusal. These unit controls are distinct from the genuine
normal-source qualification below.

Full device strict Clippy is NOT clean: the initial gate stopped on the two
pre-existing eight-argument functions in diagnostics.rs (lines 88 and 106).
That file is byte-identical to parent fbb5ca1866aa8c502dab1c122834e45f28d9f53e;
SHA-256 `46a6a137b1e86c4c80379abf2478fb8d801c298e781fd2312ff55334877df2ab`.
The later device Clippy run reports warnings without denying them; no lint
suppression or unrelated API rewrite was introduced. The standalone consumer
strict gate remains independently successful.

The failed initial receipt is 13,473 bytes, SHA-256
`3af206c7084d4fc1dbdbdc771b4d907a6121260d4928ea008b8435587389dd82`.
The successful regression/build receipt is 24,298 bytes, SHA-256
`81aef14680253dc1fcd28bc70113eb5dd23a70d62799c71d520d86ea47b50598`.

## Fresh existing-source qualification

Freshly rebuilt matching backend, wrapper and exporter were used with new
extraction caches. The existing one/two/fifteen/repeat source fixtures produced
four normal source exports, eight exact refusals and 120 whole-kernel CPU
simulations (136 process stages). No old compiled SDK/backend pair was used.

Refusals cover zero/oversized/huge repetition, expanded instruction overflow,
dynamic and nested syntax, undefined initialization and aliased physical roles.
Each refusal has its expected diagnostic and no output KIR. Repeated equal
source reproduces equal current canonical identities; historical identities
are not imposed on the updated provider.

Four fresh diagnostic typed LLVM lowerer calls then revalidate the source/KIR
joins, exact declared descriptors and register constraints, instruction order,
result-to-store linkage and repeat equality. That second gate rechecks the
retained 120 simulations; it does not add 120 new simulations or run LLVM native
code generation, a GPU, or the new complete-body representation.

| Retained receipt | Bytes | SHA-256 |
| --- | ---: | --- |
| source/CPU | 354007 | `62a7310410dd3fcb2da00c9170bebf77b3bb0a5243897a72777439875c044a97` |
| diagnostic LLVM | 207502 | `c0b2b0ce067630a649be9b84df6f935efd1e62e262ee5740d3f3752e885b855f` |
| outer source/LLVM command | 25634 | `017a03baa8e32c2f71a06400a7832f3f6b69c3a622333a7b5c72977b3ee5be35` |

The selected backend DSO is 246,042,568 bytes, SHA-256
`032baba3484ba7565c9ad048c93bd9c4946af74a778497db9244ade78231f5d8`.
Selected input/tool/output pins and source census were checked before/after;
these observations are not transitive compiler-binary attestation.

Both passing outer gates used the same 6,777-file / 102,240,131-byte source,
SHA-256 `242dfa4f47cf6569e10753e27bf4503ac5200e9e35a7080c63e720623149f086`.
This report and status documentation were added afterward. Private receipts
are retained under phase28-repeat-source-device-closure-r1,
phase28-repeat-llvm-device-closure-r1 and the matching
logs/phase28-resume-r4-compiler-device-closure-* directories.

## User-facing continuation

The [const-builder tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/complete-body-const-builder-v1.md)
uses the tested constant and distinguishes grammar from all-path initialization.
The companion [logical lifetime view](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/ordered-role-liveness-qualification-20260923.md)
is separately qualified retained-data analysis, not new physical observations.

Actual authenticated whole-body source collection, one canonical CFG/SSA,
normal verification/optimization/ABI/emission and source re-entry remain M2
work. Protected finalization/generated-host admission remain distinct M6 work.
Accepted original exits are unchanged: M1/V1/V2/U1/U2/U3 (6/18).
