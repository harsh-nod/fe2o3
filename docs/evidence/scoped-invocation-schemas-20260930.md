# Scoped Invocation Schema Checkpoint

This is #272 integration evidence, not production activation. M0 is complete;
M1-M7 remain incomplete and strict production-to-safe-GPU-launch coverage is
**0/47**. The compiler candidate is separate from public main.

## Contract

Candidate `1855225e000789d2647373ff0e9c6fba6f119b68` extends existing schema
families, not a second executable graph:

| Family | Extension |
| --- | --- |
| MIR V41 | Exact V40 extension; nominal roles 20-21, intrinsic tags 102-105 |
| KIR V23 | Exact checked-storage V18 extension; role tags 14-15, operations 48-52 |
| SO V4 | Execution family 6, opcodes 7-11 |
| Combined source V10 | Four source terminal tags 154-157 |

Invocation derivation retains the original context. Its scoped index is not an
ordinary scalar index; reading coordinates yields data without consuming the
witness. Checked mutable access consumes the witness and must retain its ancestry
on the resulting pointer. Scope closure follows the original context borrow and
surviving descendants, not a temporary handle's destruction. The synthetic scope
end has no public source terminal.

These records preserve identities and payloads only. Existing consumers refuse
unsupported scoped operations. Frontend recognition, original SSA/loan and helper
correspondence, pointer ancestry, lifecycle verification, optimizer preservation
and executable lowering must agree before any production policy is enabled.
Neither successful encoding nor a well-shaped ABI proves these properties.

## Executed Tests

The guarded runner used pinned nightly-2026-04-03, locked/offline dependencies,
one Cargo job, serial tests and disabled GPU visibility. Whole source/tool
inventories were unchanged during every run.

| Run | Result |
| --- | --- |
| r149 | Compile failure: missing early return and opaque-identity Debug assertions; no tests executed |
| r150 | KIR 1,348 passed; MIR 397 passed and eight failed |
| r151 | KIR 1,348 passed; MIR 405 passed; zero failures or ignored tests |

The r150 failures were three non-rooted schema fixtures and five obsolete
unknown-version assertions. Corrected fixtures now explicitly test the omitted
type/callable rejection too. V41 is recognized but still rejected at the exact
V35 boundary; V42 remains unknown. No closure or historical-version check was
weakened. The full r151 result is **1,753 passing tests**, including 14 new scoped
MIR/KIR tests for encoding, typed get_mut ABI, malformed payloads, profile
separation and exact/one-short accounting. Source terminal/identity tests belong
to the compiler crate and were not executed by these two suites.

r151 source snapshot SHA-256:
`022f65ed4642c23e24ae24325795ae9c88a55635c95ee554c00684c05903ceaf`.
Log SHA-256 values:

- r149: `fadeb22e7ab1086ad3c385cb7049282f70bb7ad958ba86d9806eacc55e26b0e8`
- r150: `3b4918e50995262b15e018c5d0d999b9ebe12e65aa4350f8ccc58f5202fdca8a`
- r151: `9f7c7d754f77164146f8312ac2f763630ab410962418bf5dc9a9fa2147a43ccd`

Five signed candidate commits pass DCO and the repository hygiene delta policy.
The whole compiler dependency check and genuine frontend tests remain separate
requirements. No protected runtime, proof execution, simulator or GPU ran here.

## Compiler-Wide Check

r152 checked all targets of `rustc-codegen-fe2o3` at the r151 candidate. It
failed on a legacy tile-lifecycle match that omitted the five new scoped
operations. `50fa0245cb25edbe4eb5c6482d2b2ba59e6f6d1e` adds explicit rejection
at that unsupported consumer; it does not activate scoped execution there.

r153 repeated the compiler-wide check at that fix and reached the compiler
crate, where it failed with nine errors: eight references to
`fe2o3_hsaco_finalize` without a normal library dependency, and an importer
match missing the four new source expansions. The finalizer was only declared
as a dev-dependency. Its dependency promotion and the frontend integration
require another check. Neither failed run executed compiler tests.

Both runs retained unchanged source/tool inventories. Log SHA-256:

- r152: `041105265af6ea02013504f56bf7f3ace14e2110a8eb40a83a4ef36ff8f013ae`
- r153: `7b0201d4d259adc790b90f06dfa760a66f5c6106aff847adc1adf1affdbda05a`

## Frontend Candidate

Candidate `40bbf819bac3ace4780d67407ec9d934df9cee01` adds exact-instance
classification, all four scoped source expansions, versioned source-body
import and corresponding identity/ABI fixtures. The same `get_mut` method is
distinguished by its authenticated concrete argument types, not its name.
The finalizer dependency promotion and the missing importer arms are included.
This does not enable the production profile or establish source lifetime proof.

r156 checked all compiler targets at that candidate. It failed with one E0282
in the new test's untyped signature ordinal; compiler tests did not execute.
The source and tool inventories remained unchanged. The follow-up commit
`246388fd5` annotates that ordinal as `usize`; it needs a fresh check.

