# Row-softmax V1 formal contract

This standalone example specifies one **unmasked, nonempty row of exactly 64
conceptual `f32` values** and a separate 64-element output. Its proved claims
remain a source-level formal model and finite host reference, not a production
compiler/runtime or machine-code refinement result.

`production_release.rs` additionally stages the exact `gfx942:xnack-`, width-64
typed HSA lifecycle with immutable guarded input, guarded output, CPU-oracle
comparison, and fixed normal/equal/dominant workloads. Masked, exceptional,
wrong-shape, wrong-policy, provider-drift, worker-artifact, and target variants
have explicit prelaunch rejection stages. The compiler/finalizer join uses the
direct upstream LLVM/LLD worker and an exact OCML closure; it uses neither COMGR
nor a shell linker. These mechanics do not extend the formal proof above and do
not generalize to other widths, shapes, masks, targets, or numerical profiles.

This staged path is not production-reachable. A real MI300X protected attempt
crosses launcher/handoff and frozen Cargo closure, but a dynamic Rust workspace
wrapper cannot retain compiler authority: production cannot admit Cargo's
mutable loader path, and after loader variables are cleared the wrapper cannot
load `librustc_driver`. `cargo fe2o3 authority release run` therefore fails
closed at `stage=binding-wrapper` pending integrated static-wrapper support. No
compiler-origin, proof-validity, artifact-safety, runtime, or GPU-execution
authority is minted by this slice.

## Contract layers

The executable crate also exposes `row_softmax_oracle_v1` and
`compare_row_softmax_v1` as the shared bounded numerical contract for future
source and hardware profiles. The oracle supports explicit activity masks for
rows of 1 through 4,096 finite physical inputs, rejects all-masked rows and all
non-finite inputs before mutating output, converts finite signed-zero and
subnormal inputs exactly into its stable `f64` calculation, permits output
underflow when rounding to `f32`, and emits canonical positive zero for masked
outputs. The API names the pinned-host Rust `f64::exp` reference and the
authenticated device `__ocml_exp_f32` implementation separately. The reviewed gfx942 OCML comparison policy
separates per-output absolute/relative/ULP limits from an independent output-sum
limit. It is a finite comparison policy, not a proof of OCML error or IEEE
refinement.

1. **Real mathematical specification.**
   `verus/row_softmax_v1.rs` defines stable softmax over mathematical reals.
   A maximum bounds every input and equals at least one input. Each weight is
   the abstract real exponential of `input[i] - maximum`; each output satisfies
   `output[i] * sum(weights) == weights[i]`, and the specification explicitly
   requires `sum(output) == 1`. Verus proves that every admitted
   `stable_softmax_spec_v1` value has this normalized mathematical postcondition.
   Stock Verus supplies no real-field multiplication laws here, so V1 does not
   claim to derive that postcondition from the pointwise equation alone.
2. **Finite algorithm state.**
   The Verus `denominator_state_v1` predicate models the fixed 64-step
   sequential reduction, including its zero initialization, and proves one-step
   invariant preservation. `maximum_reduction_state_v1` similarly binds the
   initialized one-element maximum state and completed reduction state. The
   separate `finite_numerator_premises_v1` predicate explicitly assumes
   positive integer weights, pointwise-equal output numerators, and a denominator
   equal to the weight sum. Its conditional lemmas only transport those premises
   to lane positivity and equality of the numerator sum and denominator. This is
   a derived common-denominator invariant, not a proof of real division. The Rust
   `FiniteAlgorithmStateV1` records the maximum, 64 weights, and denominator
   produced by the executable host reference. Neither finite model is claimed
   to refine `f32`.
3. **Mask and empty-row policy.**
   V1 has no mask. `explicit_activity_mask_v1` therefore requires all 64
   positions to participate and rejects mask drift. An empty row is not
   representable, so there is no all-masked fallback or zero-denominator rule.
