# Bounded authored instruction-program qualification

Observed on SSH host `mi350-2` on 2026-09-19 UTC. The actual-source, debugger
and native runs used clean compiler commit
`f5e81f985ff3e2771ad0f132d483f5cf74976ad6`. This document is a later evidence
summary, not compiler-closure attestation or a protected publication receipt.

## Acceptance and limits

The #280 M1 exit is met for the current integer instruction slice: an ordinary
authored Rust kernel retains source/canonical identities, executes against an
independent result oracle, rejects illegal inputs, and reaches independently
decoded native instructions. Existing production blockers remain visible.
This does not close #280 or its other milestones.

The exact profile is gfx942:xnack-, wave64, required and maximum workgroup
64x1x1, one unconditional acyclic direct-root program, one to sixteen u32
move/wrapping-add/wrapping-subtract/AND/OR/XOR steps, and five distinct v0..v63
bindings. The sixteen-step source covers all six instructions, repeated writes,
an unused write and a self-move. Used and unused-result variants are separate
actual source exports. This is one constrained LLVM inline-assembly unit;
surrounding code and permitted boundary moves remain compiler-owned.

Whole physical-register kernels, branches, authored memory/synchronization,
gfx950/matrix semantics, physical lifetimes, source-owned promotion, protected
final artifact admission and GPU execution remain outside this qualification.
The two-architecture catalog is metadata, not executable coverage.

## Actual retained observations

| Run | Observed result | Receipt SHA-256 |
| --- | --- | --- |
| `phase9-ordinary-source-r1` | Six real source exports; eight exact source refusals; 36 CPU cases checking all 64 output words, initialization and both canaries; 15 ordinary CLI negative controls. | `e0efff8694beb6e9decebe3f95f5ba0940aecdc63e06dc58941ef985401dde9f` |
| `phase9-public-debugger-r1` | 36 real JSONL sessions, 1,224 request/response pairs; lane-zero logical before/after/reverse/repeat checks, unused-result retention, stale-state refusals, full output memory and write-history checks. | `7b61dfb1f581830779da2352afbcf3589b437f6d62dd3653abf7809201167893` |
| `phase10-source-native-r1` | All six retained source exports joined to six LLVM emissions and six native processes; O0/O3 give 12 compilation cases and 80 independently decoded authored instruction sites. | `efc8acc2bc4da9d20ba3022ab855a5e80c5b8bbb716ac48d4e0d76823277206f` |

The source refusals cover aliased or dynamic physical roles, divergent
placement, wrong launch bounds, invalid count/opcode, read-before-initialization
and nonzero unused descriptor padding. CLI negatives include wrong wire,
launch/request errors, incompatible selectors, unsupported persisted scheduling
and create-new output preservation. Expected semantic refusals require their
specific diagnostics; setup failures are not counted as successful negatives.

The native join rechecks exact source-export, KIR, LLVM, tool and process
observations, instruction order/words/register operands and descriptor capacity.
Its 24 cache-census subprocesses are not additional native compilation cases.
The LLVM file alone does not authenticate source ancestry. Native payloads have
retained digests and decoded observations, not retained complete HSACO files.
No resulting object was loaded or launched.

Debugger values are logical CPU values. The whole program is one simulator
operation, not sixteen instruction microsteps. Declared VGPR roles are not
captured physical contents. Scratch intermediates, unqueried lanes, EXEC,
allocator lifetimes and hardware timing remain unavailable. Existing frame
depths and constant occurrence fields do not establish dynamic helper activation
identity. Reverse navigation does not undo compilation or authorize mutation.

The task-owned source/debugger orchestration and their original logs are retained
under `/home/harmenon/fe2o3-authoring-280-282.FEW3gj` on the named host. The
checked-in native join is
[ordered-program-source-native.mjs](../../scripts/ordered-program-source-native.mjs).
The [source contract](../ordered-program-authoring-v1.md) links the actual fixture;
the companion tutorials give ordinary exporter/simulator/debugger commands.
Task harnesses and observation JSON are not new public compiler protocols.

## Compiler and tool qualification

The pinned compiler is nightly-2026-04-03, rustc
`55e86c996809902e8bbad512cfb4d2c18be446d9` (1.96.0-nightly). Native inspection
uses the retained ROCm 7.2.1 LLVM 22.0.0git worker. Node is 22.22.3.
Source and executable measurements bracket the actual runs.

The shared Rust/Cargo/crate-README census is
`8476068626a78e4678291c7777899bb055d331469c1a516bcbda02228cf54dfd`:
3,685 files, 72,847,018 bytes. It is a scoped change detector, not a complete
toolchain/dependency-closure attestation. It is unchanged between the tested
`cd018579` code and the later fixture-inventory/documentation-only commit above.

The following gates pass; overlapping focused and aggregate counts are not added:

- Twelve library packages: 3,809 passes, 15 explicit ignores
  (`phase9-libs-r13`).
- Backend library/binaries: 1,213 passes, 36 explicit ignores
  (`phase9-backend-r13`).
- Simulator/debugger CLI suites: 258 passes, three ignores; documentation:
  219 passes; inspection examples: 16 passes; V16/V17 simulator compatibility:
  33 passes.
- Device and source-authoring library/binary/integration suites; frozen MIR/KIR
  and scalar assembly compatibility; moved local-order simulator integration.
- V17 executable CLI integration: six tests including 36 real CLI positives
  and negative classes. These inputs are synthetic canonical fixtures, separate
  from the actual-source batch above.
- Formatting, dependency policy, shard policy, all 34 standalone locks,
  quickstart script controls and actual vecadd source-check/host tests, and
  unsafe-source inventory. The extra workgroup integration target compiled
  but all three tests were ignored; the ranked target passed eight tests with
  29 ignores. Neither is additional actual-source qualification.
- Pure runner controls: 17 source-oracle, 17 debugger, 15 native-program and
  16 source-to-native join controls. The earlier synthetic native run passed
  12 cases/80 sites, but is not counted again as actual-source evidence.

The final normal backend library, four binaries and three inspection/emission
examples were rebuilt after test-feature builds, before the real source batch.
Strict Clippy, the entire repository's opt-in tests, protected final admission
and hardware qualification are not claimed. Existing warnings remain.

## Retained failures and corrections

Intermediate failures remain in the host logs: merged match/test-census
adaptations; old scalar terminal goldens; stale CLI help; formatting and nested
lock drift; and a task-runner DCO invocation missing Python isolation.
The quickstart harness initially supplied a forbidden RUSTC override and a
symlinked target path; it now lets the CLI select the pinned compiler and passes
the canonical cache path. No compiler-selection/path check was weakened.

The unsafe inventory found the already-published Wave64 capture negative
fixture absent from its baseline. Review confirmed one deliberately unsafe
call template and three unreachable unsafe-signature lookalikes used by
source-authentication negatives. The baseline and rationale were reconciled
without changing fixture code or source admission. See
[the unsafe-code policy](../unsafe-code-policy.md).

## Resource and publication boundaries

All builds and observations ran on the requested host, with Cargo jobs=2 and
serialized builds. Guards retain a 40 GiB free-disk reserve, a 64 GiB available
RAM floor and a 20 GiB combined task-cache cap. Source, retained evidence and
other agents' work are not disposable caches. Current caches use durable
task-owned storage; earlier vanished RAM-cache observations remain historical.

Normal signed-off main publication and remote-ref readback are separate from
test execution. Website captures retain their original compiler/source pins;
new observations do not relabel the historical V5/V6 resource captures.
No curriculum release pin, generated-host or protected-runtime gate is advanced
by this instruction-slice qualification.
