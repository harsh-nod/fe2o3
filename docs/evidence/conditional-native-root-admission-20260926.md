# Native Root Descriptor Admission

Date: 2026-09-26. Continuation of the
[native launch checkpoint](conditional-native-compiler-launch-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed.** The installed root entrypoint remains V1.

Base: `23a55e686df93e32bafdbca0318d4e84ff4a395d`.
Source reader: `5d49acc010808f3f0d18e1827a5ed73b94a096f0`.
Integrated code: `5c846de5b1f71be90ff325c58985694f62bfdaeb`.

## Implementation

`InheritedCompilerExecutionDeploymentV2/V3` now admit genuine native root inputs
and compose anchor-first launch into the existing compiler preparation and
retained supervisor launch. One closed macro supplies the two nominal families;
there is no V1 authority upgrade, provider selector, second reaper or alternate
compiler pipeline. The V1 source bundle still means three untrusted Files only.

Intake is explicitly unsafe: the activation caller must transfer unique ownership
of the fixed fourteen descriptors, with no other owner or signal/foreign code able
to close or reuse a slot. Exact root identity is required. A fixed 1024-byte RawDir
buffer and at most four iteration attempts require only the main PID in the
thread directory. No libc directory stream or retry loop is used. All F_GETFD,
file-type and byte-length checks precede adoption. The three roots must be
directories; five images are positive bounded regular files; four records and
two seeds have exact sizes. Only after the complete preflight does RAII take the
entire table and make every descriptor CLOEXEC.

Three independent native lifecycle leases are opened before either seed is read.
The supervisor and anchor state roots are joined to the same canonical lifecycle
parent. Root-owned public records require exact mode, owner/group, access, link
count and size with no capability or ACL. Two single-attempt reads, explicit EOF
probes, constant-time byte comparison and before/after metadata checks precede
native decoding. Snapshot checks include mtime/ctime nanoseconds but exclude
atime, which the reads may change. These are observations, not a proof that the
source never changed between observations.

Policy is decoded first; supervisor, anchor deployment and provisioning decode
against actual same-family contexts before sealing fresh native capabilities.
All five declared image lengths, including the policy's issuer, are bounded
before seed reads. Both seed buffers and the EOF byte use zeroizing guards.
The original native key-constructor error is preserved. These guards do not prove
erasure of kernel pages or every compiler-generated physical copy.

V1 and native startup share extracted runtime-root/listener filesystem mechanics.
Paths are bounded before owned copies. The fixed listener is bound, non-listening,
nonblocking and CLOEXEC with exact owner/group/mode and pathname checks. A missing
creation identity no longer authorizes unlinking any socket: cleanup checks an
established matching inode before unlinking. If creation identity could not be
observed, cleanup refuses to unlink; trusted stale-path recovery remains a
deployment concern. These checks assume the trusted root/runtime-directory TCB;
they are not protection against an adversarial root racing pathname operations.

Complete admission returns its full unreserved owner charge. Consuming launch
validates trust, root leases and program sources, installs the persistent guard
before the anchor, and passes the resulting real anchor into native compiler
preparation and retained supervisor launch. All operations retain the original
request and independently funded cleanup accounts. Native APIs can refuse when
those finite accounts are exhausted; funding does not promise eventual reaping.

`SOURCE_STORAGE` is a conservative raw-source reservation, not a complete
successful admission/launch work or scratch quota. Nested operations meter their
own work and peak storage. Full composed startup quota queries and genuine
root-path boundary/failure tests are still required before installing native
activation. No successful native root boot is claimed.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time, compiled inputs frozen during each run. One native worker
implemented source validation in a private worktree and reviewed integration;
the primary integrated, added shared listener/root composition and ran all gates.
A second worker slot was unavailable; no Qwen or external-model worker was used.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, R4, no filters | 490 reported passes, 53 failures, 42 existing ignored; exit 101 |
| Four-crate doctests | 29 positive and 282 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |
| Full coordinator musl release suite | 92 reported passes, 8 failures, 0 ignored; exit 101 |
| Static V1 coordinator gate | Static ELF inspection and two fail-closed activation smoke cases passed; exit 0 |
| Changed Rust formatting and diff whitespace | Passed |

GNU totals are coordinator 92/8/0, supervisor 216/42/42, anchor coordinator
63/1/0 and shared spawn 119/2/0 (reported pass/fail/ignored). Nested subprocess
output is not counted twice. Failures are 51 socket EPERM and two ACL setup
EINVAL, including the new source-reader ACL fixture. That new fixture remains
failed, not skipped or relaxed. Its ACL-rejection body did not execute. Eighteen
of nineteen new source tests pass; five root-account/intake-predicate tests and
two shared-listener predicate/path-bound tests pass.

The earlier root-dependent cleanup-guard test explicitly skips under uid 1000,
but libtest reports it as passed; its real-root body receives no validation credit.
No root-only complete inherited admission/launch test executed. Neither passing
component checks, compile-fail ownership checks nor all-targets compilation proves
protected startup, generic GPU architecture coverage or tutorial completion.

Initial compilation caught Rustix API spellings and missing public-field docs;
both were corrected. Worker review found the issuer's declared length cap was
checked too late; all five declared limits now precede both seed reads. Raw-source
length preflight independently bounds actual files before ownership transfer.
The worker's final read-only review found no issue in the fixed-buffer thread
enumeration; the worker ran no builds or tests.

Musl retains the same seven coordinator socket EPERM and one new ACL setup
EINVAL. The static binary is the existing V1 root entrypoint, not a native root
boot test. The script checks ELF64 ET_EXEC, absence of dynamic-loader sections
and undefined symbols, a non-executable stack, and exact refusal without
activation metadata or with a forbidden argument. It does not run protected
image admission. Its SHA-256 is
`12667aed93161d35b54e00a4ab30ebd5e18aec3f420784036d584763594babaa`.

Logs are retained at
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
27703f514a0b6177c12f3f18e28e86892e639ec1cfdaa7dd7880deb13cf6bd86  conditional-native-root-admission-all-four-r4.log
87dea1c9159ffb8ee0705f96986d278fd2ce09e5b4ad323adb6e8a55774fc6cc  conditional-native-root-admission-docs-r1.log
cc4b2932e4f20d2f57adc496b2907dd0c9dc23cdab57a7a5e816bff6ce9e467e  conditional-native-root-admission-all-targets-r1.log
2da7327ac0c794671829b0ce494052bf00270e844b8f2842df0aba5ef749fb19  conditional-native-root-admission-musl-r1.log
dd5caab90d53b62142aef954ef6b6b93b7b91ecb6665bf8467ff03a961f38c61  conditional-native-root-admission-static-r1.log
```

## Remaining Gates

1. Derive complete startup work/peak and cleanup funding queries, including the
   overlapping inherited, anchor, compiler preparation and managed owners. Add
   genuine root-owned exact/short, descriptor-close, seed-failure, staged-object,
   malformed Ready/EOF, unwind and deferred-cleanup tests through this composition.
2. Migrate installed root activation to the native path: bounded arguments and
   activation environment, environment clearing, blocked termination signals,
   actual-main-PID readiness, finite cumulative monitoring, cancellation and
   original-account cleanup draining. Existing `main.rs` still calls V1.
3. Validate static native root binaries, trusted provisioning and full protected
   startup/recovery on the intended host. Static V1 refusal smoke is not this gate.
4. Finish downstream compiler admission, postchecks/finish/revalidation, V5
   publication, SubjectV3 transport, backend receipts and conditional finalization;
   validate semantic/machine/numerical proof, generic non-AMD behavior and all 47
   tutorial kernels on gfx942/gfx950 through that single production path.

The current MI350 SSH attempt failed DNS; no remote job or scratch was created.
Publication to both main branches and the issue requires separate confirmation.
