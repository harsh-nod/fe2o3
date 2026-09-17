# Generated Typed Completion Candidate

Development only. See [the C5 contract](../../runtime-generated-typed-completion-v1.md).
R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain accepted; C5, A1/A2,
#182, protected native composition, formal correspondence and HIP/HSA parity
are not accepted by this packet.

The frozen-source CPU qualification is complete. GNU and scoped musl each pass
915 runtime tests with seventeen ignored and 271 host tests with four ignored.
Complete rosters match exactly, and source/executable hashes agree before and
after both harnesses. Strict all-features/all-targets Clippy, workspace formatting,
both crates' no-default-features checks, unsafe-source policy (five passes and one
maintenance ignore) and all 58 doctests pass. The doctests include actual public
await/join type linkage and move-only contract failures. The audit checks exact
commands, complete rosters, source identity, executable identity and GNU -> musl
-> gates -> parser ordering. Parser calibration rejects eighteen malformed
transcripts, including trailing failures or extra harnesses after a valid footer.
The final audit is recorded outside `raw/` in `qualification-audit.*`; the archive
is sealed with `SHA256SUMS` after its successful completion.

`raw/exploratory-*` records are development diagnostics, not frozen-source
qualification. They must not be substituted for final-source test results.

## Frozen Source

The eighteen Rust source files are now frozen above canonical C4 commit
`8fa6485ec18e2f0126e2ae4804ed1d7160cb7d15`. The complete source patch has SHA-256
`116bd039b4c2cc5e17252ac5a4a6f51aae59bdd4ad0843a047bf976b27317209`;
the file manifest has SHA-256
`5b9775d3e2259171e9119aefdc884650f29c73cd39eded296dea19ef2067743a`.
The manifest's exact filename roster must equal the changed-source roster.

The musl cohort deliberately uses `FE2O3_HIP_SYS_DISABLE=1` while retaining
Cargo's all-features configuration. This disables optional legacy native HIP
discovery/linkage, not direct KFD. The missing musl C toolchain failure was already
established and preserved in the preceding C4 archive; this packet does not claim
native HIP validation or an unrestricted musl build.

## Development History

The initial broad exploratory run found an old readiness-panic test that dropped
the newly returned completion observer but still expected its reply credit to
remain held. Its assertion under a fixture mutex also caused a destructor panic.
`exploratory-readiness` preserves the exact reproduction (exit 134). The test now
retains the public observer, checks exact error delivery, and separately verifies
retained driver ownership after reply-credit release. Assertions no longer keep
the fixture mutex locked. A separate domain-fault test initially failed to compile
because it accessed a private ticket field (`exploratory-domain-faults`, exit
101); it now checks exact retained gate ownership without widening that field.
These failed records remain history, not successful qualification.

The first strict Clippy pass also failed (`exploratory-clippy`, exit 101): older
tests implicitly discarded returned futures and the new typed API carried a
larger general argument error than needed. Explicit drops and a narrow typed
output error enum correct those issues without allocating error boxes. The
separate corrected exploratory Clippy run passes; the frozen gates remain
independent requirements.

CPU coverage is compositional: runtime tests exercise receipt minting from the
original completion cell; host tests exercise exact domain matching, original
typed allocations, credit lifetime and shared typed observer control flow using
inert metadata. Public compile-only examples check actual API linkage and Send
observers. No receipt constructor or native-authority bypass is added for tests.
Authentic protected typed success and post-receipt native/fault campaigns remain
open, as do automatic heterogeneous bundle collection and C6 graph integration.

This packet runs no MI300X workload and makes no performance comparison.
