# fe2o3 protected-service spawn

This package owns the shared root-to-service process transition used by
fe2o3 deployment coordinators, with distinct locked-service and proof-controller
types. It stages one caller-admitted executable and a
bounded, destination-unique descriptor table above every admitted target,
requires the root parent to own `SIGCHLD`, creates the direct child with
`clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)`, and falls back only on `ENOSYS` to
x86-64 `clone(CLONE_PIDFD | SIGCHLD)`. Both return the original pidfd atomically;
there is no numeric-PID reopen. The child performs only direct syscalls
in the post-clone child. Executable measurement, ownership, sealing, and static
ELF admission remain mandatory policy checks in the calling coordinator.

The calling thread blocks the complete kernel signal set before either clone.
The child retains that mask until every catchable disposition has been reset.
The parent owns the returned pidfd before restoring its exact prior mask, and
restores before any readiness wait. A restoration failure contains and reaps the
child before retrying restoration or failing closed. Unexpected wait errors
retain cleanup custody. This permits systemd's namespace restriction to remain
enabled for the locked-service role even when its filter rejects `clone3`;
other clone errors do not fall back.

Before reporting profile readiness, the child resets signals, binds
`PDEATHSIG=SIGKILL` to the exact parent, installs the dedicated UID/GID with no
supplementary groups, empties all capability sets including bounding, sets
`no_new_privs`, nondumpability, a zero core limit, and umask `077`, and reads
every property back. It re-arms and verifies `PDEATHSIG` after credential changes,
which otherwise clear that setting. The parent must independently validate the child and its
namespaces before sending the one-byte release token. Only then does the child
install the fixed descriptor table and execute the staged image with one fixed
argument and an empty environment.

`StagedProtectedServiceExecV1` preserves the locked securebits required by existing
issuer/supervisor and anchor services. `StagedProofControllerExecV1` is a separate
role requiring zero securebits and no seccomp, for trusted analyzer/Verus
controllers. Its calling-thread preflight rejects inherited filters or securebits
before cloning. Its profile is not convertible into a locked-service profile,
and its child owner is not convertible into a protected-service child owner.
Do not remove the compiler coordinator's filter to launch it: an independently
approved unfiltered root launch boundary is required.

The admitted static proof-controller image must use the shared secure entrypoint
and capture `ProofControllerProcessProfileV1` after exec, before proof resources
are opened. The profile observes the calling thread, retains namespace identities,
and revalidates only on its original PID/TID. The `test-support` static profile
fixture qualifies this startup contract but does not execute analyzer/Verus or
establish deployment approval.

The returned move-only child retains the atomic pidfd and exact reaping
ownership. Dropping it kills and synchronously reaps the child. The package
does not interpret service protocols, manifests, keys, paths, compiler data,
publication evidence, or GPU authority. Direct-child exit is neither descendant
containment nor proof of GPU settlement; both require separate ownership.
