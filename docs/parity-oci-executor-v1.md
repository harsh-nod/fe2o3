# Hermetic OCI Evidence Executor V1

## Status

This is a fail-closed executor foundation. It does not yet authorize a parity
promotion.

The repository executor currently exposes three explicitly non-authoritative
test operations:

- `test-verify` validates test-domain inputs without granting plan authority.
- `test-plan` stages and validates an audit-only OCI creation plan, destroys
  both leases, and only then emits content digests and logical identifiers.
- `test-preflight` compares bounded host, Docker daemon, and loaded-image
  observations with the protected profile. These observations are not a full
  daemon/runtime closure.

Production operation exists only behind a separately installed, root-owned,
Linux-immutable static PIE `/usr/libexec/fe2o3-oci-operator`. The artifact must
be ELF `ET_DYN` with no `PT_INTERP` segment and no `DT_NEEDED` dependency. That
launcher has
`verify`, `plan`, and `preflight` commands, but accepts only a request ID. It
clears inherited process state before starting a fixed isolated Python
distribution and loads all trust inputs from fixed operator configuration. The
repository contains launcher source and non-authoritative tests, not an
installed production launcher, and the current host has no valid production
installation.

There is intentionally no `run` operation and no execution receipt. The V2
promotion parser continues to accept only shell queue schema 3, whose closure
is `inert`, and continues to reject that closure for hardware promotion. No
field in an execution request can claim `execution_closure=verified`.

The missing execution and receipt state must not be replaced with a caller
assertion, a successful preflight, or a signed copy of candidate data.

## State Boundary

The implementation has separate immutable states:

```text
fixed immutable operator config + external config digest
    |
    | pins policy identity/path/size/digest/owner and queue trust
    v
candidate Request selected by fixed-inbox request ID
    |
    | protected policy selects exact profile path, size, and digest
    v
AuthorizedRequest
    +-- protected queue authorization + canonical SHA-256 source manifest
    |       +-- authenticated loose Git closure --> ephemeral SourceSnapshot
    +-- protected staging ----> ephemeral OutputStage
    +-- bounded observations -> ObservedRuntimeRequest
    |
    | NOT IMPLEMENTED: execute fixed plan and stream bounded output
    v
ExecutedRequest
    |
    | NOT IMPLEMENTED: independently validate output and issue receipt
    v
ReceiptedExecution
```

Only a future protected promotion verifier may derive the parity value
`verified`, and only from a valid `ReceiptedExecution` bound to an authorized
profile in the protected base. `AuthorizedRequest`, `SourceSnapshot`,
`OutputStage`, and `ObservedRuntimeRequest` are not evidence classes and are
not promotable results.

## Protected Authorization

An OCI executor policy is canonical ASCII TSV:

```text
oci_executor_policy_schema_version  1
trust_domain                        production
profile_count                       1
profile  0000  mi300x-gfx942-v1  profiles/mi300x-gfx942-v1.tsv  SIZE  SHA256
```

Tabs separate fields. Policy contents are not their own trust anchor.
Production trust is not accepted from CLI arguments, environment variables,
the request, or the policy being opened. The fixed launcher reads the canonical
root-owned directory `/etc/fe2o3/oci-executor`, containing
`operator-v1.tsv` and the separate `operator-v1.sha256` digest provision. Every
directory component must be root-owned and non-group/world writable. Both files
must additionally be single-link regular files with the Linux immutable flag.
The provisioned digest must exactly bind the configuration before any policy
field is used.

The operator configuration schema is:

```text
oci_operator_config_schema_version  2
config_id                           mi300x-gfx942-production-v1
trusted_root                        /etc/fe2o3/oci-executor/trust
policy_path                         policy.tsv
policy_identity                     mi300x-production-policy-v1
policy_size                         SIZE
policy_sha256                       SHA256
trusted_owner_uid                   0
trusted_owner_gid                   0
trust_file_contract                 linux-immutable
inbox_root                          /var/lib/fe2o3/oci-inbox
inbox_owner_uid                     0
inbox_owner_gid                     0
request_owner_uid                   DEDICATED_UID
request_owner_gid                   DEDICATED_GID
queue_authorization_root            /var/lib/fe2o3/oci-authorizations
queue_authorization_owner_uid       0
queue_authorization_owner_gid       0
queue_trust_sha256                  SHA256
```

