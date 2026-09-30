# Native V3 Activation Prerequisite

Status: **private, unqualified prerequisite, not a deployable default switch**.
The paired executable, unit and offline installer changes must remain private
until the intended original-root direct startup route is composed and tested.
The native entry now prepares the independently retained anchor, keeps the
actual `Prepared` owner and activates its sole original-root listener. It runs
the bounded intake receiver on that owner and the original request account.
This source has not been built or tested by its author. The old indirect V3
`Deployment::launch` / `PreparedCompilerExecutionSupervisorV3::launch` still refuses with
`native V3 indirect launch requires the original-root FD12 route`.
Here FD12 means the issuer's original-root control socket, not the unit's
policy-input slot in the descriptor table below.
That guard is unchanged. A complete inert intake currently returns only
`RuntimeEnforcementUnavailable`, then cancels and drains the anchor through its
original cleanup pool. Existing service-manager activation notification is not
compiler readiness. No successful startup, compiler launch, proof or GPU result
is claimed.
M0 alone is complete; M1-M7 and the strict production matrix remain incomplete.

## Paired Contract

The dedicated coordinator and provisioner mains select their existing unsafe
V3 entrypoints exactly once and terminate on every return or unwind. There is
no argument-selected family, old-record upgrade, retry or V1/V2 fallback.
The coordinator retains the original activation, signal owner, request account
and independent cleanup account. `KillMode=mixed` and the external manager's
whole-cgroup termination obligation are unchanged; the library does not admit
the effective unit configuration merely from the checked-in file.

The unit opens exactly fourteen descriptors, in this order:

| FD | Role | Fixed input |
| --- | --- | --- |
| 3 | runtime-root | `/run/fe2o3` |
| 4 | supervisor-root | `/var/lib/fe2o3/compiler-execution` |
| 5 | anchor-root | `/var/lib/fe2o3/external-anchor` |
| 6 | supervisor | `fe2o3-compiler-execution-supervisor-v3` |
| 7 | launcher | `fe2o3-static-preexec-launcher` |
| 8 | issuer | `fe2o3-compiler-execution-issuer-conditional` |
| 9 | anchor-helper | `fe2o3-external-anchor-provisioning-helper-v3` |
| 10 | anchor-daemon | `fe2o3-external-anchor-service-v3` |
| 11 | supervisor-deployment | `supervisor-deployment-v3` |
| 12 | issuer-policy | `issuer-policy-v3` |
| 13 | anchor-deployment | `anchor-deployment-v3` |
| 14 | anchor-provisioning | `anchor-provisioning-v3` |
| 15 | issuer-key-seed | `issuer-signing-key-seed-v3` |
| 16 | anchor-key-seed | `anchor-signing-key-seed-v3` |

Images are beneath `/usr/libexec/fe2o3`, records/seeds beneath
`/etc/fe2o3/compiler-execution`. All OpenFile entries are read-only.
The native provisioner measures the same five images and publishes the same
records plus public `client-profile-v3`. The profile is not an activation FD.
The lifecycle lock remains `compiler-execution-lifecycle-v1`, derived from the
retained state roots; changing record families does not create a second lock.
Existing V1 configuration is not overwritten, renamed or inferred as V3.
Missing native files or V1 bytes in a native record slot refuse.

The bounded capability set adds only `CAP_DAC_READ_SEARCH` (lifecycle `..`
traversal through service-owned mode-0700 roots) and `CAP_SYS_PTRACE` (actual
namespace observation of the distinct-UID, nondumpable anchor after profile
readiness). These do not replace any process/namespace observation. The native
process fixtures do not qualify this exact systemd unit or its host LSM policy.

## Versioned Offline Installation

Schema ownership stays in `fe2o3-compiler-execution-deployment`, coordinated
under #272. This change allocates no KIR, proof, capability, RPC or compiler
opcode. The frozen V1 header, roster, BUILD-INFO and public verification and
qualification APIs retain their meaning. V3 has a separate header
`fe2o3-compiler-execution-install-manifest-v3`, file `INSTALL-MANIFEST-V3`,
`schema_version=3` BUILD-INFO and nominal verified/installed owners.
Neither nominal owner converts to its V1 counterpart.

