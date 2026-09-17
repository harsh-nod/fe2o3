# N5 Borrowed Native-Source Prerequisite

This development packet adds the private input handoff needed by generated
DATA-ADOPT. It does not install production adoption hooks, allocate or bind
native DATA, publish a dispatch, retire native owners or complete N5. R125
remains the accepted Native CPU/test checkpoint; R126 qualification, A1/A2,
issue #182, formal correspondence and HIP/HSA parity remain open.

## Source And Behavior

The seven-file Rust delta is based on
`8e16409976a8cbfb9d54dffb354f3dac917b72dd`. `source.patch` records the complete
delta, `source-files.sha256` identifies the files, and `source/` preserves their
final contents as `.rs.txt` evidence snapshots, not compiled repository modules.
The preceding native evidence packet qualifies its own source
and binary, not this subsequently changed runtime.

`RuntimeGfx942GeneratedSourceV1::with_native_inputs_v1` joins the original HSACO
length/digest, existing Worker authority, selected device and exact reserved
source occurrence/roster. It reconstructs a loader-validated program from the
original HSACO, checks the materialized image and descriptor resources, and
shares ABI reconciliation with the existing persistent projection path. The
callback borrows every original buffer, including unused complete extents.
No artifact, buffer contents, authority or packet is duplicated.

The complete original dispatch-contract digest was checked before packet/data
separation. This view joins that stored identity to the authority and exact
source occurrence; it does not claim to recompute the complete digest without
the original packet's private kernarg bytes. The native caller still needs the
exact one-shot packet transfer into its rooted backend phase.

A higher-ranked callback lifetime prevents safe program/buffer borrow escape.
Its concrete return type is `Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>`,
which carries no native owner. Authority currentness is checked during source
validation, immediately before callback entry and after every normal return,
including callback failure. Closing rejection overrides that owner-free result;
callback or currentness panics propagate without a second check during unwind.
The wrapper does not retry, quarantine or dispose backend resources. A callback
performing native work must already root every admitted prefix, and its outer
runtime envelope must terminalize failures before releasing any hold or credit.

The input view itself is stage-neutral: it leaves packet availability unchanged
both before and after a separate one-shot transfer. Source checks alone are not
adoption, publication authority, a lane lease or proof of native cleanup.

## Qualification

Final GNU and musl runtime suites each pass **852 tests**, with eleven native
tests ignored. Each target's focused suite passes all ten new tests. All 33
runtime doctests pass; strict Clippy, formatting and no-default compilation
pass. The final unsafe-source policy passes five tests with its explicit
maintenance test ignored. All 26 final command records have exit zero.

Ten focused CPU tests cover original program/buffer pointers and all retained
bytes, independent ABI reconciliation, exact roster substitution, artifact and
authority substitution, malformed retained program/ABI data, callback failure
classification, both opening currentness boundaries, closing failure/panic, and
first-panic preservation. The test authority is nonexecuting and never supplies
a native checked-device token. Test references to custody mean retained host
source/control only, not native adoption.

Three temporary compile-negative functions attempted to capture the borrowed
buffer slice, capture the borrowed program, and return an owned vector from the
callback. The real crate rejects these with two `E0521` diagnostics and one
`E0308`. `negative-source.rs` preserves the exact injected test module; the
recorded exit `101` is intentional. Those functions are removed from the final
source, whose restored contents are checked by the auditor after formatting.

The earlier `preliminary/focused-01` result has eight passes before the two
additional currentness tests were added. It is not the final qualification.
`verify-source.sh` freezes the final source and runs strict Clippy, formatting,
GNU/musl library builds, exact test rosters, focused and full runtime suites, doctests,
no-default compilation and the unsafe-source policy. Each command records its
arguments, raw output, UTC endpoints and actual exit status. Binary receipts
identify the exact library-test executables used; no executable is archived.

The unchanged KFD library-test executables and their rosters are compared
byte-for-byte with the preceding qualified auxiliary-release campaign on both
targets. Its 1,389 passing tests per target are reused baseline evidence, not
new passes in this packet. `duplicate-lower-run/` preserves an unnecessarily
started full GNU KFD rerun; its exact owned test process was inspected and
stopped with SIGTERM after confirming the identical executable. Its recorded
exit is `143`, no final test summary exists, and it is not counted as completed.
`interruption/` records the exact process/parent, thread-only descendants,
signal and process-absence check. The original runner contents are retained as
`verify-duplicate-lower.sh`; the corrected `verify-source.sh` performs fresh
full regressions for the changed runtime and exact lower-baseline comparisons.

The first unsafe-source policy invocation flagged the newly archived `.rs`
copy of `authorized_execution.rs` as an unreviewed source file. Its failed result
is preserved under `policy-before-snapshot-extension/`. The evidence snapshots
were renamed to `.rs.txt`, without changing their contents, runtime source or
the unsafe inventory baseline. The policy and closing source-hash gate were
then rerun separately to finish the same source campaign.

`audit-results.py` checks source/patch identities, all command vectors and
chronology, Cargo artifact identities, GNU/musl test rosters and outcomes,
ignored native membership, lower-baseline identity, owned-process interruption,
and the expected compiler rejections/restoration.
It requires the original checkout and retained test executables at their
recorded paths, plus Rust formatting tools; it performs no GPU or SSH work.

The initial auditor incorrectly required each compiler diagnostic to contain
only one location, rejecting rustc's additional help/standard-library locations.
`audit-initial.py.txt`, `audit-initial.stdout` and the recorded reproduction in
`audit-attempts/` preserve that audit failure. The corrected auditor binds each
primary diagnostic location and exact source expression while allowing the
secondary explanatory locations. No Rust source or compiler result changed.

`audit.log` records the successful final audit. `SHA256SUMS` covers every other
file in this closed archive, including both excluded attempts and all snapshots;
archive integrity is distinct from behavioral or formal verification scope.

## Remaining Integration

The next production packet must add backend Entering/Adopted/Retiring ownership,
pre-effect stream/lane leasing, original-buffer materialization through initial
primary/AUX/rebound binding, and exact unpublished abort/data disposal. Context
must preserve records and credits until cleanup succeeds, support Stop before
the first adoption step, and only then release the hold. Production async hooks
remain uninstalled until those operations are connected.

Native qualification needs a separately pinned execution fixture, not the
nonexecuting `TestAuthorityV1`. Later Worker-V3 host handoff, ISSUE/COMPLETE,
generated typed output, graph/drain, formal and matched HIP/HSA performance work
remain distinct. Loader/ABI parsing still allocates host metadata, including a
bounded binding vector shared with projection; no allocation-free, zero-copy
GPU transfer or performance improvement is claimed. No remote files or jobs
are created by this packet.
