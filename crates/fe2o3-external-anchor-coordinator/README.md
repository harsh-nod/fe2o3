# fe2o3 external-anchor coordinator

This root-runtime package owns one external-anchor child from measured input preparation through
termination and exactly-once reaping. It seals the helper and daemon for the deployment's dedicated
service identity, retains an exact service-owned state root and root-owned key template, launches
the helper with an atomic pidfd, and gates execution on independent child-profile and namespace
observation.

The coordinator accepts only the helper's canonical ready record with one `SCM_RIGHTS` endpoint,
then requires bootstrap close-on-exec EOF and continued pidfd liveness. It admits that endpoint
against the same process and deployment UID/GID before any supervisor transfer. The retained
admission and a separate reaping pidfd remain root-owned for the daemon's lifetime.

The coordinator grants no compiler, publication, loading, kernel-launch, or GPU authority. A
non-root test can exercise inert parsing and lifecycle pieces, but only the ignored root
qualification can authorize a real distinct-UID deployment claim.

`scripts/qualify-root-external-anchor-coordinator.sh <uid> <gid>` builds both measured static
images as the invoking user, crosses only the qualification invocation through `sudo`, performs a
signed durable exchange, requires exactly-once pidfd shutdown/reaping, and then requires a second
launch against the same root to report `Existing`.