r156 log SHA-256:
`8cda82b276742502a57a7bccff013ba1881b0497150483462b3d0ef6f88b4a4c`.

r158 repeated the all-target compiler check at
`246388fd527b9b43a431cd3e8c0824d72b4b8b85` and **passed** with unchanged
source/tool inventories. This confirms compilation of the frontend and test
targets, not execution of the compiler tests or activation of scoped production
lowering. The genuine source fixtures and lifecycle integration remain due.

r158 log SHA-256:
`062cabdb6fa4220fd30bc97cfa14c6530953542d180a095bb8c28bdd492259df`.

## Same-Graph Lifecycle Verification

The scoped lifecycle candidate extends the existing metered execution-state
table with exact context/invocation/index ancestry and the checked pointer's
original presence result. Coordinate reads leave the witness live; checked
mutable access consumes it and transfers the dependency to the pointer.
Scope closure requires the exact sorted live-descendant roster. CFG joins and
backedges require equal states. Unsupported pointer aliases, retained calls
and CFG-argument transport refuse instead of losing that ancestry.

The explicit `verify_scoped_storage_module_ref_with_budget_v23` entry uses
the existing function pass and original graph. It grants no canonical V23
owner, source correspondence, proof or launch authority. Historical V18
admission continues to reject the new profile.

| Run | Candidate | Kernel-IR Library Result |
| --- | --- | --- |
| r159 | `3a4b8fc1b7c23f7da7c4280e637d13f8da66e2d6` | 1,371 passed, one failed, none ignored |
| r160 | `719ac186c54308aabb7db8201e55abaab0728b79` | 1,372 passed, none failed or ignored |

The sole r159 failure expected historical canonical admission to reject during
verification. It actually rejects earlier while encoding the unsupported
scoped role. The corrected assertion requires exactly version 18's
`UnsupportedInVersion` encode error; the separate storage-verifier rejection
assertion remains unchanged. No production acceptance condition was relaxed.

All 24 new scoped controls pass in r160. These include independent legacy work
schedule assertions, not only limits measured from the new implementation:
39 units for a minimal context-only lifecycle and nine for ordinary role
operand rejection. Scoped-only discovery is folded into the existing scan;
legacy role uses do not incur pointer-ancestry work charges.

Source/tool inventories remained unchanged in both runs. Log SHA-256:

- r159: `9f0b4f11d0047453962392bd006c78bd0242a74b660d8462e1d8e99ebce564d9`
- r160: `5a79fb7f1632630d7a068e5c161211e4bf3d317354c6d913aa2009214d1a2b4b`

Canonical-owner/Pliron integration and authentic source-scope emission still
remain necessary. These library results do not advance M1 or **0/47**.

## Canonical Scoped Owner

Candidate `82d9bb10b846dfb94fb07bc765d386be897832ba` adds a distinct move-only
V23 canonical owner, identity, receipt and scoped borrow. Admission, resource
accounting, copying and storage queries share a private closed-profile engine
with V18. The V23 owner retains its actual checked graph and uses its own codec
and hash domain; it is never converted to a V18 owner. Existing V18 bytes,
domains, work schedules and unsupported-profile rejection remain separate.

| Run | Scope | Result |
| --- | --- | --- |
| r163 | Kernel-IR library | 1,389 passed, one failed |
| r165 | Complete Kernel-IR package, including doc tests | 1,974 passed, two failed, one ignored |
| r167 | Corrected semantic-operation integration target | 23 passed, none failed or ignored |

r163 exposed a test helper missing the nested decode/encode work-limit error;
the corrected control still requires that exact refusal. All 1,390 library
tests and 73 doc tests passed in r165, including 18 new owner/resource tests and
five new compile-fail examples. Its only failures were two stale assertions
that schema version 4 was unknown. They now require version 4's exact
noncanonical-payload refusal and independently reject unknown version 5.
r167 executed that complete 23-test target at the committed candidate. The
full package was not rerun after the two assertion corrections. The ignored
test exercises the maximum V1 block-count boundary.

All three runs retained unchanged source/tool inventories. Log SHA-256:

- r163: `5b82a4ac35b5b96550d4fb44191b1c48e9a9c41ec2eb1d94fb5686205962a484`
- r165: `e15631f1ecbfe19e244f9cdbfc06e893cc30d2009de80304c422be37846dc84c`
- r167: `8444c28cb45dfd5b8cd89682b8cc8c7ee966110410ba45a1077905f03f376703`

At that checkpoint the frontend candidate incorporated this owner, but its combined execution
tests, Pliron bridge and actual source-scope emission were unfinished. No
default production activation, protected proof, simulator or GPU credit follows.

## Scoped Pliron Bridge

Candidate `25f5d69168280a734e3359c205b668c662644699` carries the actual V23
owner through the shared storage-profile import/extraction engine. V18 and V23
remain distinct nominal owners with separate identities, epochs and ledgers.
Extraction retains actual operands and scope-close payloads, then readmits the
result as V23. No conversion to a V18 graph or new production route is added.

