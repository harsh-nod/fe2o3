# Root Guard and Shared Launch Transport

Date: 2026-09-26. Continuation of the
[supervisor startup checkpoint](conditional-native-supervisor-startup-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed.** Consuming native supervisor launch and protected startup validation
remain outstanding.

Base: `ec6e1cd64a62e009be41caf79210b92bdc22cde9`.
Integrated code: `859a982976f1dbff904de976800d761e7e194dd1`.
Subsequent documentation changes do not change executable behavior.

## Implementation

- `0ebdead7f`: integrated worker implementation of metered cleanup guard cloning.
  Both original accounts pay before one CLOEXEC duplication. The persistent
  guard survives refusal, unwind, draining, quarantine and controller recovery.
  The new File is a controlled alias, not independently admitted authority.
- `859a98297`: native anchor preparation joins the actual lifecycle lease to its
  state root. An explicit operation installs that root-bound alias before the
  first child; launch validates the pool's installed guard against the actual
  retained lease before staging or spawning. The same implementation serves V2
  and V3 with complete borrowed-context floors and nested quota accounting.
- The existing bounded anchor readiness scheduler moved to the shared spawn
  crate. It returns inert payloads: 16 bytes plus one right for the anchor, or
  88 bytes without rights for supervisor transport. Nominal decoding remains in
  the owning coordinator. This supports, but does not implement, consuming
  supervisor launch. There is no new authority provider, compiler or reaper.

Every receive adopts disclosed rights into RAII before fallible observation or
shape validation. Unknown control, extra rights, SCM_PIDFD, short/long payloads
and truncation refuse with cleanup. Liveness has a separate original-ledger
charge. The finite attempt limits and deadline checks also cover successful I/O.
Quotas bound logical work/storage and syscall attempts, not syscall latency,
generated stack, allocator use or RSS.

### Terminal Framing

A zero-byte seqpacket is not necessarily EOF, even after peer shutdown: it can
precede queued trailing data. Terminal receive now enables `SO_PASSCRED` and
rejects marked empty records. Only an unmarked zero receive after successful
socket setup is accepted as EOF. Credentials serve as a record marker, never as
service identity. Setup failure refuses without a hangup-only fallback.

This framing argument is supported by source inspection, not successful local
execution of that socket option. Linux v6.8's seqpacket receive routes actual
queued records through ancillary delivery, whereas no-record EOF returns before
it. Its ancillary code emits credentials when the receiver enables PASSCRED.
See the primary [UNIX socket implementation](https://raw.githubusercontent.com/torvalds/linux/v6.8/net/unix/af_unix.c)
and [ancillary implementation](https://raw.githubusercontent.com/torvalds/linux/v6.8/include/net/scm.h).
This is not evidence that the intended deployment kernel has been validated.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time and source frozen during builds. Unit/doc/check commands used
`--frozen`; the existing static script uses `--locked` with offline Cargo.
The guard worker edited source only; the primary reviewed, integrated and ran
the tests. A separate read-only worker reviewed the outstanding ordered-custody
design; that proposal is not implemented or credited as validation. Its later
transport review found no concrete defect, but performed no builds or tests.

| Check | Result |
| --- | --- |
| Four-crate GNU unit suite, R6, four threads, no filters | 423 passed, 52 failed, 42 existing ignored; exit 101 |
| Four-crate doctests, R1 | 27 positive and 256 compile-fail passed; exit 0 |
| Fifteen-package all-target check, R1 | Passed with existing warnings; exit 0 |
| Musl release transport suite, R3 | 22 passed, 2 failed, 70 filtered; exit 101 |
| Static V1, V2 and V3 gates | All passed, including production image admission and missing-descriptor smoke |
| Changed Rust files, pinned rustfmt check | Passed |

GNU totals are compiler coordinator 52/7/0, supervisor 216/42/42, external anchor
coordinator 63/1/0 and shared spawn 92/2/0 (pass/fail/ignored). Nested subprocess
output is not counted twice. Fifty previously failing tests still encounter
environment restrictions: 49 socket-operation EPERM and one ACL fixture EINVAL.
Two additional live terminal-credential tests fail at SO_PASSCRED with EPERM in
both GNU and musl. The suite is not green; no test was disabled or predicate
relaxed. All-targets is not GPU architecture coverage.

The real GNU/musl readiness tests exercise recvmsg with zero through five rights,
including disclosed excess descriptors and kernel-truncated rights. They verify
exact CLOEXEC transfer and closure on refusal. Parser and scripted schedules
cover framing, malformed ancillary data, finite retries, deadline rejection,
original-account work/storage boundaries and unwind. These do not substitute
for the two blocked live terminal-credential tests or protected startup.
SCM_PIDFD disposal is covered synthetically, not by a live kernel receive.

The existing isolated guard subprocess now includes seven additional schedules
for clone funding, exact/short quotas, arithmetic overflow, custody/recovery,
EINTR and unwind. Both native anchor families additionally exercise actual
root/lease substitution, guard installation/validation, unrelated-guard refusal
and real flock retention through prepared-owner unwind. Completion markers
prevent an accidentally empty subprocess filter from reporting success.

Earlier runs exposed diagnostic ordering and a test-only inode-reuse race.
Root shape validation again precedes root/lease joining; temporary endpoint
names now pin fixture inodes through closure assertions. No retries, sleeps or
weakened assertions conceal these failures. Earlier hangup-only EOF results are
superseded by the credential-based tests above. GNU R5 accidentally selected a
cold release profile and was interrupted; the completed R6 uses the established
debug test profile. No credit is taken for R5.

Logs remain in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
a5a1a1c55e06bc0cf53feed6b5488367c3b67c6e9a25fc2c444b8049e4d05c10  conditional-native-root-launch-full-r6.log
3620eda73f14575dfa30331015098c8fcd6008d007d75c4a73bbd2f484427c5a  conditional-native-root-launch-docs-r1.log
dba398ddb56386852b4c5bc1231c90324ce635a3c50edc4c4fc1699aa6c5b55f  conditional-native-root-launch-all-targets-r1.log
53ff7e1f7761903addd4805ee3d0221ec37e21a0f6c4d265e8bf0dab86991485  conditional-native-root-launch-musl-r3.log
8056b2f1478e925d3b15982774c0c533b26d196375e1dde0a582c7a40240f7ef  conditional-native-root-launch-static-v1-r1.log
565228029311bbaa160c6df7e5f014cbc8f10fa95830c4174e83ef575a3fe7ec  conditional-native-root-launch-static-v2-r1.log
12609dad7d409494e3ab28e387afd415f51e511566f2add630d4d2bfb76c5bcd  conditional-native-root-launch-static-v3-r1.log
```

Static executable SHA-256 values, V1, V2 and V3 respectively:

```text
55ddbb407d40016b1e04646335350c528275c2107bde45a628448f63199b8b74
244a172b9c68005e068dfc08b08ba3b042f4ec76385a439533ec2858a7b3febd
bb99c195441d1088285f5213f643c8ce2407e3806a5d996751652dc88181a3ff
```

## Remaining Gates

1. Integrate ordered custody into the existing cleanup pool before clone. Keep
   the supervisor's full prepared owner, live anchor and both leases until an
   exact consuming terminal wait. Pending/quarantined cancellation is not
   termination. Fund retained payloads persistently; retire outside pool locks
   so releasing preparation can safely defer anchor cleanup in the same pool.
2. Compose consuming native supervisor launch with final staged-file validation,
   profile/gate checks, nominal 88-byte readiness, EOF/liveness and continuity.
   Cover every post-clone refusal and unwind, not just successful construction.
3. Validate actual protected startup, EOF framing, recovery and runtime custody
   on the intended host. Current rootless and missing-descriptor probes do not
   authenticate a protected deployment or trusted parent provenance.
4. Complete the compiler admission/postchecks/finish/revalidation, V5 publication,
   SubjectV3 transport, backend receipt admission and conditional finalization.
   Then validate semantic/machine/numerical proof, generic non-AMD behavior and
   all 47 tutorial kernels on gfx942/gfx950 through the production path.

SSH attempts to mi350, mi350-2 and mi300x failed DNS in this session. No remote
job or scratch was created. Local implementation and evidence are distinct from
publication; both main refs and the issue update require separate confirmation.
