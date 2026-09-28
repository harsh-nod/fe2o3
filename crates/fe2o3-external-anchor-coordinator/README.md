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

## Native Coordinator

`PreparedExternalAnchorOccurrenceV2/V3` implement preparation, revalidation and
consuming launch, with actual same-family policy/supervisor capabilities borrowed on every
operation. They own native deployment, provisioning, root-owned key template,
lifecycle lease, measured sealed helper/daemon images and a pinned state root.
Exact root credentials and the preparing PID are checked. No V1 upgrade, raw
key/descriptor extractor or provider interface is exposed.
Preparation and every revalidation join the lifecycle lease to the actual state
root, rather than accepting two independently valid objects.

Prepay `prepare_input_storage`, including both full source images and borrowed
contexts, on the original resource ledger. Reserve returned growth before
retaining the result; consumed input charges stay live. Operations restore entry
storage on success, failure and unwind without resetting work or history. Drop
closes owned descriptors; the caller then retires full `retained_storage`.
Quota queries include nested native checks and overlapping image storage; they
are logical bounds, not RSS or syscall-time guarantees.

Before the first child, call `retain_cleanup_guard` with the same actual contexts
and the original empty cleanup pool. It transfers a root-bound lease alias into
persistent cleanup custody. Consuming launch validates a clone of that installed
guard against the actual lease before spawning; an unrelated valid guard refuses.
`cleanup_guard_quota` describes request work and extra peak. The cleanup account
independently funds guard installation and cloning, without renewing its limits.

Launch derives full source charges, stages the exact nine native inputs, and
validates final Files through the actual native owners before calling the shared
spawn primitive. The original ledger funds staging, child setup and all readiness
attempts; the shared cleanup controller funds deferred cleanup. Profile and
namespace checks gate release. Canonical readiness, exact endpoint transfer,
exec EOF, live pidfd and native endpoint admission precede a managed result.
The V1 and native paths use the finite readiness scheduler in the shared spawn
crate; this coordinator keeps only anchor wire decoding. Late successful reads
and unbounded gate retries cannot bypass deadlines. Terminal record credentials
distinguish empty records from EOF and grant no service identity authority.

`RootManagedExternalAnchorV2/V3` retain preparation, child and endpoint custody.
Launch returns growth above the consumed prepared owner; retain its existing
reservation and reserve the growth. Continuity requires actual contexts again.
Cancellation/Drop take one prepaid cleanup step and may defer or quarantine;
they do not guarantee eventual reaping or artifact-lock release.

`try_clone_for_supervisor` returns nominal V2/V3 endpoint/pidfd custody only after
rechecking the managed occurrence and actual same-family policy and supervisor
capabilities. Its storage receipt is the full new transfer charge, not growth.
Descriptor extraction is consuming and metered; retire only the envelope charge
while keeping both descriptors charged. Before exec, validate the final staged
Files with `validate_supervisor_transfer` on the retained managed owner and the
same actual contexts. Transfer metadata alone does not authorize a receiver.

Compiler coordinator integration remains pending.
Adapter-level post-clone failure/unwind coverage and genuine protected startup
are still validation gaps. Rootless tests do not qualify a protected deployment
or a GPU kernel. See the
[transfer checkpoint](../../docs/evidence/conditional-native-transfer-20260926.md).