4. **Explicitly unproved numeric contract.**
   `exp_real_v1` is uninterpreted. Positivity and correspondence between that
   symbol and each supplied real weight are preconditions, not proved facts.
   No theorem connects Verus `real` to Rust/LLVM/AMDGPU `f32`, rounding,
   subnormals, NaNs, infinities, `f32::exp`, an approximation library, reduction
   order, or error bounds.

## What Verus proves

- all active identity-mapped input, scratch, and output indices are in `0..64`;
- distinct lanes own distinct scratch and output elements;
- the exact full activity mask admits every index and excludes masked variants;
- only lane zero participates in the attributed scalar source and its modeled
  barrier count is uniformly zero, matching the source's lack of barriers;
- every active four-byte access lies inside a checked 256-byte row region;
- addresses selected in separate input/output regions are unequal;
- distinct lanes select distinct output and scratch element addresses;
- `stable_softmax_spec_v1` directly supplies each lane's matching numerator
  equation, so substituting lane zero's numerator is rejected;
- the maximum shift is nonpositive under the maximum contract;
- the maximum and denominator reductions have explicit initialized states and
  preserve their completed prefix invariants;
- the positive-weight premise embedded in the real specification conditionally
  gives a positive denominator; and
- the explicit integer premises conditionally transport pointwise numerator
  equality to equality of the numerator sum and assumed denominator.

The memory results are address-set facts only. The zero-barrier theorem models
the exact singleton participating-worker schedule; it is not a workgroup memory
model. The proof does not model concrete memory operations, visibility, or
machine scheduling, so it establishes no source- or machine-level data-race
result.

Three negative mutations must be rejected: `lane + 1` indexing, a duplicate
lane-63/lane-0 output owner, and an actual stable-softmax specification mutation
that substitutes lane zero's numerator for every output lane.
Seven additional named mutations reject a missing/divergent barrier schedule,
mask drift, ownership collision, zero denominator, source-identity drift,
numerical-policy drift, and specialization-width drift.

## Inert evidence certificate

`verification_certificate.rs` exposes a deterministic, inert manifest and an
exact comparison API. It binds the attributed Rust source, reachable portable
MIR and compiler-semantics commitments, typed profile, shared numerical policy,
positive proof source, canonical Kernel IR and LLVM body commitments, exact
`gfx942:xnack-` width-64 specialization, and pinned Verus/Z3 closure. Its
canonical digest is
`5b83efdc0780fa8aa316794371760f1be5ad593256f0e727024e0486bff01898`.

The certificate is formal evidence only. It grants no compiler origin,
source-to-machine refinement, descriptor or artifact admission, load, launch,
or execution authority. It also states that `exp_real_v1` remains
uninterpreted and proves no OCML/IEEE error bound.

## Measurement and trust boundary

`run-verus.sh` fails closed unless the Verus version and complete extracted
release closure match when measured before and after proof execution. The
closure manifest binds all 190 regular files by relative path, mode, length,
and SHA-256, including `verus`, `rust_verify`, `z3`, compiled support artifacts,
and the complete 130-file `vstd` source subtree.
The launcher identity remains:

```text
Version: 0.2026.08.02.b677dd5
SHA-256: ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd
```

The source checker is a small fail-closed Python lexer. It removes line comments,
nested block comments, and ordinary/raw string and character literals before
examining tokens; normalizes identifiers with Unicode NFKC; and rejects Unicode
format, control, and separator categories. This proof uses a closed single-file
policy: the exact `vstd::prelude::*` import must be followed by one balanced
`verus!` block that ends the file. All source attributes, external modules,
additional imports, `include!` forms, declarative macros, and macro invocations
other than that enclosing `verus!` are rejected.

`verus/VERUS_TRUST_VOCABULARY` binds the exact Verus release, upstream commit,
audited `rust_verify` attribute-parser source identity, bundled macro/builtin
source identities, the conservative parser vocabulary, and every reviewed
trust-bearing token. The runner re-derives the verifier-attribute vocabulary
present in the pinned release sources and fails on unknown entries. It also
derives all 177 `rustc_diagnostic_item` names from the pinned
`builtin/src/lib.rs`, checks their sorted inventory digest, and binds each
reviewed trust primitive's diagnostic name, Rust function name, and proof mode.
Drift tests mutate a diagnostic name, function binding, proof mode, and inventory.

