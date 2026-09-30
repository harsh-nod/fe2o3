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
