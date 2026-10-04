# fe2o3 protected-service spawn

This package owns the one root-to-protected-service process transition used by
fe2o3 deployment coordinators. It stages one caller-admitted executable and a
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
enabled even when its filter rejects `clone3`; other clone errors do not fall back.

Before reporting profile readiness, the child resets signals, binds
`PDEATHSIG=SIGKILL` to the exact parent, installs the dedicated UID/GID with no
supplementary groups, empties and locks all capability paths, sets
`no_new_privs`, nondumpability, a zero core limit, and umask `077`, and reads
every property back. The parent must independently validate the child and its
namespaces before sending the one-byte release token. Only then does the child
install the fixed descriptor table and execute the staged image with one fixed
argument and an empty environment.

The returned move-only child retains the atomic pidfd and exact reaping
ownership. Dropping it kills and synchronously reaps the child. The package
does not interpret service protocols, manifests, keys, paths, compiler data,
publication evidence, or GPU authority.
