# Worker V3 Generated-Only Bootstrap

CPU development above base `9b9265c6919cb8dff9506f2c6ffa7b7f2538905f`,
bound to the exact uncommitted repository source map in `raw/source-before.log`.
This is not protected native execution, sandbox composition, formal refinement,
performance acceptance or HIP/HSA parity. Accepted R125 Native CPU/test,
R118B C1/C2/C3 and R116/V3 checkpoints are unchanged; A1/A2 and #182 remain open.

## Boundary Changes

The safe `KfdRuntimeBackendV1::open_worker_v3_generated_only_v1` constructor and
checked-device sibling expose the allocation, stream and copy facilities needed
by protected generated adoption without requiring generic launch authority.
The private generated-only gate advertises no generic typed, atomic or collective
launch capability, and no generic compute concurrency or compute/copy overlap.
Its generic authorization result is false. It does not change the protected
generated ISSUE/adoption route or create a verifier/refinement provider.

A new runtime test uses panic-on-encode/bindings arguments to establish that
ordinary, atomic and collective facade submissions reject before either callback.
It checks the capability restrictions and clean module/stream shutdown. All 37
existing generated ISSUE tests now use the generated-only mock profile, exercising
that route with its existing CPU fixture authority, not native protected authority.

The unsafe startup-only `authenticate_inherited_worker_v3_application_v1` helper
consumes the inherited handoff and returns the original authenticated linear
executable owner. Runtime construction and argument reservation remain separate.
Its cooperative startup safety contract is unchanged from the handoff consumer:
it must precede threads, relevant signal handlers, descendants and unrelated
descriptor/environment mutation. The unsafe inventory increases the containing
file from two functions/two blocks to three functions/three blocks; no blanket
exception or suppression was added.

The older synchronous preparation helper delegates through this authentication
step and preserves its old public Handoff/Preparation error nesting. One host
unit test covers that error mapping. The actual handoff-before-verification
sequence is source-reviewed, not established by this mapping test or a new
process-environment integration test. The public no-default-feature API test
and compile-only doctests exercise the new exports and type linkage.

## Qualification Scope

`qualify.sh` records serial commands, UTC start/end times, complete combined
stdout/stderr and exit status. GNU and scoped musl each pass 1,102 runtime tests
with 18 ignored, and 283 host tests with four ignored. Ignored native/opt-in tests
are not passes. Musl disables optional legacy HIP linkage using
`FE2O3_HIP_SYS_DISABLE=1`; it does not replace direct KFD with a fallback.

The verifier parses entire harnesses using a hash-pinned parser, preserves all
prior runtime and host outcomes, and requires exactly one new passing test per
crate on each target. It binds four actual test executable paths and hashes
before/after execution to the recorded build and run outputs. Source maps cover
5,547 selected repository files, including all crate sources, Cargo manifests,
examples, copy benchmarks and the unsafe inventory. External Cargo cache,
compiler, system libraries and headers are not a hermetic input closure.

All 72 GNU doctests pass: host five compile-only and 21 compile-fail, runtime
four compile-only and 42 compile-fail. No doctest executes a native kernel.
The public API test, strict all-feature/all-target Clippy, no-default-feature
checks and workspace formatting pass. The unsafe-policy harness passes five
tests with one maintenance case ignored. All 24 receipts are closed and exit
zero. `verify.py` rejects incomplete receipts and verifies current
source/executable identity before sealing. Current-source verification needs the
qualified workspace; the manifest remains portable. Evidence scripts and this
README are finalized separately from the repository source snapshot.

The production protected verifier/refinement provider and corresponding
semantic-to-machine proof artifacts remain missing. This bootstrap does not
alter seccomp, establish protected typed bundle execution, or complete A1/A2.

## Development History

Before this archive was created, focused CPU tests, two compile-only doctests,
formatting and a read-only code review passed. An initial exact-name filter
matched zero tests and was corrected; an initial compile found a test-only enum
qualification error, also corrected. Those exploratory command transcripts were
not retained here and are not counted as full qualification receipts.

No MI300X job or remote directory was created for this CPU packet. The separate
matched KFD/HSA campaign used committed base source, not these working-tree edits.