| Run | Scope | Result |
| --- | --- | --- |
| r183 | Initial bridge candidate | Compilation failed; no tests executed |
| r185 | Complete dialect/Pliron libraries | 2,446 passed, none failed or ignored |
| r187 | Preserved execution dialect integration | Four passed, none failed or ignored |
| r188 | V23 bridge compile-fail examples | Three passed |

r183 exposed an ambiguous macro `core` import. Explicitly naming the existing
bridge core resolved it. r185 covers 76 dialect and 2,370 Pliron tests, including
all 12 new scoped bridge controls. The latter exercise actual operand extraction,
scope-close/use-after-close rejection, owner identity and resource boundaries.
Historical bridge behavior remains in the same full library run. DCO for the
two bridge commits, whitespace and hygiene-delta checks passed.

The final three runs retained unchanged source/tool inventories, source snapshot
`2b16aeab01eec23ba6aaa79c3adfcaac3fb651c1ae4b35d61f998a9cbc69cce8`.
Log SHA-256:

- r183: `9cb07e7ca77654a9aac71f6fb89b5bb58a154b524d70ac323c65a1fe4fb3c286`
- r185: `a34c9667b3b498effecd1d98f89053f890cd8deb5b6c328bbb58d41e3652b002`
- r187: `1353bc0265b0ec3c99bb72192cda474df44f0a02164e013f6deacc7d5092afd9`
- r188: `debbd98d98408db81797298807bd6fc762f37b83682445ef7a0fec8613009607`

## Typed Scoped Inventory

Candidate `6099f1a3dd3609be82f310df2c821b9bcc6f661d` adds
`CanonicalKirInventoryV23` by instantiating the existing private inventory
engine with the actual V23 owner. Its graph and operand records borrow that
owner; no executable graph is copied. Equal canonical bytes do not substitute
another owner's borrow. Existing V12/V18 constructors and schedules are unchanged.

| Run | Scope | Result |
| --- | --- | --- |
| r189 | Eight new V23 inventory controls | Eight passed |
| r190 | Complete kernel-analysis library | 687 passed, none failed or ignored |
| r191 | V23 inventory documentation | Eight compile-fail and one compile-only example passed |

The eight controls cover scoped operations and their exact uses/close roster,
parallel edge occurrences, scalar definitions, equal-content foreign owners,
legacy-profile work/storage agreement, exact and one-short resource limits,
prior denials, owned storage windows, and retained-size query accounting.
r190 includes r189's eight tests; they are not additional distinct passes.
The compile-only example is not runtime execution.

All runs retained unchanged source/tool inventories, source snapshot
`d55bc997bff0c5dedc546d93caeabdfdb0db48adbecc25915909d7506fb4dbce`.
Log SHA-256:

- r189: `a44cf2599e995a181c731447fd2424d85f2af13a328ccb55b9d2f8beb717c3e9`
- r190: `0b5d9152c2807ae38fdeb0350ca35373099b8a027ff380ee4079f174ce301467`
- r191: `b4942d7c7d4642fb3f7cd130832ca2a1513a25eb891d26a942b2c122af0fd77f`

These are frozen component candidates, not qualification of their subsequent
composition with the production compiler. Actual source materialization,
typed pending-owner custody, optimizer/verification consumption, protected
proof and safe launch still need integrated acceptance. M1-M7 remain incomplete;
strict production-to-required-proof-to-safe-GPU coverage remains **0/47**.

## Source-Owner Integration

Candidate `ea42e151850a667e0faab92c69376fe02373c584` composes the scoped
frontend, original-source invocation dependencies, deferred emission, V23 bridge
and inventory with immutable #271 candidate
`9533203467e356595b2f383ef9e739f9d65f40f0`. A private closed profile now lets
the existing materialization/replay engine retain either its historical V18
owner or the distinct V23 owner. V23 reconstruction keeps the original source,
SSA, launch inputs, graph and attachments together on their original account;
it does not convert a V18 owner or grant optimization/launch authority.

| Run | Candidate | Result |
| --- | --- | --- |
| r192 | `4f5dd5d3f` | Library check failed with six deferred-type lifetime errors |
| r193 | `dee83a59e` | Test compilation failed with 11 fixture errors; no tests ran |
| r194 | `ea42e1518` | Test compilation succeeded; 174 passed, 25 failed, none ignored |

r192 exposed missing propagation of the sidecar-backed type lifetime through
five signatures. The fix changes no ownership or accounting predicate. r193
then exposed missing launch-type imports and unchecked fixture projection
constructor results. Both failures are preserved, not reported as test passes.

r194 ran 199 selected lowerer tests, not the whole repository. All eight r186
failures now pass. All seven scalar-forwarding controls also pass after removal
of the duplicate V31 DFS and adaptation to the shared V32 forwarding service.
The three new closed-profile compatibility controls pass, but the complete
historical source-owner suite still needs a separate run.

