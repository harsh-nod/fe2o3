# fe2o3 compiler-execution supervisor

This package owns the protected process boundary around the static
compiler-execution issuer. Program admission authenticates the provisioned
static launcher and issuer before either can enter authority-bearing custody.

Both source images are read through stable file descriptions, checked against
exact SHA-256 and length measurements, validated as loader-independent x86-64
ELF images, copied into distinct anonymous mode-0555 memfds, sealed with
`WRITE`, `GROW`, `SHRINK`, `EXEC`, and `SEAL`, reopened read-only, and measured
again. The issuer must match the exact executable and runtime measurements in
the sealed caller policy. The launcher measurement belongs to trusted service
provisioning and is never accepted in a per-launch request.

The admitted program is move-only and exposes no descriptor. A second move-only
state now binds that exact program and policy to the canonical signing-key
capability, a dedicated non-root UID/GID profile, and a retained service-owned
root. Root admission requires a close-on-exec read-only directory descriptor,
exact owner UID and GID, mode `0700`, nonzero link count, and no file capability
or POSIX access/default ACL. Program, policy, key, service identity, and root
security metadata are all revalidated together. The key, root descriptor,
source paths, and signing operation remain inaccessible.

The credential profile fixes the eventual child state: equal
real/effective/saved/filesystem IDs, no supplementary groups, empty capability
sets including bounding and ambient sets, locked `NOROOT`, locked set-ID fixup
and ambient-capability prevention, `no_new_privs`, nondumpability, zero core
limit, umask `077`, and unchanged supervisor namespaces. This checkpoint binds
the configured effective UID/GID but deliberately exposes no process-launch
method and does not claim the complete child profile yet. The next checkpoint
must enforce and observe it while consuming the rustc handoff, constructing the
exact static pre-exec manifest, launching with `clone3(CLONE_PIDFD)`, and owning
readiness, cancellation, restart, and exactly-once reaping.