The production launcher accepts only `COMMAND --request-id 64_HEX`, then opens
`REQUEST_ID.tsv` within that fixed inbox by descriptor. The configuration pins
the external policy identity, relative path, exact byte size, SHA-256 digest,
expected UID/GID, immutable-file contract, request UID/GID, and queue trust
digest. It also pins the fixed queue-authorization root and its owner. Test CLI
trust flags are visibly prefixed `--test-`, require test-domain data, and can
only produce `test-non-authoritative` state.

The trusted root is opened once with `O_DIRECTORY|O_NOFOLLOW`, checked against
the externally configured owner and non-group/world-writable mode, and retained
while policy and profile descriptors are opened. Each relative parent is
walked by directory descriptor. Policy and profile files are opened once with
`O_NOFOLLOW`, must be regular single-link files with the expected owner and
safe mode, and are read through the same descriptor with a one-MiB ceiling.
Size, digest, and complete `dev/ino/mode/link/owner/size/mtime/ctime` metadata
must match before and after the read.

The test-only `descriptor-stable` contract provides descriptor pinning and
race detection; it does not establish filesystem immutability. A production
policy additionally requires the externally selected `linux-immutable`
contract, and the config provision, config, policy, profile, installed launcher,
installed interpreter, and installed executor descriptors must report
`FS_IMMUTABLE_FL`. The fixed launcher, interpreter, and executor must be
root-owned, single-link, executable, non-writable files at their exact
`/usr/libexec` paths. A candidate may name a profile ID but cannot supply any
protected operator anchor.

The repository contains no production policy or profile.

## Native Startup And Install Contract

The production launcher is built from
`scripts/parity-oci-operator-launcher.c` as a static PIE. A dynamic launcher is
not accepted because a caller-controlled loader environment such as
`LD_PRELOAD` would run before the launcher could clear it. The launcher verifies
its live `/proc/self/exe` identity against its fixed path and walks every fixed
path component with `O_NOFOLLOW`. Directories must be root-owned and
non-group/world-writable. The launcher, interpreter, and executor must be
root-owned, executable, single-link, non-group/world-writable, and
Linux-immutable.

Production execution is unconditionally disabled after fixed-binary
validation. A production build does not open a writable cgroup delegation or
execute the Python client. Supplying `/sys/fs/cgroup/fe2o3-oci-operator` cannot
enable it. The same identity cannot safely own the delegation and run the
executor: that executor could write its PID into an ancestor or sibling
`cgroup.procs`, escape the request leaf, and make that leaf appear empty.

Test builds may explicitly define
`FE2O3_TEST_ONLY_ALLOW_UNCONTAINED_EXECUTION=1`. That build calls `clearenv`,
installs only `HOME`, `LC_ALL`, `PATH`, and `TZ`, changes to `/`, sets umask
`077` and `no_new_privs`, replaces stdin with `/dev/null`, closes inherited
descriptors, and executes only:

```text
/usr/libexec/fe2o3-python/bin/python3 -I -S \
  /usr/libexec/fe2o3-oci-executor.py --operator-internal \
  COMMAND --request-id 64_LOWERCASE_HEX
```

The test-only native parent uses `PDEATHSIG`, a child subreaper, process-group
signaling, and bounded adopted-child draining. These controls are supplemental
tests, not a containment boundary. `setsid`, daemon double-forking, rapid
reforking, uninterruptible tasks, launcher death, and processes created by an
external daemon can escape or exceed their bounded cleanup windows. No result
from this test hook is production-authoritative.

