# Observer Publication Lock Retention

Parent: `0d9cdfbe9e77f0183f34868ca6cef00dbb12741d`.
This closes a lock-lifetime prerequisite for the ordinary multi-GPU application's
compiler admission. It does not implement the authenticated observer channel,
remote signing guard, deployed compiler acquisition, or GPU invocation authority.

## Implementation

A coordinator pidfd check or parent-death signal cannot make the interval from
final observation revalidation through signing and durable commit atomic. An
abrupt coordinator exit closes its publication token without running containment.
The future issuer guard therefore needs independent retention of the same locks.

`CompilerModuleHandoffLockRetentionV3` retains exactly the original named artifact
OFD lock and output-directory flock descriptions. Export requires a nonrepairing
descriptor-root observation, rejects an additional pathname guard, and duplicates
the actual open descriptions with close-on-exec. It never reopens their paths.
The semantic token and process-local lock reservation are not copied. Currentness
is checked before and after export; broker export also rechecks the live compiler
occurrence before and after. Previously retained locks survive failed revalidation.

The receiving constructor consumes exactly two descriptors and checks role, type,
access flags, linkage and close-on-exec. These checks are deliberately inert:
shape-valid reopened files pass, but they do not retain locks or grant authority.
The future channel must authenticate their origin and exact operation. Both
owners use the existing artifact-spawn barrier and close-only destruction; no
explicit unlock can invalidate another owner's aliases.

These are real scoped filesystem capabilities, not harmless bytes. They must
remain private to the trusted issuer and must never enter the application channel.
Ordinary path-backed tokens are outside this two-descriptor profile and reject.

## Qualification Scope

The new Linux tests cover independent contention on each lock, original-token
release, actual `SCM_RIGHTS` transfer, sender `SIGKILL`, rights still queued when
the sender dies, abandoned queued delivery, named-lock replacement, malformed
import cleanup, and release under the artifact-spawn barrier. A reopened-descriptor
negative control demonstrates that shape checks do not establish lock authority.
The helper readiness wait has an absolute deadline and every helper is killed and
reaped by its owner.

The broker fixture additionally retains the locks after the live compiler exits
and after the original occurrence is dropped, and rejects a new export once the
compiler has exited. The root qualification uses an isolated private namespace,
UID1000 client, and mode-0700 publication root. It is a waiting-process/synthetic
handoff fixture, not genuine Rust extraction or signer execution. No MI300X
resources, real deployment configuration, or host-wide ptrace settings are changed.

| Check | Result |
| --- | --- |
| Artifact library | 218 passed, 1 subprocess-helper ignore |
| Artifact ownership/API doctests | 13 passed |
| Lock-retention tests with four test threads | 5 passed, 1 helper ignore |
| Broker library | 186 passed, 7 helper/root-fixture ignores |
| Broker doctests | 47 passed |
| Isolated root/UID1000 cases | 2 passed |
| Coordinator / issuer / supervisor libraries | 31 / 3 / 47 passed, 2 static-launcher ignores |
| Broker all-target and artifact library Clippy | Passed with `-D warnings` |
| Artifact all-target Clippy | Passed with the existing exception below |
| Targeted rustfmt and whitespace | Passed |

Artifact all-target Clippy retains only the pre-existing `too_many_arguments`
exception for `tests/worker_v3_publication_intent.rs:185`; no source lint suppression
was added. A development enum-naming lint was fixed, with its diagnostic retained.
Both native agents completed read-only reviews without remaining findings.

The broker executable hash is unchanged before and after root qualification:
`15b3f9fd8894c718c21763b2c2b6ad33d4384cfa0a2fd66f2a11d509623b69d5`.

[`evidence.tar.gz`](evidence.tar.gz) contains the exact code patch, scripts, logs,
exit statuses and executable hashes, but no binaries or signing secrets.
Archive SHA-256:
`c30b9813077de77a68077fbb04f0f7a12eb541b37e225dde36b38ef67198dc92`.
Code-patch SHA-256:
`a609be3fd6f6c72706e40198a880b9fa19c6acb960e684e788788cca5578cf12`.

## Limits

This is kernel-behavior and ownership/API qualification, not a new formal proof
or a completed multi-GPU milestone. Production remains fail-closed until the
coordinator registration/session table and private issuer guard are connected.
The guard must retain these descriptions through durable commit and poison the
service on failed operation handling. Authentication, replay rejection, exact
issuer containment and genuine compiler acquisition still require integration
qualification before the two-GPU application run.