Twenty-three failures stop at the shared invocation fixture's
`InvalidTypeLayout` admission error, before dependency, emission, census or
pending-owner assertions. None earns positive or intended-negative coverage.
The fixture's nominal `usize` declaration omitted `rustc_layout_is_noundef`,
required by the existing nominal-type validator. Successor `f470d3b99` supplies
that fixture property without weakening admission; its result is not included
in r194. The remaining two failures match #271's known byte-initialization
type-closure fixture and call-return mutation hitting an earlier identity guard.

The eight integration commits after `4f5dd5d3f` passed exact DCO sign-off,
whitespace and hygiene-delta checks. All three runs retained unchanged
source/tool inventories. r194 source snapshot:
`d6c4657945157b645881e1b9672ba263c03c29b7dd04d5a36edb50a399298714`.
Log SHA-256:

- r192: `0010130cbd332170f903a163f75d94eedc0074e3758ca435a0a1d7d740765362`
- r193: `17a7cb9c41444bd1a69aa877f974c347e1d31b865f6fc699809b89d5a1e0d33c`
- r194: `883c71f5493e43bb441e396e9eb48c009da9397e5b8927426f7623c6dcca8192`

r195 repeated the focused invocation, census and pending-owner tests at
`f470d3b99924e6a339df6a5db587fc8b431524b8`, and added the complete historical
source-owner controls: **19 passed, 23 failed, zero ignored**. All 16 historical
controls and three profile-compatibility controls passed. The 23 invocation
fixtures now pass the former type-layout boundary but stop at the shared
`InvalidFunctionAbi` admission error, before their intended assertions. They
still earn no positive or intended-negative coverage; this is not a green
integration result. The source and tool inventories remained unchanged.

r195 source snapshot SHA-256:
`09e55f66a84187ecc07f8fe97f1551107c8f21dfc13c8938b89d530a4c0f7cde`.
Log SHA-256:
`679d8003026538e3ae6b8671c60fec4cbcec81f703cd0a7279e8853f92d6de40`.

Genuine V41 source-to-pending consumption, branch/terminal invocation cleanup,
simulator execution and AMD lowering are separate active integration scopes.
The V23 optimizer still needs typed structural, transition, analysis and
source-owner consumers through the existing pipeline. No protected proof,
simulator or kernel GPU execution occurred in these runs. M1-M7 and **0/47**
are unchanged; the compiler candidate is not public-main qualification.

## Scoped Transition Payloads

Isolated candidate `fd51516564fde40c116b739bcbd128d026e65dbc` extends the
existing transition payload comparator for the five scoped operations. Exact
operation variants remain distinct; scope-end payload lengths are compared,
while the existing operand visitor retains SSA identity, order and multiplicity
checks. None of these operations becomes pure. This is raw payload comparison,
not a checked V23 optimizer transition or production authority.

r196 ran the complete kernel-analysis library: **698 passed, zero failed,
zero ignored**. This includes five new controls for variant separation, each
SSA operand position, repeated/reordered operands, visitor early termination,
legacy separation and exact/one-short work limits on the original account.
The source and tool inventories remained unchanged.

Source snapshot SHA-256:
`2e7fc25b6090e8b9d7bb1bb61cc5651754ab03c5b80a5e1d905e92a0b21d23fc`.
Log SHA-256:
`61e373da4d7e9156b1b1bc30f7e5cfef18802fa2f450d5b16da23b26a03fff92`.

The typed structural, transition, analysis and source-owner optimizer consumers
still need integration. No protected proof, simulator or GPU ran in r196;
M1-M7 remain open and strict end-to-end coverage remains **0/47**.

## Scoped Source and AMD Integration

The integration candidate now retains invocation lifetimes across source CFG
and helper boundaries, composes normal scope closure with terminal cleanup,
and carries genuine source custody into pending scoped materialization. These
changes remain candidates until their integrated acceptance tests pass.

| Run | Candidate | Scope | Result |
| --- | --- | --- | --- |
| r197 | `db0759255a0e84b52ca52e3f4ba8b5fbf3401154` | Selected source/lifetime tests | Compilation failed with E0716; no tests ran |
| r198 | `ca5de3161e7dc9319b9ce8f405909e490ee6a089` | AMD library | Compilation failed on a missing test import; no tests ran |
| r199 | `30db4d6d164ae2ead6bb8511bc8466a45500fb7b` | `rustc-codegen-fe2o3 --all-targets` check | Passed; no tests executed |
| r200 | `237689b629e9c3664112de56ec714eb81415e571` | Complete AMD library | 253 passed, zero failed, three ignored |
| r201 | `237689b629e9c3664112de56ec714eb81415e571` | Complete AMD documentation tests | 28 passed, zero failed or ignored |
| r202 | `3291b9b2fdc0a228752ff3ea6e3986490efd9bbd` | Scoped AMD tests with explicit LLVM 22 verification | 11 passed, zero failed or ignored |