A separate mechanism-only test build defines
`FE2O3_TEST_ONLY_CGROUP_MECHANISM=1`. It requires a real writable cgroup v2
root, creates a per-launch leaf, holds the child behind a socket gate, writes
and verifies the child PID in `cgroup.procs`, and exercises `cgroup.kill` plus
bounded `populated 0` observation. It never returns a zero production verdict.
This mode tests kernel and launcher mechanics only; it is not a containment
authority. The launcher and child intentionally have the same UID, and the
focused suite demonstrates that the child can self-migrate to the writable
parent cgroup. It does not authorize Docker use or claim daemon-created process
membership.

The Python executor independently requires the exact parent inode,
interpreter, script, cwd, environment, isolated/no-site/ignore-environment
flags, and Python search paths beneath `/usr/libexec/fe2o3-python`. Caller
`PATH`, `PYTHONPATH`, `PYTHONHOME`, user site, startup files, and site
customization are therefore outside the test interpreter startup path.

`scripts/build-parity-oci-operator.sh /absolute/staging/path` requires compiler
support for `-static-pie`, then rejects the result unless `readelf` reports ELF
`ET_DYN`, no `PT_INTERP`, and no `DT_NEEDED`. Production
installation must separately:

1. Provision a dedicated self-contained CPython tree at
   `/usr/libexec/fe2o3-python`, including its standard library and dynamic
   dependencies, from a reviewed digest manifest.
2. Recursively require root ownership, no symlinks or hardlinked regular files,
   and no group/world-writable components in that interpreter closure.
3. Install the reviewed executor as
   `/usr/libexec/fe2o3-oci-executor.py` and the static launcher as
   `/usr/libexec/fe2o3-oci-operator`, then apply and verify the Linux immutable
   flag to all three fixed executable files.
4. Provision the external immutable operator config digest, config, trust root,
   policy, profiles, and fixed inbox independently of candidate source.
5. Run the focused operator test and an operator-owned installation audit before
   enabling the fixed-command service identity.

The repository does not install files, set immutable flags, provision a Python
closure, or grant Docker access. A test build compiled with explicit test-only
paths and without immutable enforcement is non-authoritative and cannot pass
the Python production boundary.

The protected profile binds all of the following:

- trust domain, target `gfx942`, and MI300X lane;
- absolute Docker client path, size, digest, canonical client/server version
  observation digest, and canonical daemon-info observation digest;
- protected loose Git object directory, object format, object count/byte/tree
  depth limits, commit ancestry count/depth limits, immutable source staging
  root, and source file/byte/index limits;
- operator UID/GID, protected source/output roots, and non-writable ownership
  policy; these profile values do not authorize the policy/profile root;
- absolute OCI layout path and exact bounded `index.json` size/digest;
- exact image reference by manifest digest;
- exact OCI manifest, config, and every ordered layer digest and size;
- ordered rootfs diff IDs and an image config with no inherited environment,
  declared volumes, or healthcheck;
- one fixed absolute entrypoint and fixed command vector;
- a complete, ordered environment including deterministic `HOSTNAME`, `HOME`,
  `LC_ALL`, and `PATH`; `HIP_VISIBLE_DEVICES` and `ROCR_VISIBLE_DEVICES` must
  both equal the protected GPU unique ID;
- fixed source, request, output, and temporary mount points;
- output, temporary-storage, shared-memory, log, memory, PID, and CPU ceilings;
- container UID, GID, and one supplemental render-group GID;
- disabled network, read-only root, all capabilities dropped,
  `no-new-privileges`, and an exact protected seccomp policy;
- exactly `/dev/kfd` and one `/dev/dri/renderD*` character device, including
  major, minor, access, and group identity;
- machine ID digest, kernel release, kernel-notes digest, AMDGPU module path
  and digest, GPU PCI slot/ID, and GPU unique ID.

