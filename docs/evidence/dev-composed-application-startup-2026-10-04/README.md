# Composed Application Startup Qualification

The production Cargo/host registration path now passes a composed CPU campaign.
That campaign exposed and fixed a real issuer-launch failure: an empty-capability
supervisor cannot open a non-dumpable child's proc namespace links. Ordinary
verified multi-GPU application admission remains incomplete.

Source: `c5c4ec1da8026ca2d03bd7ff93787ae9232fb5b2`.
The signed source audit, exact patch, frozen inventory, commands, selected binary
hashes and final logs are in [evidence.tar.gz](evidence.tar.gz).

## Namespace Fix

The pre-clone namespace snapshot is copied into the retained staged launch. Fixed
post-clone code checks all ten of its own namespace device/inode identities with
raw openat/fstat/close before emitting the private profile-ready byte. No allocation,
callback or caller executable runs in that check. A failed check reports child
stage 13 and follows the existing exact-child containment/reaping path.

Only the inaccessible parent-side child namespace reads are removed. Parent
namespace continuity, PID/time-for-children equality, original atomic pidfd,
parent-death signal, child proc credential/profile validation, currentness checks
and the private execution gate remain. Dumpability and capabilities are not relaxed.
Both native agent reviews found this replacement sound at the stated code boundary.
No new formal theorem or Linux refinement is claimed.

## Composed Campaign

One root campaign runs five cases: descriptor, roster, delayed transitions,
forced clone3 ENOSYS fallback, and cancellation during initial registration
preparation. It uses Cargo production context 3 and the strict static host
consumer under Cargo's actual pre-exec filter, public supervisor/registry APIs,
the actual static launcher/issuer, and a distinct-UID static anchor service.
The fallback case only tightens the helper's syscall policy.

Positive cases require successful Cargo exit, admitted/current host reports,
cleared loader environment, ACK EOF, original receipt recovery, issuer reaping,
service exit and registry drain. The delayed case pauses after every successful
registry step. Applications exit immediately after their strict startup checks.
Cancellation must reject in observer-backed preparation and cannot report host
admission; it does not test cancellation of an installed proof-bearing session.

This is deliberately a component fixture: the supervisor is a private dynamic
libtest helper, keys/publication are synthetic, and no FD195 audit is requested.
The issuer's recorded ordinary exit status 1 after that endpoint closes is
expected, not a successful compiler audit. Neither measured systemd deployment
nor genuine compiler receipt acquisition is qualified by this campaign.

The controller in `scripts/qualify-composed-application-startup.sh` requires
prebuilt artifacts and creates private mount/PID namespaces and tmpfs state. The
harness checks original versus private namespace objects, PID1/procfs agreement,
directory modes and five-role executable distinctness before fixed-path writes.
Namespace identity is rechecked before cleanup. Direct helpers are killed/reaped
on failure; full descendant containment relies on bwrap's private PID1 teardown.
The archived outer runner bounds the campaign to 240 seconds and waits for exit.
This trusted controller is not a defense against malicious root provisioning.

## Qualification

The frozen run passed 945 top-level checks, excluding nested helper reruns:

| Selection | Passed |
| --- | ---: |
| Cargo, broker, client, coordinator and supervisor unit/binary/integration | 802 |
| Strict static application tests | 18 |
| Doctests | 109 |
| Static launcher tests | 15 |
| Composed root campaign, five scenarios | 1 |

The ordinary selection has 30 default-ignored entries; not all were enabled.
New supervisor checks include 20 device/inode comparison mutations, ten complete
launch-stage mismatch/reaping cases and a non-dumpable subprocess positive.
Clippy passes for the five packages' all-target selections and feature-enabled
Cargo with warnings denied. Scoped formatting, shell syntax and diff checks pass.
All 5,097 selected source/config hashes remain unchanged; the source audit checks
23 test binaries and nine selected service/application/launcher binaries.
Three static service-image checks also passed during artifact preparation.

## Remaining Priority

Next implement the fixed keyless proof custodian and its authenticated retained
remote owner, then the conditional-only native invocation transition. One proof
must remain retained across both selected device invocations and any uncertain
native outcome. Reuse current-thread Context execution and the existing native
full64/272-byte argument/storage checks. Qualify the ordinary u32 fill, guarded
staging, settled H2D, PUBLIC XGMI and full readback on two free MI300X devices.
General opcode expansion, larger device matrices and performance campaigns remain
deferred. MI300X was not used for this checkpoint.

Source patch SHA-256:
`a931b36599c94ae13264411c6f389f9c774e4be6c6398a204c767c075a0eb3fd`

Archive SHA-256:
`05200d8a5961818758130ac79eded1cd138f540c60c612a83b86f33572e39e55`

The archive's manifest covers all bundled evidence files. Its source audit
verifies the SSH signature, committed patch, frozen inventory and binary hashes.
