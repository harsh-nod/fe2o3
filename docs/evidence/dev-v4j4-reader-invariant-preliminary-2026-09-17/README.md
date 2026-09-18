# V4-J4 Stopped Preliminary Qualification

This first qualification stopped with exit 1 in `invariant-campaign` and is not
acceptance evidence. The first seven command records passed; the eighth failed.
The complete positive crate passed 156/0. The first mutation produced exactly
155/1 with the intended invariant postcondition failure, but the unchanged strict
parser rejected its secondary diagnostic span: an implicit unit return was
located at the function signature, outside the required body range. No negative
was accepted, and no later Rust/integrated qualification command ran.

The successor changes only the generated subject's exit to explicit `return ();`,
preserving the invariant, premises, mutations and parser. This directory was
moved from `dev-v4j4-reader-invariant-2026-09-17` after the command terminated;
raw source/command paths retain their true original execution locations.
The original prospective `seal.sh` was never run: it correctly requires the
complete successful 22-record qualification, which this stopped run lacks.
`SHA256SUMS` instead seals this explicitly unsuccessful history.

## Source And Scope

Base: `f815d1dc7406851e39e77f488b6bc4b4b3f94ae2`. Eleven source/documentation
files are captured by `source-files.list`, `source-files.sha256` and `source.patch`.
Source manifest SHA:
`f9096736466e74a55014fbd165426fbd118c284f593d48e69c818ac0c83bba41`.
Patch SHA:
`8c285592c643ab9b88a240d22c742ffb42ecce01fd0f1df36e62ff2646b543b4`.

The [development contract](../../runtime-context-read-invariant-v1.md) describes
the concrete partition, exact reader multiplicities, identity/version validity,
constructor and acquisition/release preservation, writer register/abort reader
framing, and no-reissue theorem over exact modeled operation traces. The proof
has 155 whole-crate obligations: 127 inherited and 28 new. J3, J2 and J1 are
included/imported as unchanged exact pinned nominal type instances.

The trace begins with a reader-valid state and watermark 1, possibly already
enrolled. The nonempty executable witness checks fixture enrollment, overlapping
consumers, partial release, last-reader exclusion, slot reuse and stale-reference
rejection. It is a verified call chain, not a separately assembled ghost trace.
Neither establishes general production enrollment refinement or complete base
writer/member invariants from arbitrary reader-valid states.

Production behavior is unchanged. General writer membership/settlement,
retirement/Unknown disposal correspondence, physical storage/capacity, unwind,
authenticated quiescence, ordinary/generated Context custody, Rust/native
refinement and machine-code execution remain separate. Accepted Native R125
CPU/test, Admission R118B C1-C3 and Resources R116/V3 checkpoints are unchanged.
A1/A2, #182 and HIP/HSA parity remain incomplete.

## Original Qualification Plan

`qualify.sh` records twenty-two ordered commands with full output and exit status.
The standalone invariant campaign adds one test-only executable subject whose
only postcondition is the invariant. It requires two exact 156/0 positives and
sixteen 155/1 negatives at the exact intended postcondition/exit spans. These are
invariant-sensitivity mutations, not production reader-body mutations. J3's
independent body-mutation campaign is retained in the complete integrated gate.
Compilation, timeout, extra or unrelated errors cannot qualify as negatives.

The planned GNU/scoped-musl, roster, lint, format, no-default, unsafe-source,
doctest, integrated Verus and final identity records must all close before
sealing. The scoped musl run disables legacy HIP via `FE2O3_HIP_SYS_DISABLE=1`.
The six test-produced executables are first hashed after both Rust test runs;
the final check covers the later interval, not a pre-test binary identity claim.

The Rust whole-state inspector checks the complete reader arena, while a new
lifecycle trace exercises every exposed mutating wrapper category with retained
readers on unrelated allocations and unchanged storage addresses/capacities.
The existing 4,000-step multi-consumer trace also invokes the inspector. These
are executable witnesses, not mechanical Rust refinement.

No GPU workload, remote build directory or GPU benchmark artifact is created by
this qualification. Hardware performance and shared-host admission are separate.
