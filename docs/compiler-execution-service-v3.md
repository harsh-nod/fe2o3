# Native V3 Activation Prerequisite

Status: **private, unqualified prerequisite, not a deployable default switch**.
The paired executable, unit and offline installer changes must remain private
until the intended original-root direct startup route is composed and tested.
The native entry now prepares the independently retained anchor, keeps the
actual `Prepared` owner and activates its sole original-root listener. It runs
the bounded intake receiver on that owner and the original request account.
Local builds and tests have exercised parts of this candidate, but the affected
results require an isolated-cache rerun; see
[validation evidence](evidence/native-intake-restrictions-20260930.md).
The old indirect V3
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

The bounded capability set includes `CAP_DAC_READ_SEARCH` (lifecycle `..`
traversal through service-owned mode-0700 roots), `CAP_SYS_PTRACE` (actual
namespace observation of the distinct-UID, nondumpable anchor after profile
readiness), and `CAP_SETFCAP` (the existing fresh-helper identity map includes
parent UID 0). The latter is required by the kernel's
[UID-0 mapping rule](https://man7.org/linux/man-pages/man7/user_namespaces.7.html).
These do not replace process/namespace observation or change the final child's
capability drop. The native process fixtures do not qualify this exact systemd
unit or its host LSM policy.

### Creator And Child Boundaries

The paired unit permits creator `clone3` with `RestrictNamespaces=no`, while
denying `unshare` and `setns` with `EPERM`. This is not a workload exemption:
the existing protected-spawn boundary must install namespace confinement for
every compiler stage and every actual mapping-gated helper before exec. Generic
unmapped creator services retain their existing child-creation behavior; they
are not proof helpers. The compiler memory restrictions remain additional.
Systemd v255's
[namespace filter implementation](https://github.com/systemd/systemd/blob/v255/src/shared/seccomp-util.c)
unconditionally denies `clone3` whenever any namespace restriction is selected,
so a narrower `RestrictNamespaces=user` would still prevent the creator route.
Borrowed runtime inputs and these root creator privileges are not final-child
authority, runtime approval, or a reason to release the execution gate.

`ProtectControlGroups=yes` stays enabled. The unit fixes `Slice=system.slice`
and adds only `/sys/fs/cgroup/system.slice/%n` to `ReadWritePaths`. The two
existing state-root exceptions are unchanged. Empty `Delegate=` enables
subtree ownership without requesting controllers, as specified by
[systemd v255 resource control](https://github.com/systemd/systemd/blob/v255/man/systemd.resource-control.xml).
The service does not enable domain controllers on its populated parent. Its
manager owns the unit cgroup and the outside whole-service termination duty;
only the privileged creator controls fresh child domains. No final child may
receive these control FDs, mutate the subtree, migrate, or delegate it.

The writable exception is a submount, not a writable hierarchy. Systemd's
[path-access contract](https://github.com/systemd/systemd/blob/v255/man/systemd.exec.xml)
allows a writable descendant of a read-only path; the v255
[mount implementation](https://github.com/systemd/systemd/blob/v255/src/core/namespace.c)
excludes separately configured descendants from the parent's read-only remount.
The host backing superblock must already be writable. Host mount propagation
and privileged writers must remain excluded by the existing deployment contract.
No runtime string or source unit check proves that external administration.

`NativeCgroupDomainV1::prepare` derives its path from actual proc membership,
not `%n` or a supplied FD. Prefix traversal retains `NO_XDEV`; only its final
canonical component may cross the paired mount. Admission requires the same
cgroup2 filesystem, protected root ownership, and a complete protected
`cgroup.type` record equal to `domain\n` before checking this creator's direct
PID membership. A `domain threaded` PID list aggregates its subtree and cannot
establish that direct membership. `threaded`, `domain invalid`, malformed and
missing type records refuse before admission or mkdir. Prefix/final identities
and proc membership must also remain unchanged.
Actual `/` membership is unsupported: the hierarchy root has no `cgroup.type`,
and no domain-type observation is manufactured for it. A non-root path resolving
to a hierarchy-root mount also refuses at the missing type record. Root-path
parser fixtures still recognize valid proc syntax; they are not domain admission.
All generated-domain and control-file opens still use `NO_XDEV`. The added
worst-case preparation quote is 100352 logical work and
`4096 + 4 * size_of::<Stat>()` scratch; both flow through existing fresh-domain
and fresh-namespace quotes on the original account. Retained storage and cleanup
are unchanged. Refusal before mkdir closes only temporary FDs.
The existing quote also covers the parent-type check: six extra syscalls
(open, filesystem/descriptor/named metadata, two bounded reads) and one close.
Combined with the mount-admission delta, this is twelve syscalls and three
temporary closes plus bounded byte processing, within the existing
`32 * 1088 + 16 * 4096` allowance. The type and PID-read buffers are sequential;
the six-buffer/twelve-stat scratch envelope is unchanged. These are conservative
logical bounds, not measured syscall-time or exact operation-census claims.
The helper plus compiler launch envelope therefore gained 200704 work and,
on x86-64, 9344 scratch from this preparation change. Monitoring does not repeat
this preparation allowance, and it is not a hidden global budget-limit increase.

Authored source checks reject a broad/wrong-subtree writable path, duplicate
assignments, a missing capability, and creator/filter regressions. Parser and
metadata controls are not mount evidence. The ignored
`native_cgroup::tests::mounts::actual_membership_mount_boundary` uses an exact,
timed subprocess with a private mount namespace: it checks actual preparation
through a writable final submount under a read-only root, rejects a prefix
mount, an ordinary-domain ancestor without direct PID membership and wrong
filesystem, and verifies that type/procs control-file submounts
are still refused. It creates/writes no cgroup and tests no
compiler, mapping, spawn, delegation or service startup. Its setup needs isolated
root, `CAP_SYS_ADMIN`, a writable unified hierarchy, ordinary-domain membership
and a non-root ordinary-domain ancestor, no intervening mounts, stable privileged
state and an outside custodian. It must run outside the unit's denied-unshare context;
the service does not gain `CAP_SYS_ADMIN` for this test.

Separate controls cover physical-hierarchy and namespace-root substitution.
The former requires an actual root without `cgroup.type`; the latter checks an
ordinary-domain namespace root and refuses at direct PID membership. A private
namespace root does not establish the physical-root control's prerequisite.

The separate ignored
`native_cgroup::tests::mounts::actual_threaded_domain_membership_is_refused`
performs only reads in an operator-provisioned isolated topology. Its process
must have actual threaded-leaf membership with a protected non-root threaded-domain
ancestor, exposed at the resolved final membership submount. Prefix traversal
must remain mount-free. The test independently checks matching ancestor/final
inodes, actual `domain threaded\n` bytes and this process's presence in the kernel
PID list, then requires the exact ordinary-domain refusal from `prepare()` and
unchanged proc membership. Missing setup fails rather than skipping or earning
negative credit. Its timed child creates no mounts or cgroups, migrates no process,
and writes no control files; the operator retains the outside cleanup obligation.

Existing generic fresh-domain process fixtures, including
`root_exit_does_not_retire_live_descendant_domain` and
`fresh_user_namespace_preserves_profile_and_aggregate_cleanup`, now require the
root-credential test process to live in a non-root ordinary domain. Running them
at actual `/` is intentionally unsupported, not permission to bypass the check.
The provisioned root-request fixture already requires a non-root `domain\n`
parent. None of these fixtures has been rerun for this parent-type correction.

The source contract and its mutation checks passed on September 30. The complete
coordinator/spawn library rerun passed 567 tests, with 46 tests ignored.
Eight selected mapped-child/mount controls subsequently passed on MI350; the
physical-root and threaded-domain controls remain unrun. See
[the native observation checkpoint](evidence/native-invocation-runtime-20260928.md#exact-child-observation-on-mi350).
An effective-unit/drop-in
inspection and a genuine paired service run are still required: creator
clone3 and UID-0 map, root/sibling write denial versus owned-subtree creation,
mapped-helper/compiler namespace denials and legal thread/fork controls, and
outside-manager timeout/crash cleanup of the entire service. The older standalone
creator diagnostic expects `unshare(0)` to succeed and is not the unit baseline.
The V2 coordinator/supervisor/issuer chain also needs its own regression run;
a generic-stage fixture does not establish that chain. All existing approval,
proof and runtime-enforcement gates remain closed.

### Locked Exact-Child Personality Observation

The unit still sets `LockPersonality=yes`. Systemd v255 denies personality
arguments other than the selected native personality, including the syscall's
read-only `0xffffffff` query. The candidate now replaces that query with one
explicit kernel proc observation; it does not interpret `EPERM` as a value,
clear `READ_IMPLIES_EXEC`, or open an unlocked startup interval.

Only the actual compiler child acquires the observation. After its real mapping
gate, before credentials/dumpability change, it opens `/proc` with `openat2`,
requires genuine procfs and protected root-owned inode metadata, and reads the
kernel's `thread-self` link into a fixed 32-byte stack buffer. The canonical
`<pid>/task/<tid>` must match raw calling-task `getpid/gettid`; this direct child
must also be its thread-group leader. Numeric task and personality opens are
relative to the retained root/task directories with `BENEATH`, `NO_SYMLINKS`
and `NO_XDEV`, on the same procfs device. The directories are closed immediately.
The sole private personality FD is neither supplied by the caller nor exported.
The kernel's [thread-self implementation](https://github.com/torvalds/linux/blob/v6.18/fs/proc/thread_self.c)
names the calling task in that procfs mount's PID namespace. A mismatching view
refuses; this is not independent namespace-inode attestation.

The actual clone record uses PIDFD/CLEAR_SIGHAND, optional INTO_CGROUP, and
NEWUSER only for the typed mapping route: no NEWPID, NEWNS, THREAD or FILES.
That route rejects a pending parent PID transition and revalidates unchanged
child PID namespaces before releasing its gate. Both UID and GID maps contain
`0 0 1`, plus identity rows for the admitted child and peer. Thus parent-root
proc ownership remains visible as inner UID/GID zero when the child opens it;
the later profile drop uses those same mapped nonzero IDs. The isolated MI350
mapped-child control executed this sequence and checked the actual mapping rows;
it does not establish the installed service's deployment provenance.

After profile drop, the same child rechecks its PID/TID, preads exactly eight
lowercase hex digits and a newline, and separately checks EOF at offset nine.
It refuses short/extra/malformed bytes, syscall or close errors, and bit 22
(`READ_IMPLIES_EXEC`), without modifying personality. The FD closes before the
unchanged compiler filter, READY, or exec, and explicitly closes on intervening
profile/channel failures. Acquisition and consumption have separate terminal
status stages (15 and 16); filter installation retains stage 13. No partial
observation can satisfy the consumer.

Both child mapping-gate FDs close before acquisition. No common closefrom or
descriptor remap runs between acquisition and consumption: profile setup changes
credentials/capabilities only, and channel setup closes only its distinct owned
sockets/control FD. Common `close_range(CLOEXEC)`, stdio closes and final dup3
run later, after READY and gate release, with the observation already closed.

This relies on Linux's [proc personality reader and retained PID inode](https://github.com/torvalds/linux/blob/v6.18/fs/proc/base.c)
and [same-thread-group ptrace access rule](https://github.com/torvalds/linux/blob/v6.18/kernel/ptrace.c).
Acquisition occurs before proc ownership becomes inaccessible after the
credential/nondumpability transition; consumption still invokes the kernel
reader's current access check. No cap, root read, parent snapshot or alternate
reader replaces a failed post-drop read. The deployed kernel/LSM must actually
permit this exact sequence, or launch remains refused. Foreign descriptor,
mount and task mutation remain excluded by the existing unsafe caller contract,
not proved by these checks. Exec-established personality is still outside this
pre-exec observation's claim.

The original parent prepays all work/scratch before clone. Observation work is
`19 * 1088 + (32 + 10 + 1) * 64 + 256 = 23680`: four identity syscalls, three
opens, three fstatfs, three fstat, one readlink, two preads and three closes,
including refusal cleanup without retry. Replacing the old single query raises
compiler child work by 22592, to 30848 for observation plus filter installation.
Scratch adds `6 * sizeof(stat) + 2 * sizeof(statfs) + 2 * 24 + 2 * 12 + 64 + 20
+ 1024`, or 2284 bytes on x86-64; this is a conservative logical frame quote,
not generated stack/RSS. Existing coordinator compiler-setup work already bounds
the complete 46144 extra work for cwd, observation/filter and child channel;
its two spawn scratch quotes gain 4568 bytes. No retained owner,
cleanup account, retry budget, process limit or runtime gate changes.

Authored parser/quote units are not kernel evidence. Ignored isolated controls
cover an inherited native-ABI equivalent of the unit personality lock, a truly
mapped compiler child, one-short original work/storage, actual dirty inheritance,
and kernel-denied record/EOF reads reaching stage 16 with no READY/exec. Positive
controls inspect the actual gated child's FDs to check closure before READY,
PID-namespace continuity, and actual UID/GID mapping rows on the mapped route,
then execute the existing static diagnostic, including unchanged blanket
personality denial and legal fork/exec. The locked positive also reuses actual
child-channel transfer and backpressure controls, not a synthetic FD substitute.
Exact-test subprocesses have a timeout, direct-child kill/wait custody, bounded
log reads and reached markers; the
outside custodian still owns whole-tree retirement. Six unmapped controls and
six mapped controls passed in isolated MI350 containers; the latter were rerun
with two passing mount controls after correcting the physical-root fixture
assumption. Other ignored controls remain unexecuted. These tests require
isolated root, real procfs/clone3/seccomp, the
static `-pthread` diagnostic, and `CAP_SYS_PTRACE` for positive inspection. The
locked child-channel control also requires `pidfd_getfd`. The mapped positive
additionally requires writable cgroup v2, user namespaces and
`CAP_SETFCAP`. Neither these fixtures nor a successful post-drop read would
qualify the actual systemd unit, approved compiler/runtime, proof or launch gate.

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
