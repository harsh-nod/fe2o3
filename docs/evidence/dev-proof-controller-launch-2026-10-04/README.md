# Proof Controller Launch Qualification

Source: `f875c714cd180664f6d583c9a30919bfce0e3de6`.
Parent: `dd1cbb6f39acdbf5a860d38713584def2c658c97`.
This qualifies the process-launch prerequisite for the ordinary two-GPU proof
custodian. It fixes the shared launcher's lost parent-death signal and adds a
distinct proof-controller profile without weakening the existing signing-service
or application profiles. A3 and ordinary two-GPU application qualification remain
open; this is not a deployed custodian, executed-proof campaign or GPU benchmark.

## Implementation

The shared launcher previously armed `PDEATHSIG` before changing UID/GID, which
clears that setting. It now retains the initial arm and re-arms after all profile
changes, before readiness, with exact parent checks before and after setting and
an exact `SIGKILL` readback. Ordinary exec preserves this setting for the admitted
non-set-ID, capability-free executable. See the
[Linux parent-death signal contract](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).

`ProofControllerCredentialProfileV1`, `ProofControllerProcessProfileV1`,
`StagedProofControllerExecV1` and `RootOwnedProofControllerChildV1` are separate
types with no conversion into the locked service roles. A private closed enum
shares descriptor staging, atomic clone/pidfd acquisition, signal normalization,
credential installation and exact child cleanup. No arbitrary public securebits
configuration or numeric-PID reopen was introduced.

The proof role requires non-root equal real/effective/saved/filesystem IDs, empty
groups and all capability sets including bounding, NNP=1, dumpable=0, core=0,
umask=077, securebits=0 and no seccomp. Parent preflight rejects incompatible
securebits and seccomp before clone. Current-thread observations use
`/proc/thread-self`; retained profile revalidation is PID/TID-bound and not Send
or Sync. Namespace identities are retained without a thread-count/quiescence
claim. Parent-side child validation remains deliberately proc-visible only.

The existing compiler coordinator systemd service installs an inherited seccomp
filter, so it cannot directly launch this proof role. Its confinement is unchanged.
The production custodian needs its own independently approved unfiltered root
launch boundary. Static secure entry is also mandatory: ordinary exec can reset
dumpability before Rust startup.

## Qualification

| Check | Result |
| --- | --- |
| Profile and spawn unit tests | 11 passed, 6 root/helper tests ignored by default |
| Profile and spawn type-boundary doctests | 12 compile-fail tests passed |
| Compiler and anchor coordinator tests | 34 passed, 3 deployment tests ignored |
| Coordinator doctests | 10 compile-fail tests passed |
| Private-root campaign | 14 top-level test invocations passed |
| Profile and spawn Clippy | All targets/features passed with `-D warnings` |
| Static musl profile fixture | Secure entry, static ELF, no undefined symbols passed |
| Formatting, whitespace and frozen sources | Passed |

These are 81 top-level passing test executions, including repeated parameterized
root cases, not 81 distinct test functions. The root campaign covers nine existing
clone/error/signal/restoration cases, one fail-stop containment case, two incompatible
parent cases and two four-case matrices. Calling-thread-only seccomp rejection
explicitly verifies that the process leader remains unfiltered.

The first matrix launches a sealed raw static image for both profiles through
actual clone3 and test-forced legacy clone. It checks `PR_GET_PDEATHSIG` after exec,
reports `SIGKILL` and pauses. Only then does the direct parent call `_exit`, bypassing
Drop. The subreaper requires the transferred original pidfd to report death by
`SIGKILL` and reaps it. EOF cannot make this fixture pass, and native cases assert
they did not silently fall back. The test hook does not install inherited seccomp.

The second matrix executes the sealed musl fixture through the actual shared secure
entry. It requires fixed argv, empty environment and successful full proof-profile
capture/revalidation before reporting. Both clone routes pass and terminate on
parent death. Both routes using the old locked profile instead exit with the exact
rejection code. No static profile success is presented as analyzer or Verus success.

## Evidence

[Evidence archive](evidence.tar.gz) SHA-256:
`75651a6ec62c339ae82684734db1b051ca5df9365832874e2d0ef4897badd152`.
Source patch SHA-256:
`11ed1c4612dedf098cf704303e969e0f46f9aaade88584786425e722e4cb3563`.
The archive contains the verified signed source commit, identical qualified and
committed patches, 6,312 source/config hashes, two binary hashes, commands, logs
and static ELF inspection. Manifest, archive comparison, final source and binary
rechecks passed. Binaries and private keys are excluded.

All root tests ran in disposable local PID/mount/network/IPC/UTS namespaces, with
no host deployment changes or MI300X use. Owned scratch is removed after packaging;
the existing shared build cache is retained. This is focused Linux process and
type-boundary evidence, not a formal proof of the Rust launcher or full workspace
qualification.

## Next Multi GPU Gate

Run the real authenticated analyzer and protected Verus from the fixed child,
using deployment-approved resources opened after exec, and retain the original
executed proof. Qualify analyzer UID-map admission, ptrace supervision and full
descendant containment there. Then implement measured custodian deployment and
the authenticated application-session handoff, retain proof custody until both
devices actually settle, and join existing per-device native admission to the
fill -> staging -> H2D -> PUBLIC XGMI -> guarded readback path in both directions.
Broader parity matrices and performance tuning remain lower priority.
