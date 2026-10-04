# BF16 helper source-transport qualification — 2026-09-25

This checkpoint adds a bounded, genuine-source helper inspection path for
#280 M4 and #282 U4. It does not yet emit, simulate or execute that helper.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented boundary

The dedicated importer retains actual rustc Instances, HIR/MIR source spans,
source signatures and FnABI records for one root and one body-local helper.
The fixture is ordinary Rust with one `#[inline(never)]` helper taking an
immutable `&DeviceMatrix`, A/B BF16 fragments and an F32 accumulator. It
returns four F32 values in Identity or Swap01 order.

Reference nomination requires the actual root matrix-context producer and
borrow, the actual Defined helper call, exact source types and ownership,
and the helper's actual matrix receiver and conversion. The existing all-use
escape analysis remains mandatory. Ordinary references do not become generally
transparent; the existing Execution path is unchanged.

The lowerer checks the actual captured SSA occurrences, argument/formal
transport, producers and return-component ancestry in one borrowed relation.
The frontend rejoins that relation to the live source and ABI. It then moves
the same SSA owner into the unchanged ordinary materializer. There is no
second executable graph, flattened helper or alternate compilation route.

The original materialization resource ledger pays source storage, one
occurrence capture, bounded scans and scratch. Callback-owned output remains
charged on success, refusal and the supported callback-panic path; replacing
the ledger, undercutting its floor or ignoring a sticky denial refuses.
The shared finite-header scan refuses above 32 blocks or 4,096 total
locals/statements before traversing oversized headers; checked arithmetic and
per-header work charging also precede access.

## Qualification

Base: `bfa616c996fa529da67f2f6c32e7829213914e3e`, with the reviewed checkpoint
changes. The joined source census was 8,208 files / 117,880,693 bytes, SHA-256
`016ce7c0e72a8666b77f1dcb8f4ec9debeedcc40eebad0a1b274119eaa470bcb`.

- The initial 13 lowerer controls and 175 lowerer documentation tests passed.
  The corrected finite-header scan passed all 20 relation controls, including
  seven boundary/overflow/early-refusal/original-ledger tests.
- The joined focused gate passed 9 nomination, 5 existing borrow-analysis,
  13 relation and 15 frontend tests: 42 executions, zero failures.
- Five fresh genuine rustc sessions passed: Identity, Swap01, wrong launch,
  callback error and callback panic. The parent built fresh GPU-target
  dependencies offline from the pinned lockfile and nightly.

The real positive cases each contain 19 root blocks and three helper blocks.
Their actual SSA coordinates differ; no fixed ordinal coordinates substitute
for source/SSA joins. Actual helper argument pass modes are Direct, Cast,
Cast and Indirect, with an Indirect result. These are observed source FnABI
facts, not a claim about final machine call-register transport.

Both positives first produced the genuine checked source relation and then
reached exactly:

```text
helper parameter is not an exact by-value scalar aggregate or shared slice
```

An earlier refusal does not qualify as successful transport. Wrong launch
refused before the callback. Error and panic controls retained the same
original ledger and restored the floor plus 23 bytes of callback-owned
storage. Successful snapshots retained their charged 1,216-byte observation.

The completed root source gate receipt is 30,270 bytes, SHA-256
`183ca4df25bd8f3cbbba00d59a4523d102b642a6a62130c1e64c527c5d40dfc0`.
Its 42,292-byte five-session observation is SHA-256
`2d7b0e97e417cc0d7232e7e0ea3953d9fd09eb90a895b5861a73183463cd5723`.
Individual copied observations deliberately retain `accepted=false`;
qualification requires the completed parent and independent outer audit.
These retained observations grant no compiler, publication or launch authority.

## Regression and malformed-owner controls

The merged CPU regression passed 7,437 test executions in 111 result groups
(overlapping configurations), zero failures and 295 ignored tests. It covers
the selected compiler/pliron/lowerer/simulator/debugger/runtime/verifier suites,
lowerer and runtime doctests, and the unsafe-source inventory. The completed
35,749-byte receipt is SHA-256
`97e3b78fef8ae16942c7aa665cf6cfd89922071c6131788100ddbc36519c2a27`.
The earlier failed inventory gate is retained: its one missing test-only unsafe
implementation was independently reviewed and narrowly reconciled without a
runtime change or gate exemption.

Ten additional integration controls construct real immutable MIR/SSA owners
from explicitly inert requests. Identity and Swap01 baselines reach the
borrowed callback before the exact Copy, duplicate-result, conditional, extra
unreachable-block and cycle refusals are tested. Missing/unreserved capture,
exact/one-short resources and callback error/panic retention also pass. These
are whole-owner checker tests, not rustc/HIR authenticity or emitted execution.
The completed 21,473-byte receipt is SHA-256
`a7552a6ac108841ffacdfa6b96dded62e2eb5b6be04fa72fa30afe88a272e545`.
Initial failed fixture runs remain retained; only fixture array layout, unused
type removal and per-consumer lane borrows were corrected. Production admission
and reference-escape rules were not weakened.

The existing root-only inspection (four source sessions), normal compilation
(two sessions), and numerical CPU (four sessions; 18 positives/16 request
refusals) ladders passed again. Their completed combined 34,356-byte receipt is
SHA-256 `78eff848b540851c45b0a29db292a85778d62e234596371ffe8a4244e96f6a42`.
This regression evidence does not transfer numerical qualification to the new
helper inspection path.

## Reproduction

Use the pinned nightly and repository's offline dependency setup:

```sh
cargo test --offline --locked -p fe2o3-pliron --lib matrix_reference::tests
cargo test --offline --locked -p fe2o3-lower-mir-kernel --lib call_instance::tests
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib bf16_tile_values
```

For the genuine-source parent, set `RUSTC` to the absolute pinned rustc,
make its libraries available, and set
`FE2O3_TEST_BF16_TILE_VALUES_OUTPUT_V1` to a fresh absolute output directory
outside the checkout. Then run:

```sh
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::gfx942_bf16_tile_values_qualification_v1_tests::actual_bf16_tile_values_ladder \
  -- --exact --ignored --nocapture
```

The test creates new retained outputs and refuses reuse. The bounded child
helper supervises direct children/process groups, not arbitrary process families.

## Still required

Actual nominal helper parameter/component emission, independent canonical
Call/Return replay, full-participation proof, ranked/formal/target continuation,
numerical execution of the helper graph, source promotion/edit/re-admission,
machine/ABI correspondence and applicable hardware qualification remain open.
The earlier root-only BF16 numerical checkpoint remains separately scoped.
No public CLI or general BF16 helper support is introduced here.
