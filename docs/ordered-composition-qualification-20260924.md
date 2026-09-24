# Ordered-u32 composition qualification — 2026-09-24

This records bounded CPU/source qualification, not completion of issues #280–#282.
The new composition route reuses MIR32/KIR17 without widening the old singleton
route. It retains one actual source root, at most two direct scalar helpers,
eight static calls/regions and 128 expanded authored instructions. Helpers have
the exact admitted three-u32 Rust ABI. Source identities in exported files are
observations, not a way to reconstruct compiler custody.

## Completed source checks

| Gate | Observed scope |
| --- | --- |
| Normal source R6 | 36 sessions: seven finite shapes × checked observation/normal LLVM/inert handoff; seven original dynamic-launch refusals; eight malformed-source refusals; 21 descriptor-extension mutations. |
| Same-session publisher R7 | Nine actual rustc sessions, three create-new publications, two fresh promoted frontends, 128 CPU cases, four HIR-profile refusals, two stale-source and two no-replace refusals; a post-publication refusal retains possible-effect facts. |
| Public library action R1 | 17 isolated children: 14 public driver invocations plus three fresh promoted-source callbacks; three publications, 96 CPU cases and ten exact refusals, five before frontend startup. This gate does not invoke the extractor binary. |
| Extractor CLI R2 | Completed outer gate: 17 actual binary invocations, three fresh recompile callbacks, three publications, 96 CPU cases and 13 exact binary refusals, eight before frontend startup. The independent retained-artifact review also completed; no fresh promoted-candidate canonical file was independently retained. |

The normal shapes are root-only, helper, repeated calls, root-plus-helper,
distinct const monomorphizations, scalar-only helper and wrapping scalar helper.
The normal route is selected explicitly with
`FE2O3_EXTRACT_ORDERED_COMPOSITION_V1=1` for normal LLVM or inert handoff;
diagnostic/promotion selector collisions and semantic-V3/protected output refuse.

The finite fixtures have source `max_grid = [2, 1, 1]` and workgroup size 64, giving a
128-invocation envelope. The normal checker retains the original formal memory
obligations **and** an additional unresolved 512-byte writable-output requirement.
It does not claim that the source guard proves that full buffer length. The
ranked graph is a safety over-approximation, never a replacement executable or a
functional-equivalence theorem. The real helper/call/region graph remains in LLVM.

Promotion inserts a body-local helper at actual HIR/source coordinates, optionally
edits typed instructions/register bindings, and writes a new source file without
overwriting the original. Its fresh frontend/CPU checks are separate from the
seven original finite normal fixtures. No normal/native qualification is
transferred automatically to a generated candidate. See
[the source-promotion workflow](ordered-composition-source-promotion-v1.md).

## Fresh ordinary static matrix — completed

After the repeated-call correction, a new observer was built from the integrated
worker and passed its real machine-effect controls plus 40 resource controls.
The fresh normal-source R7 gate passed the same 36-session ladder on the merged
source tree. Root and an independent reviewer joined all source/artifact streams;
the independent audit rehashed 700 files, including 459 dependency files.

The new native R7 matrix passed all **14/14** ordinary LLVM/LLD/MC cases. Each
passed 18 metadata mutation refusals and eleven decoded-observation mutations:
252 and 154 respectively. The previously incomplete two-calls/O0 case now reports
two actual decoded direct-call sites with one unique helper graph edge. The fix
retains bounded site counts, cycle/depth checks and combined effect/trace
reconciliation; effect-only direct-call evidence requires a complete trace.

Root joined all fourteen child exit/stream records, unchanged observation files
and HSACO hashes. Completed outer source/tools/input snapshots matched. Neither
this static matrix nor its mutation controls prove native helper argument/result
transport, root functional equivalence, dynamic execution order, host buffer
validity, GPU behavior or protected authority. These are the seven original finite source profiles. The separately completed
[fresh promoted-candidate ladder](ordered-composition-promoted-qualification-20260924.md)
retains its own source, normal and static native evidence; no qualification
transfers automatically to another generated candidate.

| Fresh gate | Receipt SHA-256 | Report SHA-256 |
| --- | --- | --- |
| Normal-source R7 | `5cbd40515467b3648420bc30fa1b52ab36508678fdc0bf7a39bd002e5f276daa` | `c8e17d4b530e6a742bdda90fe1251bd217b4631afcdfe952fef8d7d505810d6a` |
| Repeated-call observer build R1 | `122d9633f498e134a47cfd630a5eaba27f787434ece647fc52404e38f83457d6` | — |
| Native matrix R7 / outer actual-R4 | `7b0c6f2fd5f9039adb8d417ae656e245ee2dd6a214bb42e90b9bf6f12400f329` | `19d99e0aa43d76e07946c46c947eeb08c01bc200c6e1c3058e07763262e848a8` |