The OCI validator follows `index -> manifest -> config/layers`, validates every
referenced blob by size and SHA-256, validates the config rootfs diff IDs, and
requires Linux/amd64. The layout marker, index, manifest, and config are each
opened once through an `O_NOFOLLOW|O_NONBLOCK` descriptor walk. Their regular
file type, owner, mode, link count, size, and complete identity are checked
before and after a bounded `MAX+1` read; FIFOs, devices, symlinks, oversized
metadata, and replacement races fail closed. JSON depth, node count, strings,
descriptors, counts, and numeric sizes are bounded. Preflight observes whether
the Docker daemon reports
the same config digest, repository manifest digest, platform, diff IDs, and
empty inherited environment, no declared volumes, and no healthcheck. It does
not claim to measure the complete daemon,
containerd, `runc`, service configuration, or kernel execution closure.

## Candidate Request

The candidate request is deliberately small:

```text
oci_execution_request_schema_version  1
request_id                            UNIQUE_64_HEX
profile_id                            mi300x-gfx942-v1
source_commit                         COMMIT
source_tree                           TREE
job_id                                row-04-hardware
job_path                              scripts/evidence/jobs/row-04.sh
job_sha256                            SHA256
```

The request path is opened once with `O_NOFOLLOW|O_NONBLOCK`. The descriptor
must identify a bounded regular single-link file with the externally expected
UID/GID and a non-group/world-writable mode. The parser reads at most
`MAX_FILE_BYTES + 1` through that descriptor and rejects metadata changes
during the read, FIFOs, devices, links, oversized files, and replacement
races. The request cannot name its source manifest. Production resolves exactly
`REQUEST_ID.authorization.tsv` and `REQUEST_ID.source.tsv` beneath the fixed
operator-owned queue-authorization root. Both are opened by retained
`O_NOFOLLOW|O_NONBLOCK` descriptors and must satisfy the configured owner,
mode, link, immutability, bounded-size, and stable-identity contracts. The
authorization record binds the protected queue trust digest, request ID and
SHA-256, source commit/tree, manifest size and SHA-256, and source-root SHA-256.
Missing production authorization or manifest data fails closed.

The canonical source manifest contains the commit and tree, exact file count,
and one sorted record for every exported regular file: canonical ASCII relative
path, exact Git mode, byte length, and SHA-256. It also binds the total source
bytes. The source-root digest is SHA-256 over a domain separator and the
canonical manifest body. Before any staging write, the executor independently
reconstructs the complete manifest from parsed blob bytes and uses constant-time
full-digest comparisons against the protected authorization. Git SHA-1 object
identity alone cannot authorize changed source bytes.

The request digest, source manifest/root digests, external queue trust digest,
policy anchor identity, policy digest, and profile digest are combined to derive
the staging authorization identity.

The candidate checkout is never queried or mounted, and no Git executable is
invoked. The profile names one exact operator-owned `objects` directory. The
executor opens the repository and object store by retained `O_NOFOLLOW`
descriptors, rejects object alternates, replace refs, grafts, promisor or
partial-clone configuration, and every pack/index/commit-graph entry, and
supports only canonical SHA-1 loose objects. Environment-based object,
alternate, replace, and graft indirection is also rejected.

The requested commit, every declared parent commit, its exact tree, every
reachable subtree, and every reachable blob are parsed structurally. Commit
headers require one first lowercase tree ID, contiguous unique lowercase parent
IDs, ordered and structurally valid author/committer identities, bounded
optional headers and continuations, and canonical header termination. Every
parent object must be present and be a valid commit within the protected
ancestry count and depth limits. Each loose object must be a regular
single-link file with the protected owner and mode. Its bounded zlib stream,
canonical kind/size header, payload length, and recomputed object ID must all
agree with the name referenced by its parent. Commit/tree linkage, Git tree
ordering, duplicate names, cycles, depth, object count, compressed and expanded
bytes, tree index bytes, source files, directories, and source bytes are all
bounded. Packed stores are deliberately unsupported rather than trusted.

