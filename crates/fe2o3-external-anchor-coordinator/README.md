# fe2o3 external-anchor coordinator

The V1 path in this root-runtime package owns one external-anchor child from measured input preparation through
termination and exactly-once reaping. It seals the helper and daemon for the deployment's dedicated
service identity, retains an exact service-owned state root and root-owned key template, launches
the helper with an atomic pidfd, and gates execution on independent child-profile and namespace
observation.

The root credential transition, profile gate, fixed-descriptor installation, `clone3` pidfd
creation, and exact reaping lifecycle are provided by the shared
`fe2o3-protected-service-spawn` owner used by protected deployment coordinators. This package adds
only the anchor-specific measured inputs, ready protocol, endpoint admission, and deployment
binding; it contains no second credential-drop or child-lifecycle implementation.

The coordinator accepts only the helper's canonical ready record with one `SCM_RIGHTS` endpoint,
then requires bootstrap close-on-exec EOF and continued pidfd liveness. It admits that endpoint
against the same process and deployment UID/GID before any supervisor transfer. The retained
admission and a separate reaping pidfd remain root-owned for the daemon's lifetime.
The occurrence also owns an independently opened shared lifecycle lease and
installs it at helper FD 6. This is not a duplicate of the root coordinator's
lease, so coordinator loss cannot release anchor-side provisioning exclusion.

A supervisor transfer is available only when the supplied canonical supervisor deployment and
issuer policy exactly match the anchor deployment retained with the live occurrence. The move-only
transfer carries the anchor deployment, supervisor deployment, and policy identities alongside the
already admitted endpoint and pidfd, allowing the root launcher to reject every one-axis manifest
substitution before installing inherited descriptors.

The coordinator grants no compiler, publication, loading, kernel-launch, or GPU authority. A
non-root test can exercise inert parsing and lifecycle pieces, but only the ignored root
qualification can authorize a real distinct-UID deployment claim.

`scripts/qualify-root-external-anchor-coordinator.sh <uid> <gid>` builds both measured static
images as the invoking user, crosses only the qualification invocation through `sudo`, performs a
signed durable exchange, requires exactly-once pidfd shutdown/reaping, and then requires a second
launch against the same root to report `Existing`.

## Native Preparation

`PreparedExternalAnchorOccurrenceV2/V3` implement preparation and revalidation
only, with actual same-family policy/supervisor capabilities borrowed on every
operation. They own native deployment, provisioning, root-owned key template,
lifecycle lease, measured sealed helper/daemon images and a pinned state root.
Exact root credentials and the preparing PID are checked. No V1 upgrade, raw
key/descriptor extractor, provider interface or native launch method is exposed.

Prepay `prepare_input_storage`, including both full source images and borrowed
contexts, on the original resource ledger. Reserve returned growth before
retaining the result; consumed input charges stay live. Operations restore entry
storage on success, failure and unwind without resetting work or history. Drop
closes owned descriptors; the caller then retires full `retained_storage`.
Quota queries include nested native checks and overlapping image storage; they
are logical bounds, not RSS or syscall-time guarantees.

Native staging/spawn, readiness/exec admission, managed lifetime and compiler
coordinator integration remain outstanding. Rootless tests do not qualify a
protected deployment or a GPU kernel.
