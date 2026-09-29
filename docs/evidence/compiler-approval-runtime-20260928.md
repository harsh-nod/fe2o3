# Compiler Approval and Protected Source-Proof Replay

Date: 2026-09-28. This continues the
[source-bound policy roster checkpoint](native-policy-roster-20260928.md).
It does not complete an issue #272 M0-M7 milestone, activate the conditional
production compiler, or establish 47/47 production-to-safe-GPU qualification.

Later work is recorded in the
[runtime inventory and proof-controller checkpoint](retained-runtime-controller-20260928.md).
Its r12 failure, controller fixes and local validation are separate from the
older successful r11 protected replay below; that replay does not qualify them.

## Implemented Boundary

`CompilerApprovalPolicyV1` is an inert, fixed 352-byte record. It binds all six
existing `CompilerClosureV2` pins and transition version, the exact V3 client
profile identity, an expected runtime-manifest identity, and a closed required
runtime-enforcement version. Strict framing, reserved bytes, nonzero identities,
closure aggregate and domain-separated digest are checked. Version 1 requests
complete executable-runtime enforcement; decoding it does not implement or
observe that enforcement. Public construction and coherent resealing grant no
authority.

`ApprovedCompilerPolicyV1` has one public positive constructor: it opens the
fixed `/etc/fe2o3/build-authority/policy-v1` and
`/etc/fe2o3/compiler-execution/client-profile-v3` paths internally. It requires
root ownership, launcher-compatible directory permissions, descriptor-relative
non-symlink traversal, exact read-only file modes and lengths, single links,
absent ACL/capability attributes, and immutable policy backing. It reads stable
snapshots, matches the profile identity, retains both original files and a sealed
profile, and rechecks current paths. Revalidation rejects original-object drift,
identical-byte inode replacement, profile rotation and loss of immutability.
This is bounded origin observation, not exclusion of privileged policy writers.

The move-only owner retains its original budget address and work ledger. Its
full unreserved storage quote includes both origin-file images, not just the
sealed profile and Rust fields. Exact and one-short tests include an independent
size assertion that detects omission of those 632 backing bytes. No public
alternate-root, byte, file or inherited-descriptor constructor creates approval.

Cargo's native continuation now carries approval through preparation, readiness,
finalization and persistence. Its implemented checks compare the complete
approved closure with the independently matched parent invocation and require
the exact approved profile. Raw transport fixtures remain separate and have no
finalization transition.

**Activation remains explicitly refused.** Approved preparation and the lowest
Cargo finalization entry both call the same unconditional
`RuntimeEnforcementUnavailable` gate before channel/command mutation or
publication acquisition. There is no environment, feature or test bypass.
Approval, a successful child exit, or a protected source-proof result cannot
replace the missing compiler-runtime guard. The continuation beyond this gate
is compiled, not a demonstrated positive production path.

## Proof-Controller Hardening

The existing retained proof controller now refuses writable executable mappings,
WX mmap, and every EXEC mprotect/pkey_mprotect request, including restoration of
EXEC on privately dirtied pages backed by an unchanged approved file. Initial RX
mapping of admitted immutable ELF ranges remains supported. Ordinary data maps
and non-executable protection changes, including RELRO, remain supported.

The controller mediates open/openat flags and refuses write-capable opens. It
also refuses creat/openat2, userfaultfd and personality changes, and rejects an
inherited READ_IMPLIES_EXEC personality. Existing ptrace, process-memory and
io_uring denials remain. The selected proof wrappers use pipes rather than
output/temp file creation; the actual pinned runtime passes the protected tests
below with these rules.

This is not complete compiler enforcement. Shared-FD substitution, concurrent
mapping/clone-argument races, external-writer exclusion and composition with an
outer compiler controller still require implementation. The compiler also needs
controlled ordinary output writes and the real approved `fe2o3_macros` DSO;
copying the proof controller's write-open rule or denying every proc macro would
not produce a usable compiler.

## Local Validation

Pinned nightly, locked offline resolution, one Cargo job, serial tests, empty
GPU visibility, no incremental/debug-info output, a 12 GiB virtual-memory limit
and 1,200-second outer deadlines were used. Guard records preserve the source
and tool snapshots before/after each command. Logs are in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory.

| Check | Result | Log |
| --- | --- | --- |
| Approval codec | 9 passed | `approval-codec-tests-rone.log` |
| Approval owner/origin/accounting | 22 passed | `approval-owner-tests-rone.log` |
| Proof controller and memory policy | 27 passed; 1 local pinned-runtime test ignored | `approval-memory-controller-tests-rone.log` |
| Cargo native transport, accounting and activation refusal | 22 passed | `approval-cargo-tests-rtwo.log` |
| Approval-owner compile-fail doctests | 2 passed | `approval-owner-doctests-final.log` |
| Compiler policy capture and native packing regression | 14 passed | `approval-backend-regression-final.log` |
| Verifier policy reconstruction regression | 14 passed | `approval-verifier-regression-final.log` |