Only canonical ASCII relative paths are admitted; regular blob contents are
arbitrary bounded bytes. Symlinks, submodules, `.git`, unsupported modes,
malformed paths, excess files, and excess bytes fail closed. The resulting
source tree, protected source manifest, and canonical request copy
are created inside a private staging lease named from the full authorization
digest plus 256 bits of operator randomness. The tree is made read-only and
contains no Git metadata. Open directory/file descriptors and device/inode
identities are retained and rechecked immediately before plan emission. Every
regular file is fsynced after its final read-only mode is applied. Populated
directories are then chmodded and fsynced bottom-up. Any metadata or sync
failure rejects the snapshot with a controlled executor error.

Unknown or trailing request fields are rejected, including any closure, image,
runtime, environment, device, command, or isolation setting.

Every current test subprocess starts in a new process group with closed inherited
descriptors, a fixed environment, a protected timeout, and independent stdout
and stderr ceilings. Overflow and timeout kill the process group. Reap polling
and pipe-thread joins have bounded grace windows; there is no final unbounded
`wait()`. If a process remains uninterruptible, the command returns a
controlled failure and leaves eventual reaping to the OS/Python subprocess
reaper. This is not containment: a kernel call, detached descendant, or task
created by the Docker daemon can outlive the command. There is still no
production container runtime invocation.

All descriptor closes, path resolution, staging finalization, and filesystem
cleanup failures are normalized to controlled executor errors. Grouped closes
attempt every descriptor, and a cleanup failure is appended to rather than
replacing the primary failure. The command entrypoints also contain residual
`OSError` as a single bounded stderr line without a traceback.

## Fixed OCI Plan

The protected plan constructs the following Docker arguments internally:

- `--network none`;
- `--pull=never` and `--no-healthcheck`;
- `--cgroupns=private`;
- `--read-only`;
- `--cap-drop ALL`;
- `--security-opt no-new-privileges=true`;
- the digest-bound protected seccomp profile;
- private IPC, PID, and UTS namespaces;
- `--log-driver none`;
- fixed PID, memory, shared-memory, and CPU limits;
- fixed non-root UID/GID and render supplemental group;
- only the complete protected environment;
- a recursively read-only exact source bind;
- a read-only request bind;
- bounded output and temporary tmpfs mounts;
- exactly the protected KFD and render devices;
- the protected entrypoint, command, and manifest-digest image reference.

Container names contain the complete 256-bit request ID. Ownership labels bind
the full request ID, profile digest, and source tree. A future executor must
remove only the exact container ID it created after checking all labels; it
must never remove a pre-existing name collision.

The runtime control socket is never mounted. The plan never uses
`--privileged`, host networking, host IPC/PID/UTS namespaces, ambient
capabilities, a writable root, or a host output bind.

The emitted plan is an audit artifact, not authority for another process to
execute later. It contains only logical IDs, bounded counts, content digests,
and a domain-separated digest of the complete internal Docker argument vector.
It contains no Docker arguments, container name, staging path, mount source,
lease nonce, or output path. The future `run` operation must authorize, stage,
preflight, create, stream, and clean up in one process while retaining all safe
descriptors and the queue lock. Reopening plan data in a later process is not an
accepted integration path.

`plan` first removes both exact source and output leases, fsyncs their parent
roots, and verifies by descriptor that both lease names are absent. Only after
all of those operations succeed does it perform one bounded stdout write. A
cleanup, close, verification, or output failure emits no successful plan.

Source and output staging roots are independently locked while a lease exists.
Each dedicated root has a protected entry quota, and source bytes, files, and
directories are bounded. Reuse, a pre-existing random-name collision, lock
contention, or quota exhaustion fails closed and never grants cleanup authority
over the pre-existing entry. Cleanup walks by descriptor without following
links, restores private directory modes, removes entries within a traversal
budget, and fsyncs each changed directory and the staging root. A crash may
leave a random authorization-bound partial lease; it cannot be reused, counts
against the bounded stale-entry quota, and requires operator cleanup.

