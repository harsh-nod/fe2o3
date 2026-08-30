# fe2o3 protected-service spawn

This package owns the one root-to-protected-service process transition used by
fe2o3 deployment coordinators. It stages one caller-admitted executable and a
bounded, destination-unique descriptor table above every admitted target,
requires the root parent to own `SIGCHLD`, creates the direct child with
`clone3(CLONE_PIDFD | CLONE_CLEAR_SIGHAND)`, and performs only direct syscalls
in the post-clone child. Executable measurement, ownership, sealing, and static
ELF admission remain mandatory policy checks in the calling coordinator.

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
