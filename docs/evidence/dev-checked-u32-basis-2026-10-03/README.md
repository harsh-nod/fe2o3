# Checked U32 Argument-Basis Qualification

Candidate baseline: `902e05a54a665674d7f4106f123b161cf8690a8b`.
This checkpoint proves the argument-basis initialization used by the existing
[entry-prefix checker](../../runtime-checked-u32-prefix-v1.md). It also records
the reviewed [multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).
It does not implement frame-to-list forwarding or grant application authority.

## Implementation And Proof

Production and Verus include one executable initializer over the existing
argument-row shape. It accepts exactly matching scratch length, positional
argument labels and bounded, distinct source locals. Accepted rows establish
paired source/KIR argument origins and uninitialized unmapped source cells.
The contract includes acceptance completeness: an always-reject implementation
fails the proof. Private scratch on rejection is discarded, not rolled back.

The initializer uses O(locals + arguments) work and constant extra space beyond
caller scratch. No second parser, input mapping or proof-only executable body
replaces the production operation. A denotation lemma and an uninitialized KIR
suffix bridge compose its result with the existing concrete-fold theorem.
KIR identity uniqueness remains checked by the existing adapter, not this proof.

Whole-file pins bind the actual wrapper, row schema, caller and dense KIR slot
insertion. They do not prove ABI discovery or statement normalization. Physical
machine entry, continuation, memory, completion and protected invocation custody
remain outside this proof. The 18 results are verifier obligations, not 18
independent end-to-end theorems.

## Final-Source Results

| Check | Result |
| --- | --- |
| Verifier all-feature library | 120 passed, four existing ignores; includes six new basis tests and exhaustive small injective-map cases |
| Public checked-add integration | Seven passed |
| Verifier doctests | 30 passed, including owner-lifetime compile failures |
| Genuine Rust-source extraction | One integration test passed: two actual source profiles accepted, four specific unsupported cases rejected |
| Production-library Clippy | Passed with `-D warnings` |
| No-default library and changed-module formatting | Passed |
| Source-bound proof campaign | 21 accepted stages; two whole-proof positives, each 18 verified/zero errors; 16 logical negatives; eight runner controls |
| Local CI dispatch and actual proof shell wrapper | Passed; wrapper-owned scratch removed |

Seven mutants fail an exact postcondition and nine fail an exact loop invariant.
Eight target the fold and eight target the initializer. Parser/type/bounds errors,
timeouts and cross-target diagnostics do not count as logical rejection. Source
controls reject row-schema changes, wrapper argument swaps, removal of the actual
call and incorrect KIR slot forwarding.

The extraction witness rebuilds and runs the current compiler wrapper against
ordinary Rust sources. It checks nonempty copy/reassignment and direct-argument
prefixes, conditional boundary evaluations and specific fail-closed diagnostics.
It emits no HSACO or launch artifact and is not GPU execution.

`qualification.tar.xz` contains command/environment receipts, source inventories,
tool/binary identities, staged positive/negative proof sources, logs, the candidate
patch and audit scripts. Ten accepted final-source stages share one 6,872-file
cohort; the proof campaign binds 25 source files. The audit checks source/output
hashes, test summaries, exact staged mutants and logical diagnostic classification.
Every accepted stage records closed owned process groups and unchanged source.

Earlier solver-development failures, a multiline control mismatch, the runner's
target-variable shadowing failure, a misspelled integration-test target and the
pre-format check are retained as excluded diagnostics. Earlier passing cohorts
are not substituted for the final-source roster. The target-shadowing defect is
fixed by keeping theorem selection separate from staging destination paths.

No MI300X resources, GPU workloads or performance measurements were added.
Compiler and shell-owned temporary directories are absent after qualification.
This is not workspace-wide testing, complete adapter refinement, A3 completion
or HIP/HSA parity.
