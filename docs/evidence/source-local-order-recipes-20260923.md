# Saved local-order recipes — 2026-09-23

This report qualifies the bounded Linux, direct-u32, gfx942:xnack-/wave64
Create/Replay workflow for #282 U3. The named composition remains
`source-local-order-policy6-v1`; ordering is canonical KIR order, not a
promise about LLVM or machine scheduling.

## Implemented author workflow

The public `SourceLocalOrderRecipeRequestV1` and
`run_source_local_order_recipe_driver_v1` accept inert intent. Create returns
a new bounded recipe and actual-current-L LLVM. Replay reads intent into a
fresh ordinary-source compilation; it never restores a serialized compiler
owner. Source-owned current HIR/Instance, original N, the unchanged fixed
Policy6 prefix I and the independently checked I→L continuation are retained
in the same invocation. Current constraints, descriptors and applicable formal
analysis are derived again. The protected output-owner/finalizer is unchanged.

The supported expression is one unambiguous `(a ^ b) & (c | d)` using four
distinct immutable u32 formal parameters, required/maximum workgroup 64×1×1.
The whole Rust kernel, including its guarded output store, remains the
computation. SourceOrder and ReverseReady select [14,15,16] and [15,14,16]
respectively for this fixture. There is no caller-selected pass list.

ExactRevision requires the original source digest and both full N/I
digest-and-length predicates. RebindCurrent is explicit permission to derive
fresh current bindings; it does not rewrite the saved recipe or discard the
five item/instance axes, target, launch or eligibility checks. A changed kernel
item rejects the old recipe; explicit Create regenerates new intent.

Exact constraints refuse an incompatible requested order. Advisory constraints
report NotHonored without silently changing computation, weakening checks or
running an alternative pipeline. Recipe annotations are historical information,
not source authentication or reusable proof/analysis authority.

The normal `source_local_order_recipe_v1` Cargo example exposes Create/Replay,
bounded no-follow recipe reads, independently supplied source/recipe hashes
and fresh no-overwrite recipe/LLVM outputs. Callers supply the complete observed
rustc argument vector, working directory and source-profile environment.
Ordinary rustc argument side effects are not redefined. File outputs are
nontransactional: an earlier newly created file may remain if later publication
fails. No native object, protected proof, final artifact or launch is produced.

## Fresh actual-source qualification

`phase28-source-local-order-release-actual-r1/observation.json`:
514,275 bytes, SHA-256
`fc9c289b0501e423474605b5c8b5cdf96369672346c047fc4bbaef00e012612e`.

The separately selected ignored test exercises the same release adapter, with
a test-only observer for independently checking and simulating actual L:

- 25 main compiler callbacks: 15 successes and 10 exact refusals.
- Two persisted legal schedules, same-source deterministic replays, compatible
  renamed/commented-source rebinding, and changed-item explicit regeneration
  followed by fresh replay.
- Rejections for edited exact revisions, changed item, altered operator, local
  alias, ambiguous initializer, target, launch, stale supplied source and an
  unmet exact constraint. Advisory mismatch is an explicit successful report.
- Three separate fault probes refuse source mutation after retention, repeated
  callback entry, and a genuine rustc fatal error. There are 28 compiler
  callbacks / driver attempts and 29 callback-method invocations in total.
- Fifteen successes each pass 30 whole-kernel CPU runs: five independent input
  vectors, lengths 0/1/65 and two replays. The independent expression oracle
  checks output, initializedness and canaries: **450 simulations**, 470,700
  steps, 1,288,086 prepaid host-payload bytes. Finite tests are not universal
  equivalence proofs.

Seven recipes and twelve isolated source files are retained. Maximum observed
logical continuation storage/work are 67,797,289 bytes / 91,681,379 units.
These are ledger bounds, not measured process RSS or latency.

The foundation gate passed 1,761 backend tests (117 ignored), built the normal
example and separately passed the actual-source ladder. Its receipt is
`logs/phase28-resume-r3-compiler-release-recipe-foundation-r3/receipt.json`,
22,324 bytes, SHA-256
`6de85e54c7afb63d319052c8fe17e9dd5951243db6c01c3aec4f4cd667ea4e57`.

## Normal public-example qualification

Eight fresh ordinary example processes passed: five successes and three
designated refusals. Create/Replay, compatible edited-source replay and the
advisory mismatch matched the retained actual-L LLVM bytes; each successful
Create recipe matched independently retained expected bytes. Edited exact
revision, changed item and exact-order mismatch returned the exact failure,
with no recipe/LLVM output. Compiler warning streams were compared, not filtered.