GitHub-hosted CI fetches both upstream source files by exact commit, verifies
their pinned sizes and SHA-256 values, and re-runs those structural audits. All
proof-source attributes are denied independently of those lists. Tokens including
`admit`, `assume`, `assume_`, `assume_termination`, `inline_air_stmt`, axiom and
external specification forms are rejected even when comments split adjacent
syntax. Direct access to the `verus_builtin` namespace is also forbidden. The
sole exception is one active, unadorned direct declaration in the outer
`verus!` block with exactly these tokens: `pub uninterp spec fn
exp_real_v1(value: real) -> real;`.

The regression corpus covers split comments, U+200E, NFKC-confusable identifiers,
raw identifiers, qualified builtin calls, strings, nested comments, all audited
trust tokens, `#[path] mod`, external modules, `include!`, `include_str!`,
`include_bytes!`, code-generating macros, and configured, adorned, nested,
duplicate, renamed, or ordinary-definition substitutions of `exp_real_v1`.
Five pinned exploit fixtures are rejected by the scanner and then, only as a
controlled bypass test, passed directly to pinned Verus; each otherwise verifies
a postcondition of `false`. This demonstrates why scanner admission is required.

These are point-in-time source and release-closure measurements, not an
immutable or descriptor-bound Verus execution. Proof execution clears the
ambient environment and fixes `VERUS_Z3_PATH` to the measured sibling. A
malicious process with the same UID can modify and restore proof, checker, or
closure files between measurements and use; that attack is explicitly outside
this example's local trust model. The checks also do **not** authenticate the
host OS, shell and Python utilities, dynamic libraries, or rustup toolchain, and
they do **not** bind the proof to generated LLVM IR, ISA, HSACO, loading, launch,
or observed GPU execution.

PR, merge-group, and untrusted branch checks run only on GitHub-hosted runners.
The persistent reviewed-host job is in a separate workflow restricted to the
exact `harsh-nod/fe2o3` repository, `refs/heads/main`, and either a main push or
manual dispatch. Mutation tests reject weakened repository, ref, event, branch,
or runner policies.

On that trusted workflow, the reviewed-host job first runs the repository's
authenticated Verus V2 controller tests, which use sealed executable snapshots
and descriptor-bound inputs. Stock Verus and Z3 do not implement that
controller's nonce protocol, so this row proof still runs through the
point-in-time path afterward. The controller test is a deployment prerequisite.
It is not authenticated execution of the row proof. It also makes no same-UID
replacement-resistance claim for that proof.

The positive proof pin is 12,966 bytes with SHA-256
`cacf81e02eb071cc29b1124811e911097fd62e7d29556dda8380418a631f5db5`;
all ten semantic-negative and five trust-exploit sources are pinned
independently as well.

## Commands

```text
cargo fmt --manifest-path examples/row_softmax_v1/Cargo.toml \
  --package fe2o3-row-softmax-v1 -- --check
cargo test --locked --manifest-path examples/row_softmax_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/row_softmax_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/row_softmax_v1/Cargo.toml \
  --all-targets --all-features -- -D warnings
VERUS=/absolute/path/to/pinned/verus examples/row_softmax_v1/run-verus.sh
```

## Remaining proof obligations

- prove the selected real exponential laws instead of accepting
  `exp_weights_contract_v1`;
- supply and verify the real field/division lemmas needed to derive positivity
  and exact normalization of the real output sequence from the V1 relation;
- define and verify an `f32` exponential approximation with stated ULP/error
  bounds and exceptional-value policy;
- refine finite `f32` max, exp, sum, and divide operations to an error-aware
  specification, including the exact parallel reduction order;
- model concrete memory operations, barriers, memory order, and conflicting
  accesses before making any data-race claim;
- bind the Rust kernel body through MIR/LLVM/AMDGPU lowering to emitted HSACO;
- authenticate descriptor/finalizer/runtime admission and hardware results; and
- add masking, all-masked rows, variable row widths, batches, and striding.
