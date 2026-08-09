# Hermetic OCI Evidence Executor V1

## Status

This is a fail-closed executor foundation. It does not yet authorize a parity
promotion.

`scripts/parity-oci-executor.py` currently implements three operations:

- `verify` authenticates a candidate request against a profile selected by a
  protected policy.
- `plan` emits the exact, argument-by-argument OCI container creation plan.
- `preflight` compares bounded host, Docker daemon, and loaded-image
  observations with the protected profile. These observations are not a full
  daemon/runtime closure.

There is intentionally no `run` operation and no execution receipt. The V2
promotion parser continues to accept only shell queue schema 3, whose closure
is `inert`, and continues to reject that closure for hardware promotion. No
field in an execution request can claim `execution_closure=verified`.

The missing execution and receipt state must not be replaced with a caller
assertion, a successful preflight, or a signed copy of candidate data.

## State Boundary

The implementation has separate immutable states:

```text
candidate Request
    |
    | protected policy selects exact profile path, size, and digest
    v
AuthorizedRequest
    +-- protected Git objects --> immutable SourceSnapshot
    +-- protected staging ----> retained OutputStage
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

Tabs separate fields. The policy and profile are read through a trusted-root
path walk that rejects symlinks, escapes, non-regular files, multiple hard
links, size changes, and digest changes. Production CI must extract both from
the protected base commit. A candidate may name a profile ID but cannot supply
the policy, profile bytes, profile digest, or trust domain.

The repository contains no production policy or profile.

The protected profile binds all of the following:

- trust domain, target `gfx942`, and MI300X lane;
- absolute Docker client path, size, digest, canonical client/server version
  observation digest, and canonical daemon-info observation digest;
- exact Git executable and version, protected Git object directory, immutable
  source staging root, source file/byte/index limits, and export timeout;
- operator UID/GID, protected source/output roots, and non-writable ownership
  policy; production roots and their parents must be root-owned;
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
requires Linux/amd64. JSON depth, node count, strings, descriptors, counts, and
numeric sizes are bounded. Preflight observes whether the Docker daemon reports
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
are created beneath an operator-owned staging root, made read-only, and mounted
without Git metadata. Open directory/file descriptors and device/inode
identities are retained and rechecked immediately before plan emission.

Unknown or trailing request fields are rejected, including any closure, image,
runtime, environment, device, command, or isolation setting.

Every current subprocess starts in a new process group with closed inherited
descriptors, a fixed environment, a protected timeout, and independent stdout
and stderr ceilings. Overflow and timeout kill the process group. Git object
enumeration and blob payloads have additional file-count, index, per-file, and
aggregate byte limits.

## Fixed OCI Plan

`plan` emits each argument as canonical hex-encoded ASCII. The protected plan
uses:

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
execute later. The future `run` operation must authorize, stage, preflight,
create, stream, and clean up in one process while retaining all safe
descriptors and the queue lock. Reopening printed paths in a later process is
not an accepted integration path.

The in-container output tmpfs is bounded working storage. It is not copied
after container exit. The protected entrypoint reserves stdout for
`fe2o3-artifact-stream-v1` and stderr for logs. Before any future execution,
the planner creates single-link `0600` artifact and stderr stream files beneath
an operator-owned durable output directory named by the full request ID, opens
them with `O_EXCL|O_NOFOLLOW`, retains their descriptors, and rechecks their
inodes. Before emitting a plan, it fsyncs the artifact stream, stderr stream,
execution directory, and protected staging root in that order; any failure
rejects the plan. A future runner must stream into those descriptors while the
container runs, fsync finalized content and metadata, and terminate the process
group at the protected byte or time limit.

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
```

## Current Operator Blocker

On the assigned `mi300x` host, `/usr/bin/docker` is installed, but the `harsh`
account cannot connect to `/var/run/docker.sock`; Docker reports permission
denied. `sudo` requires an interactive password. Unprivileged user namespaces
are unavailable, so rootless `runc`/Docker and bubblewrap network isolation
cannot provide an alternative under this account.

Do not add `harsh` to the general Docker group merely to unblock evidence:
Docker daemon access is effectively root authority. Provision a dedicated
evidence identity behind a root-owned, fixed-command service or an equivalently
restricted operator launcher. That launcher must expose only this protected
profile and must not accept arbitrary Docker arguments.

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
