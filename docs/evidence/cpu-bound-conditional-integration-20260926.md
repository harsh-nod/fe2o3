# CPU-Bound Conditional Integration

This continues [the component checkpoint](cpu-bound-conditional-components-20260926.md).
It is not 47/47 completion. All [#272](https://github.com/harsh-nod/fe2o3/issues/272)
milestones remain open. Conditional production compilation still refuses before
native output, publication, load and safe GPU launch.

Latest local checkpoint: `9bf3bdded` passed 606 filtered library tests and all
269 documentation tests across the backend, verifier, lowerer and AMD model.
The 35 ignored library tests are not execution evidence. The earlier unfiltered
library timeout remains unresolved; this is not a passing full-workspace run.

## Integrated Boundaries

The live backend now executes and retains one CPU-bound formula V2, then projects
invocation V2 only after the original source/target account postchecks. The
existing receipt JSON remains unchanged. Outer descriptor V5, its FFI owner and
structural finalizer accept invocation V2; V4 remains tied to invocation V1.
These structural APIs do not manufacture compiler or machine-proof authority.

The opt-in genuine V2 suite defines 54 ordered cases per target, including strict
reimport and CPU/source/policy substitutions. Its sidecar is joined to the actual
serialized contract by the Rust parent. Only four cases use the original live
account; the other fifty are explicitly component-only. This suite compiled but
has not run in the protected runtime. Its separate runner passed a static scope
audit and 144 offline tests, which do not establish compiler or proof execution.

The packet layer is integrated and its local component tests pass. Genuine
protected integration tests have not executed:

- The bounded V2 codec transports complete source, original native-neutral IR,
  ordered roots, CPU inputs and inert receipt signatures. It grants no authority.
- The independent consumer replays source/IR correspondence, exact root and
  launch associations, recipe ordinals, source rows and CPU content. Accepted
  policies come from the caller, not transported keys. Existing lower requests
  and one strict V2 import per root retain the real arenas and proof owners.
- Borrowed-policy staging reuses the existing admission engine without cloning
  the accepted policy. Ordinary V1 paths retain their original behavior.
- Producer hooks retain that original policy and optionally capture the real
  CPU frame, staging commitments and formula signature during the existing
  replay. Replacement compares both charged owners before dropping the old one.
  Complete roster borrows finish the original source-phase postchecks first.

Producer hooks add 15 passing component tests. Conditional roots now retain
an inline policy header, and their existing type-sized scratch charge grows;
conditional/fixed6 storage thresholds are therefore not claimed unchanged.
Non-capturing replay performs no new CPU encoding or proof work. Ordinary routes
and all resource caps are unchanged. The conditional F entry now assembles and
attaches the complete independently replayed packet after both original account
postchecks. Opaque failures retain terminal charges through the outer wire and
capsule entries; only the unchanged finalizer refusal follows successful custody.

Two additional checked boundaries passed local component tests. The lowerer
accepts a genuine source request and sealed
independently checked optimization history, checks exact source/target subjects
and external limits, and follows every conditional memory occurrence through F.
The typed V5 adapter shares the existing V3 physical ABI, capability and native
text engine, comparing the complete descriptor section without a V3 downgrade.
It establishes content agreement only; a coherently changed contract must still
be rejected by the separate source/CPU/formula contract join.

The strict importer now lends its actual V2 execution during the same visit,
with graph and account postchecks before callback results escape. Its 13 component
tests pass. The private contract checker compares theorem fields, CPU/staging
commitments, ordered roots/read occurrences/premises, source arguments and physical
descriptor layout against that request and execution. Its first 17 component
tests pass; five additional full-field layout tests also pass at `7648f5e31`.

The private source-through-F composition checks the independently accepted history
limits, source/target coordinates, complete root roster, each root's final-history
and contract agreement in that same import, then the V5/native-text relation. It
returns only existing source-content custody, not final artifact authority. Review
found and fixed a missing fifth simultaneously live account-header charge, adding
an exact/one-short regression. The composition compiles and all 12 new component
tests pass at `7648f5e31`. The production caller now invokes this composition
during the existing packet visit, retaining original nominal/context custody
alongside actual F, its history, the V5 descriptor and native text. It does not
rerun the optimizer or import the packet twice. Successful content checks still
end at `ConditionalFinalizerRequired`; machine refinement and native authority
remain separate required boundaries.

Test-only observations compare retained packet/history/catalog/descriptor/text
bytes and the actual history roles, source, contracts and formula reports. Their
call/completion/installation state tracks the outer composition, not an internal
strict-import count. Two observation component tests passed at `1a4a02120`; the
genuine protected child has not executed.

V5 now explicitly checks original Rust layout evidence for every argument using
the existing physical-layout validator. This closes the all-slice case skipped
by the nominal-only packing gate. Missing evidence and valid-but-wrong donor
layouts reject; the separate usize/isize evidence convention is preserved.
Ordinary V3 behavior and resource charges are unchanged.

The consumer supports a complete conditional root roster, not mixed ordinary or
UnitLocal roots. Existing per-root single-output and CPU-control-flow limits
remain. Registration origin and launch max_grid are external facts. Declared
resource receipts do not constitute exact whole-process heap accounting.

## Completed Local Checks

Results belong to the stated source snapshots, not subsequent changes. Cargo
used pinned nightly 2026-04-03, locked offline dependencies, one job/test thread,
hidden GPUs and the unchanged resource limits.

| Snapshot | Check | Result |
| --- | --- | --- |
| `08082add8` | descriptor V5 | 105 unit/integration and 13 compile-fail passed |
| `08082add8` | FFI | 165 unit/integration and 25 documentation tests passed |
| `f8e690170` | full finalizer run | 347 passed, 3 failed, 26 ignored; docs not reached |
| `fd7f41eb2` | corrected finalizer cases | 3 identity and 7 V5 publication tests passed |
| `fd7f41eb2` | finalizer documentation | 53 compile-fail and 1 V3 positive control passed |
| `fd7f41eb2` | focused backend regressions | 170 passed, 3 protected tests ignored |
| `fd7f41eb2` | backend and verifier capture | both compiled successfully |
| `41b641ca7` | packet-integrated backend library | bounded offline Cargo check passed |
| `50a696f75` | backend, verifier, lowerer and AMD model test targets | bounded offline Cargo check passed; no test functions executed |
| `13e0da0f2` | conditional verifier/lowerer/AMD-model library tests | 260 passed, 1 development-Verus test ignored |
| `13e0da0f2` | complete AMD-model library | 225 passed, 3 inert fixture exporters ignored |
| `13e0da0f2` | backend conditional library tests | 156 passed, 29 capture/protected tests ignored |
| `13e0da0f2` | kernel compile matrix shell harness | 79 manifest, 12 occurrence, 56 identity and 28 fixture-binding tests passed, plus shell controls |
| `acda59d9f` | contract checker components | 17 passed; all four affected library test targets compiled |
| `7648f5e31` | integrated conditional library suite, all four packages | 450 passed, 30 capture/proof tests ignored |
| `7648f5e31` | all four packages' documentation tests | 264 compile-fail and 5 positive tests passed; none ignored |
| `2ddfdc4fc` | unfiltered library tests, all four packages | timed out at the unchanged 1,200-second limit; AMD model completed 225 passed/3 ignored, lowerer incomplete, verifier/backend not reached |
| `1a4a02120` | production-bridge conditional library suite | 463 passed, 15 failed, 30 ignored; not a passing run |
| `1a4a02120` | legacy nominal library regression filter | 121 passed, 5 actual-source tests ignored; new conditional-descriptor tests explicitly excluded |
| `1a4a02120` | all four packages' documentation tests | 264 compile-fail and 5 positive tests passed; none ignored |
| `45ad533b9` | corrected conditional library suite | 485 passed, 30 ignored, zero failures |
| `9bf3bdded` | conditional/nominal and targeted shared-fixture library regressions | 606 passed, 35 ignored, zero failures; no nominal exclusion |
| `9bf3bdded` | all four packages' documentation tests | 264 compile-fail and 5 positive tests passed; none ignored |

The shell harness uses fake Cargo/ROCm controls, not actual tutorial compilation
or hardware. Filtered and ignored tests receive no execution credit. These local
results do not include genuine imported proofs. Log SHA256s for the five rows
from `13e0da0f2` through `acda59d9f`, in order:

```text
007e967ba1b1088725e0063bb4f4308472efba52c5c8bea6cdc7e61a57727dc5
5b56a819cc62dcc4f633b179a93a94e90f61e583b3dab6ec2384fb4720a5d639
cd62839f07d89dcc6c8f38083c08ed7fb7391a2b207be82f8fc9c4f0ea38a5db
03f731635168d4bad37362119ea29f3f77ddaecfe41affb3e4afbfce056cc375
d77ad3f653084653de66c1f0bc770580e22037a4ffab1d7eaf0404d670389d2b
```

The integrated library log SHA256 is
`4b9042087393de1337f03a60b9340c15ae9b71df9b9c64c4b63a1b8cf3964921`;
the documentation log SHA256 is
`daed0a79c68a89ade2cb0591fce3fb625f52a244dbbb7d13094984eb17eb30e1`.
The 450 passing tests comprise 16 AMD-model, 139 lowerer, 139 verifier and 156
backend cases. Counts overlap earlier snapshots and must not be added together.
The private same-import hook has a compiled caller-shape control and component
tests, but no external borrow-escape compile-fail test; an external import of
that private function would test privacy instead of the callback's lifetime.

The production-bridge run exposed three shared fixture errors: raw pointers
inherited reference-only non-null validity, a positive fixture used a device-export
role rejected by V12, and the history fixture configured the generic 2 GiB ceiling
instead of the native history ceiling of 256 MiB. Corrections preserve the
negative cases, existing production limits and original resource account. The
native ceiling is selected before source/optimizer work, never by replacing an
account after work has begun. All 15 failures are resolved in the passing
`45ad533b9` run. The failed run remains part of the evidence. Its log SHA256 is
`b3f3d02e51b33e6146eadb2cf3acab548572d0f380275cb9a77b6465e1ddb294`.
The later documentation log SHA256 is
`8524c8d5da762702fe46f8b11d8595f10c7eeb993e92c1d7bebfc4b4f7002d34`.

The expanded `9bf3bdded` run comprises 31 AMD-model, 171 lowerer, 150 verifier
and 254 backend passes. It includes all nine targeted default-fixture regressions
and the nominal tests without the earlier exclusion. Filters overlap; counts
must not be added to earlier runs. The protected child's assertions now preserve
the typed cause, bridge state and resource snapshot on failure, but that child
has not executed. Log SHA256s, in order, for corrected conditional tests, expanded
regressions and latest documentation tests:

```text
581b3177d50e8394b8de03529ead0dcce908ebbed3afd6ac9365c747e20fc2ba
551a943c0e61242e7f9f40cce5f086144d85ce27e2f0f0ee3fd9914877247ce9
bd7a751f5145ea21f0654fa59ad23aaa669d8567ed3aa0ccc4ff87d520ff75f8
```

Earlier failed runs are preserved. The merged lockfile required matching pending
manifest contract digests and the dependent curriculum/inventory snapshots; no
proof status or expected kernel output changed. One resource test wrongly expected
`usize::MAX` to overflow a fresh `usize::MAX` meter; the corrected test charges a
prefix first, matching the concurrent public-main correction. The corrected local
runs above passed. Public-main CI `36220960215` failed the old inventory
snapshot; its five codegen shards and parity job passed, but that is not a green
integrated workflow or protected-runtime evidence.

Both public main branches subsequently advanced to `123a2c59` with the same
inventory correction and concurrent BF16 work. That commit is not the tested
local integration snapshot. Normal Git fetch currently fails DNS here, so the
new local integration commits have not been merged with it or pushed. Preserve
that concurrent work; do not force-push the local branch over either main.

The `50a696f75` check fixes a missing descriptor backing/view lifetime bound
found in the preceding `3f8a6dcff` attempt. Those terminal checks used the normal
terminal path, not the protected-capture guard. The guard's Node/Git subprocess
failure remains recorded; these checks cannot supply protected capture artifacts.

The failed finalizer run remains failed. Two fixtures used invalid crate names;
another expected rejection after substituting identical bytes. Corrections retain
the distinct-input rejection tests and add the identical-byte positive control.
No production validator was weakened. The full suite has not been rerun.

The fd7 source inventory was 8,282 files, SHA256
`63356188e4c09035c8a5275b40b87835400f3ce78d7b5daa64c6b5918bb918e8`.
Focused backend log SHA256:
`b6ff874197e39520b760ea9c22e636aef5e2fb7e76fa09e929d5d420694ed697`.
Finalizer correction log SHA256:
`a629ad5ed23f5cd593617a0a5d5bd3c2b044f278ac893c6e8e74e109cb6cd2d5`.

## Remaining Gates

An environment reset interrupted the subsequent real-invocation preparation and
removed its RAM-backed artifacts. No completion guard exists for that capture.
Fresh matching compiler/verifier/preparation artifacts are required. No new
protected run or GPU job was started. The two newly created, empty remote scratch
directories await cleanup when SSH is available; the shared runtime is unchanged.

Packet integration has passed static review, formatting, typechecks and component
tests, not protected tests. Review fixed test-module paths and an opaque-token
assertion; hygiene
required the established test filename convention, not a policy exemption.
Genuine multi-root packet and source/CPU/contract/final-history success, full-chain
resource negatives and protected execution of the new production wiring remain
required, followed by native-machine custody,
publication, generated host admission and the complete tutorial hardware matrix.
The conditional native handoff and restart-recovery continuation are still
implementation work, not merely unexecuted tests. Existing ordinary capsule
schemas cannot be reinterpreted as conditional authority.
Shared-IEEE source proof is not LLVM/ISA arithmetic or a transcendental error-bound
proof. Neither clean analysis nor V5 bytes bypass the existing authority gate.