r197's temporary occurrence-view lifetime was corrected without relaxing
admission. r198's missing `MemoryAccess` fixture import was corrected before
r200. These earlier failures remain failures, not intended-negative coverage.
r199 includes immutable #271 candidate
`c23b709a3b6c55a13d7e9fefc188fbe4e252bea2`, but predates the later AMD and
terminal-cleanup composition. It is a compiler build check, not production
source or launch acceptance.

The AMD component passes the exact scoped owner to the existing shared emitter;
it does not convert that owner into a historical profile. Context and invocation
remain logical, zero-ABI capabilities. Index reads retain their selected SSA
identity, checked pointer formation uses unsigned bounds, and unsupported
operation families fail before emission. r200 includes ten new scoped controls;
its three ignored tests are pre-existing explicit inert-fixture exports.

r202 includes those ten controls and a new explicit LLVM verification test,
so these counts are not disjoint. LLVM 22 accepted all four combinations of
gfx942/gfx950 and workgroup size 64/128. Four malformed-LLVM controls returned
the expected rejection status and diagnostic. The test used
`/opt/rocm-7.2.0/llvm/bin/opt`, SHA-256
`13cb4c99d1810b4db40bca5db0759ca94c8efd3c437bb8f7fe1a94f5bda66203`.
This is LLVM structural validation, not machine-code generation or refinement.

All six runs retained unchanged before/after source and tool inventories.
Log SHA-256:

- r197: `eaff9a73b1e1cf46f215b17f0cfbdff1dc5142d07d73d4d9dbe2e263d42ff200`
- r198: `3c5affa0d2cf4213e5853ad0051db7539b88bd9d1325ad14e864e927560b1ad4`
- r199: `58e552edd8b9415e4b3ac224ab9d7cb8ba58f64b8d8151f843eee4b1be0841e7`
- r200: `0de1b6b7749b9f9887d43712a9848b1a38496afcb25eaff6ac05ee9647b3ec7d`
- r201: `aab0fb69e6fdc0f22f744e11231a781f612fb51a490af3acd1618ce3c5f52ee2`
- r202: `934435e2ed6a090b44e912749d7645ea8a1d86201fcf3a1211d64119bb8d027e`

Simulator and typed optimizer integration remain separate unfinished work.
No protected proof or GPU execution occurred in these runs. M0 remains complete,
M1-M7 remain incomplete, and strict production-to-required-proof-to-safe-GPU
coverage remains **0/47**. Publishing this evidence does not activate the
integration candidate on public main.

r203 subsequently tested the combined source/lifetime/terminal-cleanup candidate
`5b87cc8e4f684b10ffc8d3c15feb0ea7fd2cdc03`: **48 passed, 35 failed,
zero ignored** (83 selected tests, not the complete lowerer suite). Compilation
succeeded. Most failures stop at the shared source-dependency consistency check,
`scoped source dependency differs from its original formation`, before the
intended emission, pending-owner or source-replay assertions. One negative
fixture instead stops at `InvalidTypeOperation` for an aggregate during fixture
admission. These failures earn no intended-negative coverage.

The earlier ABI and temporary-view compilation failures no longer stop this
run, but the integration is still failing. The source/tool inventories remained
unchanged. Source snapshot SHA-256:
`47922ae57b118994c8660ff2d9b3c44fa1bb632477456de943a206465b1daf40`.
Log SHA-256:
`e2e27cd3a57ffc551312f3e00d74416420f4ec667418d026868733b368249d5e`.
No simulator, protected proof or GPU execution occurred. Milestone and strict
end-to-end coverage counts are unchanged.

## Scoped Consumer Validation

These runs validate candidate components, not a production publication or
safe-launch transaction. Before/after source and tool inventories were stable
for every run below. All runs used the pinned nightly, locked offline inputs,
one build job and serial tests; GPU access was disabled.

| Run | Candidate | Scope | Result |
| --- | --- | --- | --- |
| r204 | `e69e3a541338162fcb8c83652cecc668dce21760` | Complete Pliron library | 2393 passed, two failed, zero ignored |
| r205 | `aee04ee3f04e4cba110106a8a3226a92c8be0e74` | Simulator all-targets check | Test compilation failed; no tests ran |
| r206 | `eb1a969dd82abd03ca4f4bdf2acf1eb643032b30` | Simulator all-targets check | Passed; no tests executed |
| r207 | `187d6effa1cd09ef85bcd2694e49107f6b6677b6` | Selected lowerer and SSA borrow tests | Lowerer: 65 passed, 21 failed; Pliron: 33 passed, zero failed; zero ignored |
| r208 | `eb1a969dd82abd03ca4f4bdf2acf1eb643032b30` | Complete simulator library | 145 passed, zero failed or ignored |
| r209 | `dbcc523fcc6f9de5701e06aca4ec29216de566ad` | Selected structural tests | 55 passed, zero failed or ignored; 2342 filtered out |