V3 reuses the V1 canonical grammar, out-of-band manifest SHA-256 and Git commit,
static target, modes and byte limits. It binds twelve content files: BUILD-INFO,
SHA256SUMS, unit, sysusers, tmpfiles, coordinator, provisioner and the five images
listed above. The V1-only client checker is deliberately absent, not presented
as a native client. The manifest itself is the thirteenth installed file.
The exact independent grammar/roster goldens are test-only synthetic records,
not approved executables or installable bundles.

Both families share retained-directory inventory checks, bounded stable reads,
sealed source copies, create-new copying, no-replace publication, fsync,
reacquisition and interrupted-staging cleanup. The caller selects a closed
versioned API before opening inputs; bundle bytes never select the profile.
V3 roots use `compiler-execution-v3-<manifest_sha256>` and staging uses
`.compiler-execution-v3-staging-<random>`. Recovery refuses mixed-family roots
or staging before deleting anything. Recovery preserves a named published root
but does not verify its contents; installation/reacquisition performs that check.

The paired default builder uses the existing V3/conditional child build options
and the explicit V3 manifest/verifier mode. Protected child image builders retain
their secure pre-runtime entry checks. Coordinator/provisioner/tools retain
ordinary Rust entry with static/no-dynamic-loader checks; they are not claimed
to use the protected children's secure shim. No image is relabelled as V1.

The private candidate route, for later primary validation, is:

```console
scripts/build-static-compiler-execution-deployment.sh BUNDLE_ROOT
fe2o3-compiler-execution-deployment-verify --v3 BUNDLE_ROOT MANIFEST_SHA256 GIT_COMMIT
fe2o3-compiler-execution-deployment-install --v3 BUNDLE_ROOT MANIFEST_SHA256 GIT_COMMIT INSTALL_PARENT
fe2o3-compiler-execution-deployment-install --v3 recover MANIFEST_SHA256 INSTALL_PARENT
```

Installation requires the existing root-owned mode-0700 offline parent and
publishes an offline root, not an atomic update to a running host's `/usr`.
Within the prepared deployment, the dedicated native provision command accepts
one canonical nonzero generation and uses fixed root-controlled paths and NSS
accounts. It keeps exclusive lifecycle custody, no-replace record publication
and zeroizing seeds. No installer option supplies authority from arbitrary
files or turns install success into service readiness.

Tool invocations without `--v3` retain V1 behavior for old bundles. The V1
qualification executable still admits V1 only and cannot qualify this V3 root.
Do not pass a V3 digest to it or use its historical result as V3 approval.

## Validation And Remaining Work

Authored coverage includes exact descriptor/image/record roles, genuine V1
record rejection with no resource denial, V1/V3 manifest goldens, cross-family
and mixed inventory refusal, immutable source custody, install/reacquire/tamper,
all shared publication fault points, unwind, parent replacement and isolated
crash recovery. No builds, tests or privileged startup were run by this author.
Subsequent primary integration builds, fixture corrections, the strict V4
child-entry fix and partially completed tests are recorded in
[the candidate evidence](evidence/native-intake-restrictions-20260930.md).
Socket-dependent and native-exec validation remain incomplete.

## Authenticated Intake Candidate

The private V4 release/broker family carries a freshly loaded, fixed-origin V3
client profile; the old V3 transport still means V1 profile. Distinct release
magic, authentication domains, route grammar and exact profile identity reject
cross-family records. There is no V1-to-V3 conversion or downgrade. The V4
release launch remains deliberately unselected by the working default driver.
Selecting it is a later paired validation/integration step, not a route hint
that upgrades authority. The new profile's load/admission account survives
descriptor transfers, thread moves and the retained invocation stream.

The protected wrapper's V4 branch retains its genuine invocation authority
stream while it contacts the fixed root endpoint. It prepays the bounded native
descriptor capture before copying, retains the actual cwd and original stdio,
and never enters the old local compiler spawn or FD195 readiness path. Legacy
load-readiness/publication recovery refuses in this branch. These quotas cover
the native descriptor capture and transport, not every allocation in the older
Command/environment preparation pipeline.

