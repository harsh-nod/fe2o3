# Conditional Source Output Boundary

Date: 2026-09-27. Base: `73cd83b5ba78c942c3d316b38685a0c7bcde88a4`.
Follow-up to [Cargo durable custody](conditional-cargo-durable-custody-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**This checkpoint adds regression coverage, not production activation. An
output-bearing semantic MIR fixture now passes canonical output coverage and
source-to-ranked effect/value correspondence. It does not obtain an aggregate
request, a reference proof, protected source-origin evidence, an artifact or
safe launch authority. No full M0-M7 milestone or 47/47 GPU completion is claimed.**

## Source And Checks

The fixture describes `if global_x < output.len() { output[global_x] = stored; }`
with a mutable `u32` slice, a compiler intrinsic index witness and a dynamic D1
launch. It uses public inert MIR constructors, current-production admission,
`ProductionSemanticMirOwnerV1`, `ProductionSemanticSsaOwnerV1` and
`ProductionPreRankedKirOwnerV1::try_materialize_with_budget`. It is not an actual
rustc capture. No checked source owner, canonical body, correspondence table or
private proof owner is fabricated or mutated after construction.

The source fixture, ranked fixture and assertions are separate test modules.
The source fixture reuses existing layout/place builders. Production code,
dependencies and authority APIs are unchanged.

Seven tests cover:

- Canonical conditional output coverage and exact source-argument binding.
- A real pending graph without a reference contract: its single global write
  and stored scalar match the actual source through the existing translation
  checker. Aggregate continuation subsequently refuses the missing contract.
- The output occurrence, extent and guarded paths of an inert ranked recipe
  containing a reference request. This descriptive join does not materialize
  that recipe, authenticate a CPU reference or supply proof authority.
- Refusal to materialize an unproved reference request.
- A separately admitted source writing `8` against a ranked expression writing
  `7`, rejected as `ValueExpressionMismatch` before the reference-contract gate.
- Wrong root/rank, absent or duplicate access rows and a wrong statement index.
- Exact measured work/storage limits and one-unit-short denials, preserving the
  original account and retained source storage without invoking the callback.

These are logical ledger limits, not whole-process allocation or RSS bounds.

The changed-scalar test establishes a source-to-ranked translation rejection,
not CPU/GPU semantic equivalence or a user-facing rustc diagnostic.

## Verified Receipt Prerequisite

The attempted positive aggregate fixture exposed an earlier prerequisite than
the no-output wrapper fixture could exercise:

1. `RequestEffectRefinement` is inert. `construct_registered` refuses it with
   `unbound functional-refinement request cannot be materialized`.
2. The production reference join must execute and import the per-effect proof,
   bind the request to `RequireEffectRefinement`, and stage it under its accepted
   policy. `CompilerOwnedReferenceEffectRequestV2::prove_and_bind` already opens
   the retained protected Verus runtime for this operation.
3. Only then can the pending ranked graph reach the source-bound aggregate
   request, with the same source, CPU subjects, access rows and resource ledger.
4. The aggregate formula itself still needs genuine execution/import. Worker,
   finalizer and durable-recovery checks follow; none is replaced by these tests.

The independent replay path in `compiler_native_conditional_source_proof_v2/root.rs`
similarly checks the transported effect signer against externally accepted
policy before reconstructing the pending graph. A local test-key signature
would not close the protected execution gap, so no such stand-in was added.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
single-job Cargo command at a time, with compiled source frozen during each run.

| Command | Result |
| --- | --- |
| `cargo test -p fe2o3-lower-mir-kernel --lib conditional_source_output_v1_tests` | 7 passed |
| `cargo test -p fe2o3-lower-mir-kernel --lib` | 1,832 passed; zero failures or ignored tests |

The focused tests are included in the full suite, which finished in 934.74
seconds. Rustfmt and whitespace checks passed. No unsafe code, dependency change
or inventory allowance was added; the preexisting unsafe-inventory failure was
not rerun or rebaselined here.

Intermediate failed runs identified an incorrect fixture ABI annotation, a
finite launch instead of a dynamic launch, and the unproved-request
materialization gate. Their logs are retained; they are not successful evidence.

Logs are under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
88d4169b7093e7eb133e4fe305b0e3f9bd2ab0b0a52420475516aa5d339da3e2  conditional-source-output-20260927-r4.log
41bb09a9c1291c12ba78fa76e7b9db92268af35ccbe645d85426b00137aac219  conditional-source-output-all-lib-20260927.log
```

All commands terminated, and the empty private scratch directory
`/tmp/fe272source.lYBXEYoX` was removed. Source worktrees, reports and the existing
build cache were preserved.

## Remaining Work

The next positive fixture needs actual CPU-reference input and genuine
per-effect execution/import before it can test aggregate continuation. The
complete conditional source/Worker/journal round trip remains untested.
Selected parent/profile/broker/driver activation, independent root-policy
admission, sealed verifier/generated-host integration, machine/numerical
refinement, target-matched 47/47 runs, selector retirement and release gates
remain open.

Native agent spawning hit the thread limit; this checkpoint was implemented
locally. All three GPU aliases failed DNS resolution, and no remote job or file
was created. A GitHub `ls-remote` briefly succeeded and reported origin/main at
`0c01920842bc72e4a9f66fac23ca9c5c2f3bc465`; subsequent fetches from both remotes
failed DNS. That remote history must be fetched and integrated before a normal
push can publish this branch. Push and issue-update outcomes are reported
separately from local test results.