r204 exposed two incorrect test expectations: a short-storage rejection has a
nested canonical resource error, and an invalid geometry mutation is rejected
by the mutation verifier before structural extraction. The corrected tests
assert those actual boundaries, retain successful checked extraction, and check
exact resource limits and restored account floors. r209 includes all twelve
new structural controls but is not a rerun of the complete library.

r205 used the nonexistent test constant `F32`; changing it to `F32Bits` allowed
r206 and r208 to run. r208 includes four scoped simulator unit tests, but does
not execute the separate canonical V23 integration tests. No ordinary Rust
source-to-simulator result is established by this library pass.

r207 includes exact receiver classification for the four scoped invocation
intrinsics. It preserves the distinction between a borrowed receiver and a
by-value consumed index, and passes all three new classifier controls. The
earlier source-dependency formation failure no longer stops the selected tests.
Eighteen lowerer failures now stop at `embedded pointer kernel arguments have
no owned region binding`; three frontier assertions disagree about the exact
statement gap where a scope ends. The affected emission and replay negatives
have not reached their intended boundaries and earn no negative coverage.
The revised selection contains 86 lowerer tests, not r203's 83; counts are not
an unchanged-suite comparison. No live-rustc compiler-driver test ran in r207.

Log SHA-256:

- r204: `9746bbcab0d1aa0e09cb0bc9dcb72c7dd29206c030365ef948bf9f1f872a6dc1`
- r205: `14128061fc60d7cc5559e9d6cfecb4addf6bd2edba26e841255e5af1c9faf5be`
- r206: `571b653c1ed49e3bff5aab56a9150e0498b8113dd364a930915c2051cc64a7a2`
- r207: `1ab8fd552eb2a018d85ec6674ae283375d046b08f13fd7966f04b6fb50c355b8`
- r208: `46b9378e37d9f8ff1ca0a8c17dbbcb02b29bfa20fa33146111d8b0162115c734`
- r209: `9fcc9680b59c61ae347d0f23fade6a0bb69f2d42df20eec38abb35095a0288a6`

The compiler candidate remains separate from public main. Typed optimizer
integration, actual-source presence/control flow, protected proof and GPU
validation remain unfinished. M0 is complete, M1-M7 are incomplete, and strict
production-to-required-proof-to-safe-GPU coverage remains **0/47**.

## Shared Optimizer and Simulator Regression

These later component runs retain the same pinned, offline, serial, GPU-disabled
runner contract. Source and tool inventories were unchanged during every run.
Counts overlap between runs and must not be summed as independent coverage.

| Run | Candidate | Scope | Result |
| --- | --- | --- | --- |
| r210 | `a31e0f524feabf25ea5892dc7a6d48c0ace5b316` | Complete analysis and Pliron libraries | Analysis: 717 passed; Pliron: 2404 passed; zero failed or ignored |
| r211 | `4fb782dea318eca004638631c0b91e8b1696a03d` | Complete IR and simulator packages | 2494 passed, two failed, one pre-existing ignored |
| r212 | `a31e0f524feabf25ea5892dc7a6d48c0ace5b316` | Analysis and Pliron documentation | 181 passed, zero failed or ignored |
| r213 | `d6849f8a3d5c06d24de5e36ee1d47aef28b15011` | Complete simulator package | 494 passed, zero failed or ignored |
| r214 | `b49bfe39da7fde320f6a3763c04637dd7ce41e17` | Selected integrated source and Rust-driver tests | Simulator CLI test compilation failed; no tests ran |
| r215 | `884e7565db439cd7d6f465c5df7d0e790d202240` | Complete IR and simulator packages | 2501 passed, zero failed, one pre-existing ignored |
| r216 | `59d6a8a48bd308a5f664fff06f747f2ecad7ea03` | Complete Pliron library | Ambiguous module import prevented compilation; no tests ran |
| r217 | `a68557fe1db0025cdb302c3ec7d6dd3bff304ec3` | Complete Pliron library | 2423 passed, zero failed or ignored |
| r218 | `77b271a5493d9b14417f8a568f60166cf5914d6a` | Selected integrated source and Rust-driver tests | Rust-driver test compilation failed; no tests ran |
| r219 | `2ca8f6e13a8a6116af2343cfbefcc67d1f64c0bf` | Complete simulator CLI library | 92 passed, zero failed or ignored |
| r220 | `a68557fe1db0025cdb302c3ec7d6dd3bff304ec3` | Complete Pliron documentation | 110 passed, zero failed or ignored |

r210 and r212 exercise scoped canonical analysis factories, structural admission,
and the shared occurrence transition checker alongside their legacy regressions.
r217 and r220 additionally exercise actual scoped-owner execution of the existing
fixed optimizer policies, including typed extraction and consuming adoption. The
same shared optimizer machinery is used; these results do not cover the full
production LICM, native, composed-CFG and source-final continuation.