These are 110 passing local Rust tests. The owner fixtures use a private
synthetic tree and immutable-check probe, not an installed production policy.
Actual ACL cases ran; this user could not set security.capability, so the two
capability-xattr cases explicitly report no coverage. Cargo approval-custody
success and late-rotation behavior still need positive protected integration
tests; the current Cargo fixtures exercise inert transport and refusal.

The genuine offline invocation preparer separately passed one exact, otherwise
ignored test, capturing both gfx942 and gfx950 without proof or GPU execution.
Backend and verifier no-run builds provided uniquely selected JSON artifacts.
Formatting, whitespace, DCO, source-hygiene delta, and all eight hygiene-policy
unit tests passed for the code checkpoint.

The five-package all-target integration check passed on the Rust source at
`b26f313551d78a9ee957717f2991e6cc2724cad1` after incorporating the public
Option/enum, scalar-inventory and origin-worklist changes. Documentation edits
followed. Warnings remain, including the intentionally unselected native
continuation. The final check log is `approval-integration-check-publish.log`,
SHA-256 `808780be1fdb0e17853066695a9c46319144096285319da413c93cfd490b3ec9`.
Its before/after source and tool snapshots match. Earlier integration checks
also passed; none constitutes a protected execution of the later merged source.

## Protected Replay

MI350-2 ran the unchanged, hash-pinned r11 controller on an independently copied
root-owned snapshot. This tested commit
`ece171683532888445392107e7bfcae0ece1722a`, which remains in the merged history;
it did not test the subsequent Option/enum integration commits. The 9,125-file
source snapshot identity is
`674eb8057dc19c1a32f5cb702918e339526878d41490b50d39878d7eb20a9703`.
All three successful build/preparation guards name that same identity.

The runtime audit and all three protected public-lease preflights passed:
installed-closure audit, genuine Verus proof, and false-proof rejection.
The actual-source parent passed for both targets in 174.31 seconds. The
conditional F-prefix parent passed in 444.87 seconds, with all eight
target/mode combinations independently checked by the frozen controller.

For each target, the F child retained actual source policies, replayed their
agreement, installed the conditional bridge and stopped at
`FE2O3-COND-FINALIZER-001`. The fixed-six child remained separate. Zero-work
and short-storage cases refused, preserved the original account and restored
the incoming storage floor. Every native-output, launch-authority and
qualification flag remained false. This exercises actual source-bound policy
capture, not native V2 artifact packing, Worker publication, machine refinement
or GPU execution.

The supervisor exited zero after 698.99 seconds, reached drained state, and
reported `tests_passed: true`. Full output was collected and each archive member
validated before cleanup. Evidence directory:
`r11-invocation-20260928.GPSPbyEu` in the sibling evidence root.

| Evidence | SHA-256 |
| --- | --- |
| Frozen r11 controller | `b0943f174adddb1c210d75c3bacb42229f07354ba2527ab62f61a236fa84688e` |
| Bound request | `1fa616737d8d9543d650aa95640c1f0eb3855a81e0752c608151d1c822eaeff0` |
| Terminal inside report | `2cdbbe6d46e1eca9becf909e5b573eea969d9d5e0572119a5bf6ab3b79c71301` |
| F-prefix matrix report | `94518c0739b1bef0a885b7dfb998cb29b07f3de24c49418cbe43629ffa67d58e` |
| Collected terminal archive | `e42c28ba03305f70f7511c84dcb9c3bf6f9f001aff442e00ba87af119e68d1cf` |

## Cleanup

The owned remote run, upload, service, slice, cgroup and private mounts were
removed after terminal evidence collection. The local staged duplicate was also
removed. Both temporary remote provisioning roots, including the six-file
nightly support installation created for this attempt, were removed after
checking their exact inventories, hashes, metadata, historical inode records
and absence of users. The self-written staging report had no historical inode
record; its original digest and metadata were checked, and its current inode
was bound across the cleanup checks. Existing toolchain parents, the shared
proof-runtime volume/image and system security settings were preserved.

Local preparation metadata and all execution reports remain in the evidence
root. The compiled preparation tree and two obsolete local test executables
were removed. The active build cache is retained; its inactive musl target
cache was relocated to a separately recorded RAM-backed directory, not deleted.
These operational cleanup observations do not establish runtime enforcement.

## Remaining Production Work

The full compiler runtime needs an independently approved, bounded inventory
covering loader libraries, actual proc-macro code, permitted descendants and
the proof executor, in addition to the unchanged six compiler pins. It needs
pre-exec-through-completion enforcement, race-stable mapping/FD checks,
external-writer isolation, ordinary compiler output support, V3 release/broker
delivery and exact invocation/receipt joins. Restart still requires fresh
Worker/external-anchor currentness admission.

A reviewed next direction is a protected sibling proof executor using the
existing retained verifier execution and receipt path. That avoids nesting two
ptrace controllers while preserving process-affine leases. Its closed endpoint
bootstrap, request/source/runtime association, hostile lifecycle tests and
actual guarded macro compilation are not implemented by this checkpoint.
Neither an arbitrary RPC response nor an inert manifest may become a positive
execution owner. Both Cargo activation refusals remain until the integrated
runtime owner and its tests exist.
