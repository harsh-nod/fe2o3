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
tests, Pliron bridge and actual source-scope emission remain unfinished. No
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