The paired sender/receiver now use inert 240-byte V4 intake records, not an
upgrade of the retained 224-byte V3 codec. Both versions share the same closed
canonical engine; each rejects the other version, and V3 still rejects role 6.
Hello, challenge and ACK carry no rights; ordered input records carry exactly
one CLOEXEC right: sealed invocation, cwd, mandatory output directory (role 6,
child destination 197), then each originally non-CLOEXEC stdio selected in the
mask. Every V4 frame binds the output device/inode at bytes 192..208 before its
digest at 208..240. Neither scalar coordinate grants authority; the actual
received descriptor must match. The actual
invocation length must match the bounded declaration before read/admission.
Fresh nonce/challenge and preceding digests bind order, but do not authenticate
the peer. The fixed root-owned directory chain, socket identity, actual root
SO_PEERCRED and per-record SCM_CREDENTIALS are the transport TCB. The V3 profile's
supervisor UID/GID is not reinterpreted as a measured root identity. The wrapper
borrows its original brokered `PinnedDirectory` through the exact ACK, checking
its actual FD and full bounded capacity on its original funded profile account.
Root never reopens a wrapper pathname or searches a PID's descriptor table.

The coordinator accepts through the original listener; issuer staging exports
only its root directory, never a competing listener. Received rights enter the
outer receiver before fallible validation. Native invocation admission uses a
separately funded duplicate so constructor failure cannot remove the original
right. The receiver keeps those rights through the exact terminal refusal ACK
and handled cancellation/drain. Dropping Prepared starts anchor cancellation;
the original pool retains its child/domain and canonical guard until terminal
cleanup. No failed request refunds its account or retries on a fresh ledger.

The receiver similarly keeps the original output right and installs a separately
prepaid, move-only `CompilerOutputDirectory` duplicate before leaving the intake
scope. Errors and unwind cannot remove the original right or refund either
reservation. The existing `CompilerInvocationBacking::prepare` now requires and
consumes this full output owner on the same account; revalidation includes it,
staging quotes cover its additional duplicate, and final output validation reads
the actual Stage binding at 197. The current refusal-only receiver does not
extract it or construct a fake runtime to call that preparation. The accepted
descriptor route is exactly `/proc/self/fd/197`, without argv/env rewriting.
This is directory custody only, not a read-only snapshot, source mapping,
namespace isolation, publication lock or a runtime-enforcement guard.

Additional authored, unrun coverage checks V3/V4 refusal, output identity on all
transcript joins, missing/duplicate/wrong output inputs, pre-ACK trailing input,
original right retention after constructor/outer failure, one-short funding,
original/moved/foreign budgets, actual Stage destination checks and unwind.
One new unsafe test block only stages inert owned descriptors and never spawns.
No native qualification, proof, compiler or GPU claim is added.

New authored coverage includes real one-right socket transfer/closure, all
stdio masks, original right retention on constructor denial, malformed sequence,
unwind, one-short funding before dequeue, original-account profile transfer,
family negatives and entry cancellation order. These source tests are unrun.
They do not instantiate an approved compiler runtime or qualify a service.
The primary native lane still needs the actual nonroot client endpoint and
paired executable exchange, with bad peer/record credentials, substituted
directory/socket, timeout/EOF, unexpected/extra rights and exact-ACK stream
lifetime checks. No transport ACK can create RootSession, Ready or proof.

Before landing default switches: test the sole original-root direct startup and
intake route with complete cleanup and exact unit confinement. Then connect
admitted runtime/source-view custody through `CompilerInvocationBacking`, the
existing Stage/Trace and `Prepared::launch_root_attempt`. The actual root must
join the complete profile/anchor association; today's inert refusal transcript
binds the policy and descriptor, not full compiler authority. Invocation authority
must survive until the exact root acknowledgment before using the child-created
FD195 channel.
Genuine cwd/stdio/source/runtime/proof custody, original-account funding and
trace-derived completion remain required. `RuntimeEnforcementUnavailable` and
the indirect-launch guard must not be removed to make activation appear ready.