The in-container output tmpfs is bounded working storage. It is not copied
after container exit. The protected entrypoint reserves stdout for
`fe2o3-artifact-stream-v1` and stderr for logs. For audit validation, the
planner creates single-link `0600` artifact and stderr stream files inside its
ephemeral output lease, opens them with `O_EXCL|O_NOFOLLOW`, retains their
descriptors, rechecks their inodes, and fsyncs files and parent directories.
It then removes them without producing evidence. A future runner must retain
its own live lease and stream into those descriptors while the container runs,
fsync finalized content and metadata, and terminate the process group at the
protected byte or time limit.

## Required Execution And Receipt Work

After operator access exists, a separate reviewed change must implement this
fixed lifecycle:

1. Hold the production MI300X queue lock through authorization, execution,
   output capture, cleanup, and result signing.
2. Re-run protected authorization and runtime preflight immediately before
   container creation.
3. Create the container from the exact emitted plan.
4. Start with attachment while Docker logging remains disabled. Decode the
   framed artifact stdout and log stderr concurrently into the already-opened
   durable staging descriptors. Terminate the complete process group on stream
   overflow, malformed framing, or timeout.
5. Require a normal exit and exact zero status.
6. Finalize the stream only after validating every frame, declared size,
   digest, total byte count, and canonical output manifest. Never use a
   post-stop tmpfs copy or candidate-selected host output path.
7. Reject unexpected artifact names, duplicate paths, oversized frames, excess
   total bytes, and a malformed or incomplete output manifest.
8. Recompute every artifact and log size and digest outside the container.
9. Inspect the exact created container ID and ownership labels, remove that ID,
   and verify removal before signing anything. Name collisions are failures,
   not cleanup authority.
10. Produce a canonical receipt binding the protected profile digest, request
    digest, source commit/tree, full plan digest, OCI manifest/config/layers,
    runtime identities, host/GPU/driver identities, timeout, exit status, log,
    output manifest, and every artifact.

A runtime failure, cleanup failure, output validation failure, or missing
receipt must produce no hardware result signature.

## Promotion Integration Hook

The promotion parser should be changed only after the lifecycle above is
implemented and tested on a provisioned runner. The reviewed schema should:

1. Add profile ID/digest, plan digest, and receipt path/digest to the hardware
   result and signed queue job.
2. Load the executor policy and profile exclusively from the protected base.
3. Recompute authorization and validate the complete receipt against the
   signed queue, result, source tree, and archived outputs.
4. Derive `execution_closure=verified` internally only for the recognized
   receipt schema and protected profile. The candidate result must not contain
   a self-asserted closure value.
5. Continue rejecting shell queues, unknown profiles, test-domain profiles,
   preflight-only records, and every older schema for promotable hardware.

Until then, central parser wiring intentionally waits. The generic
`parity-evidence` lane runs the executor tests through:

```sh
scripts/tests/parity-oci-executor.sh
scripts/tests/parity-oci-operator.sh
```

## Current Operator Blocker

On the assigned `mi300x` host, `/usr/bin/docker` is installed, but the `harsh`
account cannot connect to `/var/run/docker.sock`; Docker reports permission
denied. `sudo` requires an interactive password. Unprivileged user namespaces
are unavailable, so rootless `runc`/Docker and bubblewrap network isolation
cannot provide an alternative under this account.

The host also has no immutable root-owned `operator-v1.tsv`, external
`operator-v1.sha256` provision, static-PIE installed launcher, self-contained
fixed Python closure, installed executor, production policy/profile, fixed
inbox, or fixed queue source authorization. Production CLI trust arguments do
not exist. Therefore the
repository source and current account cannot authorize a production request
even before Docker access is considered.

The host is on cgroup v2, but the current SSH session is in a root-owned
`session-*.scope`. Neither that scope nor its parent is delegated writable to
`harsh`, and `/sys/fs/cgroup/fe2o3-oci-operator` does not exist. Docker runs in
the separate delegated `/system.slice/docker.service` cgroup. Therefore a
launcher-owned process group or subreaper cannot contain Docker-created
processes, and migrating only the Docker client would be insufficient.

Production remains disabled until one reviewed change implements and tests all
of the following as one boundary:

