# fe2o3 compiler-execution deployment boundary

This crate owns the canonical install manifest, descriptor-relative bundle
admission, sealed source custody, and atomic offline-root publication boundary.
Verification requires an expected manifest SHA-256 and git commit from outside
the bundle. No digest or commit read from the bundle can authorize itself.

The verified value grants no installation, compiler, signing, publication,
loading, launch, execution, or GPU authority. It retains immutable sealed copies
of the manifest and exact admitted files. The effective-UID-zero installer
consumes that custody, creates the fixed root-owned 12-directory/14-file tree,
and publishes it beneath an exact mode-`0700` install parent with one durable
`RENAME_NOREPLACE`. The returned move-only installed-root value retains the
sealed deployment evidence so every installed object can be revalidated after
publication, while exposing identity metadata but no descriptor or service
authority.

The crate also prepares disposable-root qualification custody. It freshly
revalidates the installed root, admits an independently SHA-pinned SquashFS V4
base image through two reads into a sealed memfd, verifies the exact image
profile, and retains an empty root-owned qualification-parent descriptor. This
preparation grants no mount or execution authority. A second root-only
transaction creates and descriptor-retains the exact empty base/root,
upper/work, run/state, and evidence staging tree. Its fault campaign proves
every interrupted boundary restores an empty parent. The private-namespace
attachment implementation now uses an
atomic read-only autoclear loop device and detached upstream Linux mount APIs,
then rechecks the composed deployment projection against sealed custody. The
same transaction now launches the exact pinned-base loader and `systemd-nspawn`
through retained descriptors. Before boot it runs the shipped static
generation-1 provisioner inside the composed root and independently admits
its exact inventory, modes, identities, key-to-policy bindings, canonical
record graph, sealed runtime measurement, and five executable measurements.
It then opens and retains an exact nonblocking Unix `SOCK_SEQPACKET` connection,
admits the canonical listener path and root peer credentials, revalidates the
socket object, performs bounded pidfd shutdown, proves pathname removal, and
revalidates the installed lower before cleanup. The outer supervisor now attaches the
lease-blocked worker to one retained writable cgroup V2 child before release,
kills residual descendants through `cgroup.kill`, boundedly removes nested
machine cgroups, and withholds output until the scope is empty and removed. It
has not yet run under real host root, so live mount, boot, distinct-UID service,
and cgroup-teardown qualification remain open.
`fe2o3-compiler-execution-qualification probe` reports each host
prerequisite without mutation; its `run` command is the sole static
verify/install/prepare/stage/mount/preflight/provision/boot/revalidate/cleanup path. `fault-points`,
`fault`, and `campaign` expose one closed post-transition interruption set and
accept success only after exact install/staging-parent inventories are proven.
The three mutating commands now run in a dedicated parent-death-bound worker
under one descriptor-held exclusive lease over both parents. The supervisor
enforces fixed deadlines, handles `SIGTERM`, `SIGINT`, `SIGHUP`, and `SIGQUIT`,
kills and reaps before recovery, and publishes bounded anonymous worker output
only after both parents satisfy their exact postconditions.
`recover` removes zero or one bounded canonical qualification transaction.
`recover-install` separately admits an externally SHA-pinned final-root name,
preserves that root, and removes at most one bounded canonical installer staging
tree while rejecting every ambiguous sibling inventory. No privileged campaign
has run yet. See the
[disposable-root V1 contract](../../docs/compiler-execution-disposable-root-v1.md).