r211's two failures were capability-matrix test expectations that still counted
nine profiles after V23 added a tenth. r213 checks the repaired matrix, including
the explicit refusal of V23 on the older simulator entry, without weakening the
other profile expectations. r215 also checks original presence values transported
through one authenticated zero-extension into an integer switch, with paired
producer identity, dominance, live capability state and false-path reachability
controls. Its ignored test is the existing
`control_flow_bounds::sparse_wire_maximum_block_count_is_bounded_and_admitted`.

r214 and r218 both stopped before any selected test executed. The former exposed
missing new diagnostic variants in simulator CLI matches, repaired and tested in
r219. The latter exposed the same two missing variants in an older Rust-driver
CPU-capture reporting helper. Neither failed build establishes genuine Rust
source materialization, simulator execution, or the intended source negatives.
r216's module import ambiguity was qualified explicitly before r217 and r220.

Log SHA-256:

- r210: `0bfb9b3bab088a9b5281355fc4c14bffd25fa7c8bb5feb61d0e4b9a5267e7589`
- r211: `c9443015dd2ed10239e50fbac0ea7315fdad7a92f7ac32e251b555831ea548f5`
- r212: `ce355bf8fbef75165245194dabe9f17cd6fa60f5c13b8e41217c31742147218d`
- r213: `19fc434cb2861693c6a45694c6702fba958443f3ff0bdf1b6accb8e18caeaf00`
- r214: `ed2b2fcf3799bc7b20a33ed4746078e5a548f4d1cb62fe17c27304d3e8245604`
- r215: `84e987411e6fcfc585f45154fddb0aa7c6554bb2b4657a9603b619ae1e359dea`
- r216: `3638197f7b22492963e158aa9dc1b6860762625714f073f4b919738a26af1331`
- r217: `87c43d7e2d111d938300c49d41f69f86476c147d2048236e9616159a61979cf3`
- r218: `9fc05a560ef71c34afdd3bf15d097b4187ec1d27bda4080ba5f3d4e19b9d8cfe`
- r219: `8cd9c3729613524e8ee98ca239f555ecb02efd907169c6ba4dc80e5112209f21`
- r220: `4bfa4f146746ce5fcadb52020507664e2f43da996961c90f19bb7bec74fd29af`

No protected proof or GPU execution occurred. These candidate component results
do not activate the production path or advance the milestone acceptance count:
M0 is complete, M1-M7 remain incomplete, and strict end-to-end coverage is **0/47**.

## Original Source and Adopter Custody

The following candidate runs retained stable source/tool inventories and the
pinned offline, serial, GPU-disabled runner contract. Counts overlap earlier
runs; they are not additional independent kernel coverage.

| Run | Candidate | Scope | Result |
| --- | --- | --- | --- |
| r221 | `42688e2bf2108fd4704b984c8896db4df59fd37d` | Selected source, lowerer, Pliron and Rust-driver tests | Compilation succeeded; CLI 2 passed; lowerer 69 passed and 19 failed; Pliron 33 passed; driver run timed out |
| r222 | `e4bda9fcb2e2a6043bf0c01f217f6a3ff9834f45` | Complete Pliron library | 2436 passed, two failed, zero ignored |
| r224 | `d26bb4e9602f300628e5b3a5232f6fee560a3db0` | Complete Pliron library | 2436 passed, two failed, zero ignored |
| r226 | `b6e1db3433f171ab736000276ee1d27b8c5e1af6` | Complete Pliron library | 2438 passed, zero failed or ignored |

r221 repaired the Rust-driver diagnostic compilation gap. Its lowerer failures
identified an owned disjoint-slice fixture still using a by-value ABI descriptor;
the production admission correctly rejected that mismatch. The driver completed
one implicit-capability control, then stopped during the first actual-source
frontend parent. No genuine scoped Rust-source positive or intended source
negative is credited by this run. The later fixture correction preserves the
production contract and requires its own rerun.

r222 and r224 exposed two setup errors in new late-history refusal tests. The
first used an over-limit module name. After fixing that, the second exhausted
storage at the transfer wrapper instead of the intended history allocation.
The corrected tests prepay the concrete wrapper headers and retain the exact
late-history failure equation. r226 reaches and passes those controls for both
legacy and scoped owners, including selected-error preservation through hostile
destruction. No production limit or rejection boundary was weakened.

Separate provisioning preparation built a script-side static readback helper.
r223 (`e74951a17260e173728adc26063efa1974fa8493`) failed direct linking because
host proc-macro dependencies were absent from the rustc search path. After that
and an explicit numeric conversion were corrected, r225
(`b7d3ef205ba23e2b2dc61089f8410142e74bd16c`) built successfully and passed the
standalone static ELF checks. Helper SHA-256:
`afc9123fa5e425fa7d04d23ee07d9a0f0a143ee083786da016b04f92747b4d70`.
This is not container qualification, native provisioning, service activation,
protected proof execution or administrator approval of the helper.

