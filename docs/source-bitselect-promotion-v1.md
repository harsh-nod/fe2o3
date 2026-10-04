# Bounded source promotion: Rust to an ordered assembly expression

The experimental Linux Rust API `run_bitselect_source_promotion_driver_v1` creates
a distinct Rust candidate file from one eligible bitselect initializer. It does
not mutate the original, resume a compiler snapshot, compile the candidate
automatically, or launch a kernel. This is a narrow integration seam for authoring
tools, not a general decompiler or a new command-line/wire protocol.

## Supported starting point

Use the repository's pinned nightly and existing targeted invocation/provider
setup. The kernel must be the sole sealed, local, nongeneric kernel root for
`gfx942:xnack-`, wave64, with required and maximum launch bounds `[64, 1, 1]`.
The sole eligible top-level immutable initializer must be the direct typed-u32 expression
below; its inputs are three distinct immutable formal parameters in ordinals
1, 2 and 3. Existing typed-HIR, semantic and Kernel IR checks must all agree.

```rust
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn choose_bits(mut output: DisjointSlice<u32>, a: u32, b: u32, mask: u32) {
    let selected = b ^ ((a ^ b) & mask);
    let index = thread::index_1d();
    if let Some(slot) = output.get_mut(index) {
        *slot = selected;
    }
}
```

This excerpt uses the imports and feature setup of
[`source_bitselect_feasibility.rs`](../crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/source_bitselect_feasibility.rs).
Macros, aliases, extra eligible initializers, source normalization, escaping
intermediates, unsupported effects and ambiguous attribution are refused.
Merely matching source text or supplying a source range is insufficient.

Up to eight preceding immutable `u32` bindings are supported when each is one
direct AND, OR or XOR of those original formals. They cannot depend on another
prefix binding, shadow any formal or selected result, invoke a call or macro,
branch, mutate state, or perform a memory operation. Their exact source stays
outside the replacement; the current semantic and Kernel IR owners must still
agree on the selected contiguous three-operation region in the entry block.

For example, this live surrounding computation remains ordinary Rust:

```rust
let tag = a | mask;
let selected = b ^ ((a ^ b) & mask);
// Inside the existing checked output branch:
*slot = selected ^ tag;
```

Only the `selected` initializer is promoted. This does not add arbitrary
statement selection, instruction scheduling, or a continuation from an old IR
snapshot.

Prefix eligibility does not guarantee that optimization preserves the selected
operator's exact attribution. If optimized MIR reuses an identical earlier
expression, the source/semantic join can refuse the promotion. It must not
substitute matching values for the missing selected source identity.

## Request a candidate

The following integration excerpt assumes the caller has already computed the
SHA256 of the exact original file bytes and obtained a complete, authentic
current targeted rustc invocation from the existing provider. A hash is only a
revision-conflict precondition; it does not authenticate dependencies or replace
the compiler's source/owner checks.

```rust
use fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1;
use rustc_codegen_fe2o3::{
    BitselectPromotionFailureV1, BitselectPromotionRequestV1,
    PublishedBitselectCandidateV1, run_bitselect_source_promotion_driver_v1,
};

fn promote(
    rustc_args: &[String],
    original_sha256: [u8; 32],
) -> Result<PublishedBitselectCandidateV1, BitselectPromotionFailureV1> {
    let registers = Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2])
        .expect("five distinct registers below 64");
    let request = BitselectPromotionRequestV1::new(
        "src/original.rs",
        "src/candidate.rs",
        original_sha256,
        registers,
    )?;
    let attempt = run_bitselect_source_promotion_driver_v1(rustc_args, request);
    let (_original_request, result) = attempt.into_parts();
    result
}
```

The caller owns process isolation, a stable working directory/environment, the
complete bounded argv, existing provider metadata and an external deadline.
The entry checks the canonical overflow-check setting as well as the live
target/closure. A fabricated list containing only a target flag is not enough.
Use one isolated invocation, not concurrent in-process compilations or mutable
global environment. The independent normal-dependency
[consumer fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/source-bitselect-headless-consumer)
demonstrates linkage and error propagation; its fixed `promote_once` executable
is a test fixture, not a supported product CLI.

