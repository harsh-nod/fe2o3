# Native Root Launch Checkpoint

Date: 2026-09-26. Continuation of the
[spawn checkpoint](conditional-native-root-spawn-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.
Native coordinator launch is implemented; successful protected startup and the
compiler coordinator's use of this path are not established.

Base: `4abdabc8a87f912ef484954ef3454154aeb39015`.
Integrated code: `e414e0ef9182db43c5e15ad749a172bc548e97dd`.
Subsequent documentation changes do not change executable behavior.

## Implementation

- `51ce0eb3b`: native worker's shared readiness scheduler and scripted tests.
  The worker wrote only two new source files in its private worktree, ran no
  Cargo, SSH or network commands, and then reviewed the primary integration.
- `e414e0ef9`: primary integration into both V1 and native coordinator paths,
  native V2/V3 consuming launch, final staged-file validation, original-account
  resource accounting and managed child/endpoint/preparation custody. It also
  corrects ancillary ABI sizing and adds real descriptor-transfer tests.

Native launch derives full transfer charges from actual native owners, not a
caller-supplied byte count. The nine fixed helper inputs are checked against the
inherited ABI at compile time. Final helper/daemon, lifecycle, context, deployment,
provisioning, key, root and bootstrap Files are checked before native spawn.
Original native owners remain retained. Full image charges overlap while source,
temporary transfer and staged aliases coexist.
The reviewed custody contract permits redundant transfer aliases to close after
final validation while admitted owners and staged duplicates retain the objects
and lifecycle lock. Charges retire only after closure; no explicit unlock occurs.

The guarded child owns the atomic pidfd, cleanup slot and artifact lease before
fallible parent work. Independently observed profile/namespaces and repeated
input validation gate release. Canonical readiness, exactly one endpoint, exec
EOF, same-child liveness and native endpoint admission precede exec confirmation
and a managed result. Late stage/admission completion cannot ignore the deadline.

`RootManagedExternalAnchorV2/V3` retain the complete preparation plus child and
endpoint custody. Launch returns additional growth above the consumed prepared
owner. Continuity borrows actual same-family policy and supervisor capabilities.
Cancellation/Drop use prepaid finite cleanup; pending and quarantined records
remain owned by the shared pool. There is no new ledger, blocking consuming wait
or V1 child-owner upgrade. Deferred artifact leases can indefinitely delay lock
release; finite funding does not guarantee eventual reaping.

The shared readiness scheduler replaces the old V1 loops as well. Polling phases
allow at most 120001 attempts each; gate writes allow 64. The deadline is shared
and capped at 120 seconds. Both successful and failed I/O meet deadline checks.
Its maximum mechanical work is 2272425504 logical units, plus at most 360000
separately charged native liveness checks. The full launch quota also includes
staging, source revalidation, spawn, profile and endpoint admission. These quotas
are not instruction counts, elapsed-time, generated-stack or RSS guarantees.

Readiness receive uses one ABI-sized fixed ancillary buffer, adopts all disclosed
SCM_RIGHTS/SCM_PIDFD descriptors before fallible checks, and closes rejected extra
rights. Truncation and unknown control are not accepted as a one-byte child stage.
The fixed raw receiver is needed to dispose of SCM_PIDFD, which the pinned Rust
ancillary enum does not expose; its scope is Linux x86-64, like this crate.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo command at a time, no source edits while commands ran. Unit tests used four
threads and a short private TMPDIR. Evidence directory:
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| R1 | Compile failed at fixed ancillary-layout assertions; no tests ran |
| R2 | Coordinator 60 passed, 1 failed; spawn 69 passed |
| R3 with real recvmsg test | Coordinator 61 passed, 1 failed; spawn 69 passed |
| Four-crate R4 | 370 passed, 37 failed, 42 existing ignored; no filtering; exit 101 |
| R5 after reviewed test-accounting correction | Coordinator 61 passed, 1 failed; spawn 69 passed; exit 101 |
| Four-crate doctests | 10 positive and 177 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |
| Changed Rust formatting and whitespace | Passed |

R4 consists of supervisor 189/36/42 (pass/fail/ignored), coordinator 61/1/0,
profile 51/0/0 and spawn 69/0/0. Runs overlap; subprocess helper entries and
their nested reports are not additional independent tests. No failed test was
ignored and no production admission check was relaxed.

R1 exposed a wrong buffer-size assumption: `rustix::cmsg_space!` includes slack
for alignment of a byte buffer. The direct libc receiver instead needs the exact
ABI `CMSG_SPACE` for its already-aligned control struct. The fix uses the pinned
libc calculation and retains compile-time layout assertions. The real SEQPACKET
test sends one, two and three rights; one is accepted CLOEXEC and all rejected
extra/truncated rights close without changing the sender's retained descriptor.

Scripted readiness tests cover each finite limit, last-attempt success, EINTR and
permanent errors, deadline crossing even on success, malformed bytes/control,
descriptor disposal, liveness/observer refusals, exact/short work, original denial
history and unwind. Actual native staging tests cover both families, final-file
identity and CLOEXEC, independent mutation of every final descriptor, complete
source/image overlap, context substitution, short floor/work/scratch and unwind.
Bounded subprocesses use the real global cleanup account to test public launch
entry-work, timeout and nonroot refusal without consuming cleanup capacity.

The review found a test retaining a returned staged owner before reserving its
charge. R5 corrects that order and retires the charge only after Drop. No concrete
production defect was found in that source review. It is not a proof of all paths.

The remaining failures are socket domain/bind/send/peer-credential EPERM and ACL
fixture EINVAL. The coordinator test reaches peer-credential observation and
fails there; its prior ready receive completed. The aggregate suite is not green
and still needs a clean-environment rerun. Historical static ELF evidence was
not rebuilt here and does not establish this revision's protected execution.

## Evidence Digests

SHA-256 of selected logs in the evidence directory:

```text
3f788dc4806bdfcd5ba6b9b467024f61445568b347f311b4c1cb5174c052c25f  conditional-native-root-launch-full-r4.log
646bdb8d1731cf46f2814c2d4e413529cf511e18b7bd0053e0428af2331657b4  conditional-native-root-launch-full-r5.log
2bd474536fd6da8eb01685bcfe01a5a1cbe88c640c41a84eb982f0e669a36afb  conditional-native-root-launch-docs-r1.log
3d42397c8dacc345695b730b88c3bf17d13a82eae946f83dffc331d8bff1ae4d  conditional-native-root-launch-all-targets.log
```

## Remaining Gates

1. Add direct adapter-level post-clone failure/unwind coverage, especially child
   storage refusal and failures after endpoint receipt. Lower-level child custody
   and scripted readiness tests do not substitute for these composed cases.
2. Implement native supervisor transfer and integrate native anchor/issuer
   preparation, launch, readiness and managed cleanup into the compiler
   coordinator. The existing compiler coordinator still consumes the V1 path.
3. Complete compiler admission, postchecks, finish/revalidation, V5 publication
   and SubjectV3 transport through the single production path. Backend receipt
   admission remains V1 and conditional finalization still refuses.
4. Validate real protected startup/recovery and cleanup, source/machine/numerical
   proof, generic non-AMD behavior and all 47 tutorial kernels end to end.
   Passing API, staging or transport tests does not close these milestones.

Fresh SSH attempts to mi350, mi350-2 and mi300x all failed DNS in this session.
No remote process or scratch was created. Source worktrees and reports were
preserved. Publishing commits and issue updates is separate from validation.
