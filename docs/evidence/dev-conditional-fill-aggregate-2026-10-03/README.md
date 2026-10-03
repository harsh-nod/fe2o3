# Conditional Fill Aggregate

This checkpoint advances the verified multi-GPU fill path from unsigned
conditional staging to an actual protected conditional aggregate proof. It does
not admit an artifact or a launch, complete multi-GPU application execution, or
establish HIP/HSA parity.

Parent commit: `5000d10fb2d5d4a1daccaa2ee0ac2ad4cd77e9b9`.

## Implementation

The new verifier entry derives its source and subjects from the exact retained
ranked owner and matching V5 evidence. It recomputes conditional staging,
requires one effect and its exact retained receipt, and supports the unsigned
32-bit identity-output profile. Ranked element widths are bits; physical buffer
extents are bytes. Unsupported widths, value types and numerical policies fail
closed.

For every integer invocation coordinate `i` and unsigned 64-bit lengths `N` and
`G`, the generated theorem requires `N <= G` and proves:

- Launched, guarded membership is exactly `0 <= i < N`.
- Within that output domain, actual and reference coordinates both equal `i`.
- Actual and reference domain/precondition expressions are true.
- The actual stored-value and reference-value expressions agree.

The expressions occur in the theorem's postcondition, using the same bounded,
topologically ordered formula renderer as existing value proofs. The theorem
does not infer semantics from hashes, assume the desired equality, or rely on a
call to an assertion-only lemma. The live structural classifier supplies unique
writers, absence of extra writes and normal completion. Classifier soundness,
compiler extraction and projection remain trusted; the generated theorem does
not independently verify their implementations.

The protected executor signs and strictly imports a distinct boundary,
`SafeReferenceMirToLivePlironConditionalCoverage`. Existing unconditional
boundary-3 APIs, TotalView counters and aggregate evidence types are unchanged.
Cross-boundary import and unsigned boundary retagging reject.

The move-only execution owner retains the exact signed receipt, verifying key,
pending staging record and complete signed-obligation preimage. That preimage
binds source/ranked/middle-end identities, generated source, all coverage
locations and layout restrictions, both distinct argument ordinals, the effect
site and component receipt/subject/toolchain identities. It is not yet a
conditionally admitted transport codec. Embedded-key verification proves
internal authenticity, not protected compiler origin.

The actual compiler path executes this new proof, retaining its execution owner
and policy in a separate pending-launch error. It never supplies this owner to
the old unconditional semantic/parallel/aggregate path and does not emit an
unproved compiler handoff. The full V9 application acceptance test remains an
unfinished, ignored target.

## Qualification

Final qualification passed 700 CPU tests and three protected integration tests:

- Functional receipt import: 7 passed, including both cross-boundary directions
  and signature rejection after retagging.
- Verifier library: 139 passed, 7 ignored. The new tests inspect actual formula
  postconditions and reject unsupported value types and malformed pair rosters.
- Compiler library: 524 passed.
- V5 middle-end evidence and conditional staging: 28 passed, including distinct
  ranked/reference ordinals, retained subgroup size and 32-bit element width.
- Conditional owner compile-fail tests: 2 passed. The owner is neither cloneable
  nor convertible into an unconditional aggregate execution.
- Protected conditional theorem: one positive and six logical negatives passed
  in 73.25 seconds. Removing/reversing `N <= G`, an off-by-one guard, equal wrong
  coordinates, equal false predicates and a changed value all require actual
  Verus assertion/postcondition failure, not parsing or runtime-policy rejection.
- Genuine AMD Rust extraction: both tests passed in 98.83 seconds. The positive
  fill reaches the new gate carrying the strictly imported boundary-4 execution
  and policy; a changed reference value fails an actual assertion. Neither emits
  a compiler handoff.
- Strict library Clippy passed for verifier, compiler and functional-proof crates
  with warnings denied. Targeted formatting and whitespace checks passed.

An earlier genuine positive test failed because the new profile check compared
ranked bit width against 4 rather than 32. This was corrected, documented on the
getter and covered by the staging assertion before the final complete rerun.
The archived failed attempt is development evidence, not a qualification pass.

[Qualification archive](qualification.tar.xz), SHA256
`7864f04b1b25918e42b90c6cd8bcab89d81d4f3fcff22c33672945a026ae4a3f`,
contains the exact candidate patch, command scripts, final transcripts and the
width-failure transcript. Candidate patch SHA256:
`dfa106efd1cf13f4ae8b62e30d1ca07982812f6e9892fe8d49b52d192610e8fd`.
The archive integrity and extracted patch digest were checked.

The harness uses `nightly-2026-04-03`, four Cargo jobs, serial tests, test
opt-level 1, enabled debug assertions/overflow checks and no incremental build.
Protected verification uses the unchanged runtime manifest
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
The archived prerequisite script checks the exact downloaded input hashes
before extraction. Root provisioning occurs only inside the private namespace;
proof controllers run as UID/GID 1000 without groups, capabilities, network or
GPU devices. Host `/opt` and installed libraries were not changed. This is
development qualification, not a deployed compiler-origin/application provider.

## Remaining Multi GPU Path

1. Add a distinct bounded conditional evidence codec/import owner. Decode the
   retained obligation fields, recompute the digest against the signed binding,
   and cross-check exact source/ranked/middle-end stages. Reuse the inert V4
   envelope; do not widen its unconditional validator or fabricate TotalView.
2. Interpret the retained output ordinal against the same authenticated semantic
   root and descriptor. The existing reference-effect join already maps the
   source output to its allocation and effect. Preserve its distinction from
   the ranked extent ordinal; do not assume equal ordinal numbers. Derive the
   physical length offset from the matched `SliceLengthU64` component.
3. Discharge the condition on the actual packed arguments, checked byte extent,
   pointer fixup, actual AQL global-X extent and retained layout restrictions.
   Bind a private consumed token to the same artifact, publication and prepared
   launch. Workgroup size is not global launch size.
4. Finish complete source/neutral/optimized guard-index-address-value refinement
   and physical dispatch coverage for the existing fill machine model. Connect
   the protected application provider, then run admitted fill, tracked upload,
   native peer transfer and readback for lengths 64, 65 and 4097 in both
   directions on freshly observed idle MI300X devices.

No MI300X job or remote file was created for this checkpoint. Protected local
qualification uses the unchanged pinned runtime in a private mount namespace,
with `/opt` and `/tmp` on namespace-local tmpfs and no GPU devices.
The private runtime namespace and this task's temporary downloaded inputs,
library tree and original logs were removed after the archive was checked.