Paths must satisfy the existing relative `.rs` path policy (at most 1024 bytes
each), must be distinct, and must refer through permitted retained filesystem
parents. The candidate destination must not exist. The publisher stages a
private mode-0600 file and publishes without replacing an existing destination.
Do not treat these point-in-time checks as a filesystem transaction or a promise
against later ancestor renames or other writers.

## Inspect and edit the generated Rust

For the register plan above, the initializer becomes:

```rust
let selected = fe2o3_device::amdgpu_ordered_program! {
    gfx942_xnack_off_wave64;
    scratch(4); out(5);
    in(0) = a;
    in(1) = b;
    in(2) = mask;
    xor(scratch, input0, input1);
    and(scratch, scratch, input2);
    xor(out, input1, scratch);
};
```

Surrounding Rust and the output use are preserved. The source remains editable.
For the supported register-only exercise, create another distinct file and
change scratch/output/inputs to `32, 33, [34, 35, 36]`. All five roles must be
distinct and below 64. Never infer that arbitrary instructions, overlapping
registers, another architecture, or a different expression have been qualified.

Compile the actual candidate file afresh through the ordinary source-input
path. The returned candidate hash, length and register plan are historical
observations, not reusable admission or launch authority. Editing the candidate
invalidates its old source identity; the fresh compiler must derive new typed,
semantic, Kernel IR and LLVM identities from the bytes actually read.

## Separate instruction-changing exercise

The publisher still recognizes and renders the original bitselect; it does not
accept an arbitrary replacement graph. After publication, an author can make
a real source edit and request fresh ordinary diagnostic compilation. The
separate instruction-edit exercise changes only the final
`xor(out, input1, scratch)` to `or(out, input1, scratch)`, keeping low-register
bindings `4, 5, [0, 1, 2]` and all surrounding source bytes.

The edited result is `b | (a & mask)`, not the original
`(a & mask) | (b & !mask)`. A source edit can be well-formed and still be the
wrong algorithm. Each variant needs its own independent whole-kernel oracle.
The normal public-seed/export/inspection/simulation runner is
[`source-promotion-instruction-edit-smoke.mjs`](../scripts/source-promotion-instruction-edit-smoke.mjs);
its output is diagnostic evidence, not production proof or native qualification.
Follow the [instruction-edit tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/source-promotion-instruction-edit-lab-v1.md)
for fresh preparation, exact runner inputs and separate native requirements.

A semantic-identity comparison must use the normal exporter's observation from
the same live owner. In the runner's explicit unavailable mode, semantic
identity stays unavailable; source preflight hashes or reconstructed IR cannot
supply it. Existing register-only, live-prefix or older native reports do not
qualify this new instruction sequence.

### Current inspection is not proof invalidation

The public-seeded debug-join regression has a separate current-owner inspection
check. It copies an actual first owner's inert typed identity, requires exact
`ProductionOrderedProgramInspectionErrorV1::StaleIdentity` when that identity
is queried against the second current owner, and then queries the second owner
with its own identity on the same accounting ledger. Owner bytes and the
retained storage floor must remain unchanged. This is a test of an existing
borrowed inspection API, not a new identity import or session-resume interface.

The regression's register-edited pair is separate from the instruction-changing
runner. Successful stale inspection or debugger capture/cursor rejection does
not establish general analysis-cache invalidation, ranked-check reuse rules or
production proof invalidation. Those require their applicable production owner
and proof route; the diagnostic reports keep those authority flags false.

## Where this fits in compilation

```text
original Rust -> fresh typed HIR + semantic/Kernel IR checks
              -> distinct generated Rust candidate
              -> optional supported source edit
              -> separate fresh compilation
              -> typed ordered region -> constrained LLVM inline assembly
              -> normal native compilation and final-artifact checks
```

The promotion call consumes the ordinary collected transaction through import,
semantic middle end, SSA, target-neutral materialization, general kernel checks
and attachment of those checks. Its narrow source/semantic/Kernel IR join then
renders and publishes the expression; the live compiler owner does not escape.