The completed matrix is `phase28-ordered-composition-native-r7/report.json`;
its outer receipt is
`logs/phase28-resume-r13-compiler-composition-native-actual-r4/receipt.json`.
The 107,768,560-byte observer has SHA-256
`7aef9c384dcb75becb25f935c0ba596caf733a46b3360f911b43354a9e3faea3`.
Its build-source census and later matrix-source census are intentionally distinct:
only pure Rust lint-test corrections intervened, not C++ or source-profile inputs.

## Historical native failures — preserved

The first ordinary-worker root/O0 attempt produced a retained HSACO but failed
the observer's metadata relation. Read-only diagnosis found two observer errors:
it assumed zero accumulator registers despite LLVM spill allocation, and used
hidden-queue offset 280 instead of the pinned streamer's 232. The retained artifact
reported architectural VGPR count 37, accumulator count 10, combined VGPR count 50
and accumulator start 40. A separate narrow observer correction now checks exact resource-symbol,
metadata and descriptor joins. It does not relax the production machine analyzer.

The first correction passed strict C++ compilation, 24 resource controls and four
Node controls. Its next matrix failed at helper/O0 because the observer also
required dynamic-stack metadata to be false. The actual absolute backend symbols
has_dyn_sized_stack=0 and has_recursion=1 produce true metadata and descriptor
bit 11 through LLVM's exact OR relation. The subsequent narrow correction checks
those exact joins without claiming source recursion or actual stack execution.

That observer passed a fresh strict build, 40 resource controls and four Node
controls. The next matrix completed all 14 ordinary-worker observations. Thirteen
reported exact authored intervals; two-calls/O0 remained incomplete because the
unchanged analyzer rejects a repeated direct callee edge. All 14 passed 18
metadata mutations each (252); the thirteen interval observations passed eleven
decoded mutations each (143). An independent read-only audit joined all child
streams, HSACOs, raw descriptors, resource symbols, 1,196 reported instruction
byte ranges and 36 authored instruction byte ranges; it did not independently
re-decode instruction semantics.

The overall native gate **failed with exit 2**, as required for an incomplete
case. Its final root source/tools/input census did not run. Retained partial
observations and the driver's final rechecks are not a successful outer receipt.
The required successor work was bounded call-site multiplicity plus reconciled
evidence consumers, call-graph cycles/depth and resource propagation. The fresh
gate above qualifies that correction; the historical failure remains unchanged.

Even the fresh fourteen interval observations do not prove complete native helper
argument/result transport, dynamic execution order, root algorithm equivalence,
GPU execution, host buffer validity or protected finalization. Scratch/spill and
dynamic-stack fields are observations, not zero promises.

## Retained evidence

Paths below are relative to the root-owned qualification workspace. Report files
alone are not acceptance; each joins its completed root receipt and exact inputs.

| Gate | Receipt SHA-256 | Report SHA-256 |
| --- | --- | --- |
| Normal R6 | `b1d52c1ca5cc7a2a5a9b8784f70624768b8c76b3ecc51db9a4967ba468b23805` | `b990b00ff41f120a9a564d9a47fcc0746cc4d52bb9029440424d780fa098f204` |
| Publisher R7 | `2108c7a836295fc3d554b08b360eec8fa091983d016d897751f106f1d56122bf` | `b4c1a381035626f3d0e3da1e6563ef5ba72b74f362d469cb9d8cc78880a46230` |
| Public action R1 | `993251eaa4307c1ebe842c79727f09eec59721917eb557d79c7a7602ec44b700` | `e14b788e966c6724c64608b5bd84f1418d5d6eb440d082ba3ee04b51196872bd` |
| Extractor CLI R2 | `7f4852d86291905c11c4fc93805bfdb94301e199970a0c5e2b6fe144db706579` | `a9ec51510506c68afd9859d1b8732044b3cb4c5225b56d214f7126430a9827d1` |

Receipt directories are `logs/phase28-resume-r13-compiler-composition-` followed
by `normal-actual-r6`, `publisher-actual-r7`, `public-action-actual-r1` or
`cli-actual-r2`, each containing `receipt.json`. Reports are respectively
`phase28-ordered-composition-normal-actual-r6/normal-observation.json`,
`phase28-ordered-composition-publisher-actual-r7/observation.json`,
`phase28-ordered-composition-public-action-actual-r1/observation.json` and
`phase28-ordered-composition-cli-actual-r2/observation.json`.

The incomplete native matrix is
`phase28-ordered-composition-native-r6/report.incomplete.json`, SHA-256
`bf029c7ba32601ffd461895bf5fa479dc8863003029242da97289c11ec088434`.
Its failed root receipt is
`logs/phase28-resume-r13-compiler-composition-native-actual-r3/receipt.json`,
SHA-256 `bce944c8335adae906e8d8d0dca2e6c7266bb14d40c2e476156f4fd1c55070b4`.
The independent read-only audit digest is
`d665b954655ed6ceab92166ef1041292a0a59b3d264d84032f1bb3a921918469`.

These receipts qualify their own source/input snapshots, not an unspecified
future main commit. Publication identity is assigned separately after root
integration and final gates. No hardware, source-custody-from-files, protected
artifact, launch-authority or general Rust↔assembly round-trip claim follows.
