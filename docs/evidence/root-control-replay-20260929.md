# Root Packet and Replay Checkpoint

Status: **component validation only; production root-session integration pending**.
This follows the [running-image checkpoint](root-issuer-image-20260929.md).
No protected proof, safe GPU launch, tutorial qualification, or milestone closure
is claimed.

## Implementation

Code checkpoint: `224c8dafea7e935ea27d2c92b045eaf86ca2787c`.

- The existing `RootLaunchChannelV3` now has private packet send/receive methods.
  They preserve its original account, process/thread and endpoint custody, require
  its parent issuer alias to be closed, and prepay one nonblocking attempt using
  shared transport mechanics. No public root-end accessor or new production
  unsafe code was added.
- A private `RootControlReplayWindowV3` retains one inert request/reply pair.
  Pending duplicates cannot become new operations; completed duplicates select
  the exact cached reply. Direction, association, content, ordering and overflow
  refusals preserve state. Full maximum storage is charged independently of state,
  including across failed sends and caller unwind.
- The window is not a retirement tombstone or issuer admission. Actual occurrence
  custody and the exact retirement record must survive connection replacement
  independently. Neither component is wired into production root RPC yet.

## Verification

Locked/offline guarded runs used nightly `2026-04-03`, one Cargo job, serial
tests, bounded runtime/memory, and disabled GPU visibility. Source and tool hashes
were stable before/after each run. All three final runs used 9309 git-visible
files with source hash:

```text
a58d1013ba2069c4e0d2c7694940b3fc049594237fcc1be79cca792ef94545d8
```

| Run | Result |
| --- | --- |
| Broker, coordinator and issuer all-target check | Pass |
| Broker `compiler_execution_root_` unit filter | 14 passed, 0 failed, 13 ignored |
| Broker/coordinator/issuer doctests | 175 passed: 85 + 78 + 12 |
| Scoped Rust formatting and whitespace checks | Pass |
| Hygiene delta `4b15165a7..224c8dafe` and eight hygiene self-tests | Pass |

The focused unit run includes all nine new inert replay tests and the new packet
quote test. Its older `unprivileged_role` helper is inactive in the default run;
that entry is not positive root coverage. The thirteen ignored entries comprise
eleven privileged cases and two subprocess helpers. Four privileged packet cases
are new and were **not executed**. Their opt-in lane requires exact root and
real socket/process prerequisites; it fails rather than silently skipping when
explicitly selected without those prerequisites.

New tests cover exact/short work and scratch, unprepaid input floors, denial
history, original-account replacement, moved Budget rejection, identity-field
faults, sequence exhaustion, exact request/reply matching, and inert cache
retention after simulated send failure/unwind. Identity-field injection is not
post-fork or cross-thread qualification. The unexecuted socket fixtures use real
constructor-created owners and cover backpressure, credentials, framing, rights
disposal, EOF, alias/shape guards, resource limits and credential loss.

The whole broker suite was not rerun for this narrowly scoped checkpoint. Its
preceding 266-pass/97-fail/14-ignored result remains documented in the running-image
checkpoint, including reproduction of all failures on the pre-change binary.
These focused passes do not make that full suite green.

Log SHA-256 values:

```text
root-packet-integrated-check-r2
f2f60a5053d172aa5b3c1e40c2cfc1af8d33b815c4dc97e00fcbdebc673af1ef
root-packet-tests-r2
66445aa8919c3d81cfebf7f803f2a5986bacb1d9224b83c80bf2d001016afaf3
root-packet-docs-r2
cc9ea5eb02b46e24f4c462b2a5f0c985cf64707d1771b68ce08d38999be01f5c
```

## Remaining Gates

All three SSH aliases failed DNS resolution in this environment; no remote job
or scratch directory was created. Required work still includes concrete
RootSession/RootConnection admission, a fresh post-readiness challenge, actual
root-owned occurrence custody, exact retirement/replay across issuer replacement,
coherent FD12 migration, the production owning attempt and cleanup, protected
proof execution, all 47 positive/negative simulator and GPU qualifications, honest
website/release gates, and verified identical publication to both public mains.