The fresh compilation still uses LLVM IR. The ordered region is represented by
constrained inline assembly; LLVM still handles the surrounding kernel.
Instruction/register control inside this region is not complete control over
the whole machine program, a proof of register lifetimes, or permission to skip
the rest of the pipeline. Final instruction and descriptor observations must be
checked on the actual resulting artifact.

You can manually replace this bounded expression with equivalent Rust and
compile again, but this API does not lift arbitrary assembly back to Rust,
preserve arbitrary edits across regeneration, recover historical compiler
owners, or provide reversible multi-level snapshots.

## Refusals and failure handling

Inspect `phase()`, `publication()` and `compiler_fatal()`; `Display` and
`std::error::Error` expose the same diagnostic bounded to 4096 UTF-8 bytes.

- Completed request, frontend or eligibility refusals before publication report
  `NotAttempted`. Zero entered callbacks also report `NotAttempted`.
- Once publication is invoked, any publication error or subsequent source
  recheck error reports `MayHaveCreatedCandidate`. An existing destination can
  produce this conservative state without creating a new file.
- An entered but unfinished callback also reports `MayHaveCreatedCandidate`:
  the driver cannot establish how far it progressed. A caught rustc fatal sets
  `compiler_fatal`. An earlier completed refusal keeps its original phase,
  diagnostic and publication classification.
- A successful publication can be followed by a failure. Do not blindly retry,
  overwrite or delete the candidate. Inspect retained evidence and the current
  filesystem, then make a separate explicit decision.
- Unexpected ordinary panics, process aborts and crashes are not guaranteed to
  become typed outcomes. No rollback or crash-cleanup guarantee is provided.

The original profile is at most 64 KiB; generated replacement at most 128 KiB.
Argv is at most 4096 strings and 1 MiB total. Own-work scan/payload and retained-I/O
budgets remain bounded separately from compiler resource ledgers and process RSS;
this API does not relax compiler quotas.

## Qualification and its limits

The source includes distinct acceptance layers:

1. A normal backend build and independent normal-library consumer, without
   `cfg(test)` or backend dev-dependency assistance.
2. Real source promotion: success, wrong target, wrong launch, genuinely stale
   original and preexisting destination; a separate fresh candidate callback
   checks 128 Boolean-oracle vectors.
3. A normal external publication followed by fresh default, edited and repeated
   source compilation; 90 whole-kernel simulator runs compare an independent
   `(a & mask) | (b & !mask)` oracle, output/canaries and repeat identities.
   Wrong plan, wrong output and stale edited-source cases refuse.
4. Test-only live-callback and real rustc fatal injection controls check retained
   file identity and conservative errors. They are not callable through the
   normal API. A dependency-local directory-sync refusal control is component
   coverage, not an actual OS failure through this public entry.
5. Separate native O0/O3 checks must consume LLVM emitted by the new public-entry
   candidate workflow and inspect exact instructions, physical operands, result
   use and descriptor capacities.
6. Public-seeded generated-negative and debug-join ladders connect the normal
   consumer to exact resource/boundary refusals and stale capture/catalog
   identity refusals. Capture identity checks do not supply a portable capture
   importer or qualify production proof invalidation.
7. A live-prefix ladder preserves the surrounding source and checks a separate
   whole-kernel `((a & mask) | (b & !mask)) ^ (a | mask)` oracle for default,
   edited and repeated candidates. Its bounded public-API controls cover the
   eight-binding limit and exact unsupported-prefix refusals.

These layers are separate obligations, not interchangeable test counts. A passing
simulator run does not prove complete race freedom or GPU execution. Native
checker reports are observations of selected output, not protected finalizer
admission, authenticated build ancestry or physical register-lifetime proofs.
General source promotion, arbitrary edited graphs, more target profiles and full
bidirectional authoring remain outside this bounded API.

Dated results and their limits are recorded in the
[September 22 evidence](evidence/authoring-source-values-20260922.md) and
[milestone contract review](assembly-authoring-contract-review-20260922.md).
