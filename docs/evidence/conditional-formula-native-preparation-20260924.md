# Conditional Formulas And Native Preparation

This is a prerequisite checkpoint for #272. It does not complete M1, any other
milestone, or the 47-kernel production-to-safe-GPU-launch matrix. No GPU run or
protected conditional aggregate execution is credited to this checkpoint.

## Compiler Boundary

The production reference path can retain a conditional root separately from an
ordinary clean lowering owner. Projection returns existing proof requests;
their consuming continuation runs after scalar, assertion and source-progress
scratch scopes close. The guarded source owner retains the additional arena
charge on its original resource account. Conditional roots cannot become V5
evidence or enter the ordinary finalizer: that boundary rejects with
`FE2O3-COND-FINALIZER-001`.

The CPU join replays the retained, authenticated CPU MIR through the existing
acyclic output resolver. It checks source/monomorphization identities, raw CPU
argument relations, output occurrences, point coordinates, read origins and
exact typed value expressions. Replay uses the original cumulative account;
its temporary storage is released after replayed expressions are dropped.
This restricted join is not a proof of arbitrary Rust or loop equivalence.

The verifier derives formula source from the current conditional graph, its
checked effect staging and its closed runtime-premise roster. A separate
obligation domain binds those identities. The existing protected execution and
signed-import mechanism is reused, but the receipt is callback-scoped and is
not accepted as an ordinary clean-graph aggregate. Copied reports are inert.
The backend's private CPU-source join must coexist with formula execution;
CPU hash fields alone are not that join.

The generated theorems establish expression equalities and conditional address,
coverage, separation and permission facts. They do not discharge host premises
or establish an executable CPU/GPU memory-transition or machine-refinement
theorem. Actual-source Vecadd still requires CPU input-bounds discharge and
premise-aware read/trap verification. Descriptor transport, host discharge,
generic conditional finalization and safe launch remain incomplete.

## Native Issuer Boundary

`ProtectedCompilerExecutionIssuerAdmissionV2::serve_native_preparation` consumes
native custody into a bounded first-sequence service. It shares the existing
singleton lock, packet transport and durable recovery mechanics. Prepare/Issue
observe the live compiler and its locked V4 publication independently, recheck
custody, and commit signed state before sending responses. Remote descriptor
inspection refusal is terminal; there is no V1 conversion or supplied-subject
fallback.

Worker publication, anchor joins, currentness, non-genesis positions and
readiness remain gated. The production serving entrypoint remains V1.
Distinct-UID deployment inspection permissions still need an isolated end-to-end
test and architectural integration. The new unit tests exercise journal/key
components, not a public protected Prepare/Issue session.

## Validation

Local guarded runs use pinned nightly `2026-04-03`, locked offline dependencies,
one Cargo job, no GPU, a 12 GiB process virtual-memory limit and a 1200-second
outer timeout. Source and tool snapshots are checked before and after each run.

The pre-review-correction full library run passed 269 broker tests (6 ignored),
106 capability tests, and 224 verifier tests (15 ignored). Its source snapshot
was `8de0ccc0cfaf8146392cf63d59676e0fca33045c726f4c0fb0cea3744d7e4c82`;
log SHA-256 was
`0d25bd7551aadd0126a0daa30e8ca9749a880583a076853aa7833b1696e305cd`.
The later ownership/recovery corrections have the focused validation recorded
below; this full-suite result is not transferred to changed source.

An opt-in development Verus regression executes three fresh solver processes:
the generated positive, a wrong-value mutation, and a missing-coverage theorem.
The final run accepted the positive and rejected both mutations. It used a
user-writable development installation of Verus `0.2026.08.02.b677dd5`, not
protected runtime custody. That is solver/source evidence only, never a
production receipt or hardware result.

### Final Candidate Checks

These runs all completed with unchanged source and tool snapshots. Their
source snapshot was
`2b8ba70ce4152a0f69db421df5dd10f1a629d6c744a65649be81e2a00f6549dc`
before this documentation-only results update. This is a content snapshot of
tracked and nonignored untracked files, not a Git tree identity. The guarded
runner SHA-256 was
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.

| Run | Result | Log SHA-256 |
| --- | --- | --- |
| `conditional-formula-final-component-tests-r3` | 819 passed, 13 ignored: 7 native issuer, 83 lower-MIR, 78 PLIRON, 651 backend | `67df3fbcedda31a17c9ce119d69e75736e4739519d511e3ab70071bcf5222601` |
| `conditional-formula-final-all-targets-r1` | All targets checked in the six affected packages; warnings remain | `51ab361b7b3fc9070490e0c1f4eda71f5c5278f9000224cfb13ec44f186a9be6` |
| `conditional-formula-final-doctests-r1` | Both new compile-fail doctests passed | `6bebbeb0d3b3663c155fce7574f0bddca13631adf8b3e21f6c7a335dbe3ca8fd` |
| `conditional-formula-final-development-verus-r1` | One opt-in test passed, checking three separate solver processes | `f12087838ba92e67f24f31b3f04a75395133628f9def3a0bf80b2d83a3371c46` |
| `conditional-formula-final-source-regressions-r1` | Five CPU replay tests and two actual-source tests passed | `446cb781507c2f3ed23c97cc054f39d9f59ef8d3ac90e87984a2a5dbb8ea335d` |

The source tests establish that default manifest fill still reaches checked
native output on gfx942, and reference-annotated fill reaches the pre-proof
prepared request on gfx942 and gfx950. They do not execute the new protected
conditional formula continuation, authenticate a live native issuer session,
or launch either target on hardware. CPU replay tests cover typed fill/add,
source and expression mutations, exact/short quotas, account substitution,
errors and unwind; they are not a live rustc CPU-source authentication test.

The component and source regressions can be selected with:

```sh
cargo +nightly-2026-04-03 test --locked --offline \
  -p rustc-codegen-fe2o3 -p fe2o3-lower-mir-kernel \
  -p fe2o3-pliron -p fe2o3-broker-authority-service \
  --features fe2o3-pliron/internal-proof-staging --lib -- \
  production_ranked_projection_v1:: conditional_ consuming_continuation_ \
  borrowed_continuation_ native_consumer_ native_transport_

cargo +nightly-2026-04-03 test --locked --offline \
  -p rustc-codegen-fe2o3 -p fe2o3-lower-mir-kernel \
  -p fe2o3-pliron -p fe2o3-broker-authority-service \
  --features fe2o3-pliron/internal-proof-staging --lib -- --include-ignored \
  cpu_replay_ default_manifest_fill_reaches_checked_native_output \
  actual_reference_fill_joins_canonical_source_and_prepared_ranked_output
```

The source tests require the pinned toolchain's `rust-src` and offline
AMD-target dependencies. The development solver test requires explicit
absolute `FE2O3_DEVELOPMENT_VERUS_BIN` and `FE2O3_DEVELOPMENT_RUSTUP_BIN` paths;
it never provisions or substitutes a protected runtime. The recorded runs also
disabled visible GPUs and used the resource limits above.

The previous [actual-source protected fill test](manifest-fill-source-proof-20260924.md)
belongs to its recorded frozen compiler. Its updated consuming-formula/finalizer
expectation has not yet run on this candidate. Missing runtime or later refusal
does not count as successful protected proof execution.

The tutorial manifest still reports `qualified=false`, pending semantic/policy
verification, and the existing `gemm-proof-plan` source-binding gap. No manifest
qualification or issue milestone checkbox changes in this checkpoint.