The r221 and r225 reported wall-clock intervals exceed the configured deadline:
r221 records
21:18:08-23:04:42 UTC and exits 124; r225 records 21:33:49-23:05:31 UTC and exits
zero. The configured timeout was 1200 seconds with a 30-second kill grace; these
observations do not establish a 1200-second wall-clock completion guarantee.
The cause of that discrepancy has not been verified.

Log SHA-256:

- r221: `afc33f933d6169924fa567b3d39a90f7ef26c405b10a9bf29c9bd9969fc8dcdd`
- r222: `ae3613566a54c3a76be642a76973b6b8bb2e3a7f7875b5e4f24891afa81342ac`
- r223: `11eb003bfda8f7fcf5ceb646b831cb3c0d467b1dcf9d3643ac6e2d9a3238f9c1`
- r224: `0c6cd34efe4f4e1ffbc30a2ca5b61d1a12bd1484ffb73338c675cacefe170e2f`
- r225: `0367215aff77b3d30385bc958c572af646cd9f5b5513810a0cbfe70061452968`
- r226: `ff11b296f3e9e8630b56d2ac8a6f8be877d879263c2efb2e93b5c89e488bb16f`

M0 remains complete; M1-M7 and strict end-to-end **0/47** remain unchanged.

## Helper Transport and Native Analysis

The following local runs used the pinned offline toolchain, one build worker,
serial tests and disabled GPU runtimes. These are candidate component results,
not required-proof execution or production launch evidence.

| Run | Candidate | Scope | Result |
| --- | --- | --- | --- |
| r227 | `24a1d84d494efc52f61823ac62b27e4c47621d33` | Selected lowerer source/optimizer tests | 35 passed, 27 failed, zero ignored |
| r228 | `e82baa5fc1d41fe2a0e1f2a17197c92d05e9536d` | Analysis and optimizer libraries | Test compilation failed; no tests executed |
| r229 | `31d21aad2d4f39609a231adcc835fcd253b7c7e5` | Complete analysis and optimizer libraries | Analysis 720 passed; optimizer 445 passed, three existing child helpers ignored; zero failures |
| r230 | Unmerged working tree, not a sealed candidate | IR and analysis attempt | Invalid integration attempt; compilation stopped at conflict markers |
| r231 | `522c36642619aab8539c3d91b875fa94349022fa` | IR and analysis libraries | Test compilation failed; no tests executed |
| r232 | `d76e21409f1a395da533f5e0da160656fafc9d3b` | Complete IR and analysis libraries | IR 1429 and analysis 726 passed; zero failures or ignored tests |

r227 passed the corrected owned-slice fixture gate, but all 27 failures stopped
at the internal helper's `Option<&mut u32>` ABI check. The optional reference is
tracked by the scoped source plan; it still falls through to the general enum
argument refusal. This does not validate the intended downstream optimizer
consumer controls. The ordinary Rust fill/vector-add parents were not selected
by this lowerer-only run.

r228 exposed an empty-layout fixture using a nonexistent `Default` constructor.
The correction supplies explicit zero limits for its empty table. r229 exercises
the shared legacy/scoped LICM engine and its analysis consumers. r231 exposed two
overlapping mutable test-view queries; copying each observation before the next
query corrected the test. r232 exercises the initial scoped ranked/private and
conditional memory views. Neither run proves their integration with all nine
native stages, source-bound refinement requests, or actual Rust kernel launches.

r230 was started before a dependency cherry-pick conflict was noticed. Its
recorded HEAD is not the source that compiled: the file snapshot contains
conflict markers. It receives neither positive nor intended-negative credit.
The owned failed cherry-pick was aborted, the missing dependency was applied,
and subsequent candidates were sealed before testing. The local runner now
rejects an unmerged index before creating a test snapshot.

Runner provenance also has a historical limitation: r227 began before that
runner edit but read the runner file's hash at completion. Its reported
`fd0434853cc28c56d795a956cdbef34d8f72167b85b2b5664e4ec1ef471fe0a2`
therefore does not authenticate the script loaded at startup. Its stable source
and tool snapshots and test log remain component observations; the raw report
has not been rewritten. Subsequent runs capture the runner hash at startup.

Log SHA-256:

- r227: `3e1780d07916d4eeb328e0f5d8151921a0e2996988fc2430a765fdcec2ac53e8`
- r228: `23aa173fc9cf122e8acea19c32b694a0fb9df5f1e3192dfe551dcadc93459768`
- r229: `948c3b29e90b2fa5a31c4b576f7d1a672b37f16d8820464784e4bab4d5f136ea`
- r230: `aa6746ae8b9af4d410f929f0f7e302abe54307b32b3cf1ff15d51cd658926bb7`
- r231: `cc1f48d5448603c54e1d549cb54548c7a0f717745ee682263490975cf84dc4b7`
- r232: `646be830fe148fbac02c5db5a85a35bc185351c67d4778d575292e40245bf51c`

M0 remains complete; M1-M7 are incomplete and strict end-to-end coverage remains
**0/47**. These runs did not execute a protected proof runtime or a GPU kernel.
