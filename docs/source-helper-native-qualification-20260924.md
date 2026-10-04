# Source-authored helper compilation qualification, 2026-09-24

This bounded continuation of #280/#282 connects generated helper source to the
normal ranked/formal compiler path, typed descriptor construction and the
ordinary pinned LLVM/object/LLD worker. It does not close an umbrella milestone,
authenticate a protected final artifact, or establish GPU functional correctness.

## Implemented route

The existing public helper materializer keeps actual source call occurrences,
specializations, original root arguments and launch shape. The source and native
helper projections support the six admitted scalar u32 NoMemory instructions
(MOV, ADD, SUB, AND, OR, XOR), including an ordinary singleton u32 return tuple.
Checked replay retains the exact native SSA operands, not merely equal values.

Normal target lowering now retains its actual fixed-policy optimizer output,
canonical owner, report and original versioned source identity through descriptor
construction. Formal replay and the complete mandatory ranked-check roster remain
required. Only genuine internal helpers with the exact gfx942:xnack-/Wave64,
six-u32 NoMemory profile admit the structural assembly capability at this boundary.
Canonical capabilities remain unchanged; the runtime descriptor projects execution
requirements. Generic descriptor callers and complete-body-only admission still
refuse that capability. A cloned module, foreign owner, hidden memory effect,
unknown extension, altered target or root-owned inline operation cannot obtain
this helper admission.

This retention uses the existing bounded optimizer allocation domain. It adds no
graph clone or second optimizer replay; it is not an aggregate RSS bound or a new
semantic-preservation proof for the optimizer.

## Actual source and normal driver results

On mi350, the fresh public workflow produced five source exports and passed
150 whole-kernel CPU replay cases plus three exact CLI refusal controls. The four
generated variants are default256, edited512, its exact repeat, and two helpers.
These actual-source/native examples use VOR only; the six-operation inert tests
are not six actual-source/native qualifications.

All eight subsequent normal-driver runs passed: LLVM emission and normal inert
worker handoff for each of the four variants, with exact unchanged LLVM prefix,
canonical descriptor extension, symbol roster and source/ABI joins. The kernel
keeps its four physical ptr/len/a/b components and original launch configuration.
Physical helper registers and ordinary helper ABI remain compiler-selected.

Evidence retained by the root runner:

- Public workflow gate: 4cd02f850e8761f0f60a425bce0a0eae27208d8229ba99fb1f12590cffeafce9.
  Public receipt: 410379 bytes, 7c419ab2dc2e8d555f5dc34e47243d2862e0714a5c5737fadf92a35fd295cd14.
- Normal-driver gate: c6c57251bc11ff6f338b98a7790c9376f06471e7ff8c757b002befd8eba4115d.
  Observation: 38570 bytes, 6d5b36d38ce88926c86db738f1ede8200cd257dd19d79937674b335732e09b08.
- Descriptor controls: 124 passed, including ten new helper tests and three
  existing complete-body controls; fixed optimizer architecture control passed.
  Gate: d1e3641896cf3a9ab4ee9245d0e29f501f08514e6205e2cc245a5520aeafd1e3.
- Regression run: 1581 lowering tests, 1968 backend tests, 194 debugger-library
  tests and 24 controller-example tests passed; 132 existing library tests were
  ignored, not qualified. Gate:
  4e33a475330d68a3da0cbcbf101b62c686697e7d89631078e9e0ad7c920587f8.

## Native observations, not a native semantic proof

The new helper-source-abi fixture consumes those actual unchanged LLVM bytes,
checks the exact source helper signature/call/VOR/return relations, invokes the
ordinary worker and retains bounded decoded functions, blocks, instructions,
effects, exact instruction bytes and direct-call targets. All eight O0/O3
observations passed with the pinned ROCm 7.2.1 LLVM 22.0.0git worker.

| Variant | Optimization | HSACO bytes | Functions | Instructions | Direct calls |
| --- | --- | ---: | ---: | ---: | ---: |
| default256 | O0 | 6616 | 2 | 122 | 1 |
| default256 | O3 | 5408 | 1 | 20 | 0 |
| edited512 | O0 | 6616 | 2 | 122 | 1 |
| edited512 | O3 | 5408 | 1 | 20 | 0 |
| repeat | O0 | 6616 | 2 | 122 | 1 |
| repeat | O3 | 5408 | 1 | 20 | 0 |
| two | O0 | 6992 | 3 | 160 | 2 |
| two | O3 | 5408 | 1 | 23 | 0 |

The repeat HSACO is byte-identical to edited512 at each optimization level.
O0 retains calls; O3 inlines them. Rust inline(never) is not currently propagated
as LLVM noinline here, so the test does not mislabel inlining as an ABI failure.

The initial observer report held borrowed LLVM symbol strings after their module
was destroyed. Root report inspection caught the invalid records. The fixture now
owns those names and rechecks exact helper/intrinsic rosters after module
destruction and after native compilation, with positive and mutation controls.
The original eight reports are retained but excluded from qualification. Only
the fresh r2 observations above count.

Strict native rebuild gate:
d769741823a978a476046331a8c0915657e8fc4f911d6ffbf07fa8f2bd781c0f.
Observer executable: 107722104 bytes,
58ae63a4f77d164863f910c206c92ac177b20e381a0255a87a3857cb1a38d349.
Fresh eight-case native gate:
f8c26fc18b6b7f19acc4a3a312e000f6678dc674350e30a1b10506dd73933b52.
The SDK's 2517-file closure was reverified around the native work.

## Remaining limits

Native instruction counts and VOR presence are not proof of physical helper
argument/result flow, register preservation, the complete root expression,
bounds/store safety, race freedom or functional refinement. Those qualification
flags remain false. No GPU dispatch, live register capture, protected V3/finalizer
admission, complete physical-entry source authoring or milestone closure is claimed.
The independent native matcher and actual production/finalizer continuation remain
required. See the neighboring fixture README for reproduction arguments and limits.

The lint inventory is not a strict workspace lint pass. The private error-transport
refactor reduces the new complete-body large-error sites from fourteen to two
public methods without boxing or losing typed errors; those two warnings remain.