1. Run a privilege-separated supervisor that exclusively owns the delegated
   cgroup subtree. The executor and container identities must differ from the
   supervisor and must be unable to write ancestor or sibling `cgroup.procs`,
   create cgroups, change controller state, or migrate themselves out of the
   request leaf.
2. Create one collision-resistant leaf per request before any executor code
   runs; hold the child behind a synchronization gate, have only the supervisor
   migrate it through `cgroup.procs`, and verify membership before `execve`.
3. Run the supervisor as a protected systemd service with
   `KillMode=control-group`. Abrupt supervisor death must trigger whole-service
   cleanup, and reconciliation must withhold evidence until every request leaf
   is verified empty and removed.
4. Pass the request leaf as Docker's protected cgroup parent, retain the exact
   container ID, and verify the daemon-reported container PID is in that leaf
   before workload start. Killing only the Docker client is insufficient.
5. On every timeout, error, signal, normal return, client disconnect, or daemon
   restart, issue daemon-side kill and removal for the exact labeled container,
   write `1` to the leaf's `cgroup.kill`, require bounded `cgroup.events`
   observation of `populated 0`, and remove the empty leaf. Any failed action or
   observation is a hard failure and cannot produce evidence.
6. Run adversarial self-migration, `setsid`, double-fork, rapid-refork,
   launcher-death, Docker-client-death, and Docker-daemon-restart tests against
   the provisioned privilege boundary. A same-UID `Delegate=yes` scope or fake
   filesystem fixture cannot establish this claim.

Ordinary SSH sessions can test the real refusal path. An ephemeral
`systemd-run --user --scope -p Delegate=yes` scope provides a real writable
test cgroup, so the focused suite exercises pre-exec migration, direct children,
`setsid`, double-fork, bounded fork pressure, continuous rapid reforking,
`cgroup.kill`, `populated 0`, and leaf removal there. The suite also proves the
limitation: a same-UID child can write itself into the parent cgroup. This is a
mechanism-only fixture, not a containment or production fixture. It has no
Docker socket authority and cannot bind processes created under
`/system.slice/docker.service`. Production builds enable neither test hook and
remain unconditionally disabled even when pointed at that writable scope.

Do not add `harsh` to the general Docker group merely to unblock evidence:
Docker daemon access is effectively root authority. Provision a dedicated
evidence identity behind a root-owned, fixed-command service or an equivalently
restricted operator launcher. The implemented launcher boundary accepts only a
fixed command and request ID; deployment must provision and review its immutable
configuration, policy, profile, inbox, and installed binaries. It must not
accept arbitrary Docker arguments.

A Landlock plus seccomp launcher is a possible separate design, but it needs a
new adversarial review. It must bind the launcher and all dynamic libraries,
use a digest-manifested read-only toolchain root, deny network syscalls with
seccomp, close inherited file descriptors, enforce the same device and writable
path limits, and record the exact kernel/Landlock ABI. It must not reuse the OCI
profile identity or claim OCI-equivalent closure without that review.

## Residual Risks

- `/dev/kfd` is a global control device. Exposing one render node narrows usable
  GPU access, but this must be validated against the installed ROCr/HIP stack
  and partition topology before production authorization.
- A Docker daemon is a large trusted computing base. Current preflight data is
  observational. The daemon binary, service configuration, containerd, `runc`,
  dynamic libraries, storage driver, seccomp/AppArmor state, and kernel inputs
  still need a complete measured closure before promotion can derive
  `verified`.
- OCI layout validation and Docker image inspection are separate observations.
  The future executor must prevent image replacement between preflight and
  creation and record the created container's image identity.
- The seccomp profile in the protected profile still requires syscall-level
  review against the exact ROCm workload.
- The launcher source constrains Python startup, but production remains disabled
  until the complete fixed Python standard-library/dynamic-library closure is
  digest-manifested, installed, recursively audited, and protected from the
  candidate identity.
- No production image, policy, profile, key, receipt, or hardware result exists
  in this branch.
