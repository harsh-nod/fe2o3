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

Production operation exists only behind a separately installed, root-owned
`/usr/libexec/fe2o3-oci-operator`. That launcher has `verify`, `plan`, and
`preflight` commands, but accepts only a request ID. It loads all trust inputs
from fixed operator configuration. Running the repository launcher directly
fails closed, and the current host has no valid production installation.

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
    +-- protected Git objects --> ephemeral SourceSnapshot
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
oci_operator_config_schema_version  1
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
queue_trust_sha256                  SHA256
```

The production launcher accepts only `COMMAND --request-id 64_HEX`, then opens
`REQUEST_ID.tsv` within that fixed inbox by descriptor. The configuration pins
the external policy identity, relative path, exact byte size, SHA-256 digest,
expected UID/GID, immutable-file contract, request UID/GID, and queue trust
digest. Test CLI trust flags are visibly prefixed `--test-`, require test-domain
data, and can only produce `test-non-authoritative` state.

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
and installed executor descriptors must report `FS_IMMUTABLE_FL`. The fixed
launcher and executor must be root-owned, single-link, non-writable files at
their exact `/usr/libexec` paths. A candidate may name a profile ID but cannot
supply any protected operator anchor.

The repository contains no production policy or profile.

The protected profile binds all of the following:

- trust domain, target `gfx942`, and MI300X lane;
- absolute Docker client path, size, digest, canonical client/server version
  observation digest, and canonical daemon-info observation digest;
- exact Git executable and version, protected Git object directory, immutable
  source staging root, source file/byte/index limits, and export timeout;
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
races. The request digest is combined with the external queue-authorization
digest, policy anchor identity, policy digest, and profile digest to derive the
staging authorization identity.

The candidate checkout is never queried or mounted. In particular, the
executor never runs `git status`, so candidate repository configuration,
hooks, and fsmonitor commands are outside the execution path. It creates a
synthetic bare Git control directory with no candidate config, disables system
and global config, hooks, fsmonitor, replacement objects, optional locks, and
Git protocols, then reads the commit/tree/blob objects from the protected
object directory.

Only ASCII regular executable/non-executable Git blobs are admitted. Symlinks,
submodules, `.git`, unsupported modes, malformed paths, excess files, and
excess bytes fail closed. The resulting source tree and canonical request copy
are created inside a private staging lease named from the full authorization
digest plus 256 bits of operator randomness. The tree is made read-only and
contains no Git metadata. Open directory/file descriptors and device/inode
identities are retained and rechecked immediately before plan emission. Every
regular file is fsynced after its final read-only mode is applied. Populated
directories are then chmodded and fsynced bottom-up. The transient Git control
directory is removed before the final lease fsync. Any metadata or sync failure
rejects the snapshot with a controlled executor error.

Unknown or trailing request fields are rejected, including any closure, image,
runtime, environment, device, command, or isolation setting.

Every current subprocess starts in a new process group with closed inherited
descriptors, a fixed environment, a protected timeout, and independent stdout
and stderr ceilings. Overflow and timeout kill the process group. Reap polling
and pipe-thread joins have bounded grace windows; there is no final unbounded
`wait()`. If a process remains uninterruptible, the command returns a
controlled failure and leaves eventual reaping to the OS/Python subprocess
reaper. This is not a hard real-time guarantee: a kernel call or D-state task
can still outlive the command. Git object enumeration and blob payloads have
additional file-count, directory-count, index, per-file, and aggregate byte
limits.

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
`operator-v1.sha256` provision, fixed installed launcher/executor, production
policy/profile, or fixed inbox authorization. Production CLI trust arguments
do not exist. Therefore the repository script and current account cannot
authorize a production request even before Docker access is considered.

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
- No production image, policy, profile, key, receipt, or hardware result exists
  in this branch.