The helper's 14 pure checks passed separately. The example and its dynamically
loaded backend were freshly rebuilt and independently pinned:
124,936 bytes / `8a4d9f7ab408f7f9cbf88fd8ab674f6428dcd4b27d132f61fb145111259537d6`,
and 246,972,880 bytes /
`6b80f4c42209af40f97d00c0bfba5114d47d46da9426a95c4fd355e5ff96d6d8`.
The qualifier also retained/rechecked the selected rustc/runtime libraries and
exact prepared dependency-directory membership; this is not a complete
system-loader or compiler-closure attestation.

`phase28-source-local-order-normal-example-r1/receipt.json` is 74,759 bytes,
SHA-256 `4ba3fd2f0cce25f5cd306658dda453149d8baa3900e0cf515b152a60b0d98d0c`.
Its outer gate receipt is 25,519 bytes,
SHA-256 `d2d403571a8294fa0ca5d610bcd116858186f61071dad72baf7011b21100113f`.
These eight processes used no test observer and add **zero** CPU simulations,
native executions or proofs to the separate actual-source campaign.

## Regression and acceptance

The integrated compiler gate passed optimizer/lowerer/backend library suites:
335 / 1,478 / 1,761 tests (3 / 0 / 117 ignored), plus complete debugger,
protocol and simulator-CLI tests and repeat-origin/source/LLVM/native script
controls. Clippy and the normal example build completed. Gate receipt
`logs/phase28-resume-r3-compiler-release-target-final-r1/receipt.json`:
24,084 bytes,
SHA-256 `a098120dccd8ca8d0862b5736f405af44c5976c375e29d6cc0969f58847d9153`.

A final local lint cleanup retained the bounded decoded recipe inline rather
than adding a boxing allocation, removed an unnecessary borrow, and named the
test-only recursive compiler argument explicitly. Backend tests (1,761/117),
Clippy and the example build passed again before the eight normal processes.
That receipt is 22,184 bytes,
SHA-256 `720ba77f88b37ede1ad63c694c8c4b3c49caea72c0686f2844bff5b646bdeb22`.
Existing unrelated warnings remain; warning-clean workspace status is not claimed.
The final code census is 6,749 files / 101,979,722 bytes,
SHA-256 `1ccbe38cc80ba39adb5ae32e81e613f741309d87721c365639345f083b412a7c`.
These code gates precede the documentation-only acceptance additions.

The codec rejects unknown/duplicate fields, incompatible versions/profiles and
records over 8,192 bytes. An actual regression exposed Serde's treatment of an
internally tagged unit variant; RebindCurrent now uses a closed empty-struct
variant, retaining the same wire spelling while refusing extraneous fields.
The 14 codec tests include explicit unknown-field controls for that variant.
Eight API tests and five qualification-helper tests are separate from actual
compiler runs. Two formerly test-only source/join helpers moved to shared
implementation leaves; their previous names remain in Git history.

This evidence satisfies the five original U3 exits for the declared profile:
two persisted schedules; deterministic/version/target/exact-advisory replay
with fresh records; real-edit success and stale/ambiguous/precondition refusal;
explicit checked rebind or regeneration; and no pass/check/receipt bypass.
The accepted original exits are now **M1/V1/V2/U1/U2/U3 (6/18)**.
All three umbrella issues remain open. The earlier 13-callback/150-simulation
continuation report remains separately dated evidence, not duplicated here.

The [companion tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/saved-local-order-recipes-v1.md)
documents the real Rust kernel, build, Create/Replay commands, explicit
binding modes, edits and refusal exercises. Whole-body assembly, authored
memory/synchronization, arbitrary scheduling, physical registers, tiled
examples, protected artifact completion and hardware remain separate work.

## Reproduce

Use the repository's pinned nightly/provider environment with absolute RUSTC
and a fresh absolute output directory:

```sh
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib
cargo build --offline --locked -p rustc-codegen-fe2o3 \
  --example source_local_order_recipe_v1
FE2O3_TEST_SOURCE_LOCAL_ORDER_RELEASE_RECIPE_OUTPUT=/absolute/new/recipe-run \
  cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::release_recipe::ladder::actual_source_local_order_release_recipe_ladder \
  -- --exact --ignored --nocapture --test-threads=1
```

The actual ladder is not part of the default ignored-test run. Keep the public
example and its dynamically linked backend library together and preserve the
matching sysroot/dependency closure. No historical native/proof result is
relabeled as a recipe result.
