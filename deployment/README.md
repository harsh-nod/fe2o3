# fe2o3 protected compiler-execution deployment

These files define the sole systemd deployment for the root coordinator,
protected supervisor, and local reference external anchor. They require systemd
253 or newer because the service uses ordered `OpenFile=` activation.

Install the files under their matching system directories:

- `systemd/*.service` and `systemd/*.socket` under the system unit directory;
- `sysusers.d/*.conf` under the system sysusers directory; and
- `tmpfiles.d/*.conf` under the system tmpfiles directory.

Run `systemd-sysusers` and `systemd-tmpfiles --create`, then install all seven
qualified static executables as root-owned, root-group, single-link mode `0555` files
at the paths named by the service unit. None may carry a file capability or
POSIX ACL. Build and install the root provisioning command with
`scripts/build-static-compiler-execution-provisioner.sh`.

For a complete installed fixture, build every qualified static image and the
exact deployment inputs as a read-only, hash-manifested bundle from a clean
checkout:

```console
$ scripts/build-static-compiler-execution-deployment.sh /tmp/fe2o3-deployment
```

The bundle contains all seven executables under `usr/libexec/fe2o3`, the exact
systemd, sysusers, and tmpfiles inputs, `BUILD-INFO`, a strict `SHA256SUMS`, and
`INSTALL-MANIFEST-V1`. The builder prints `bundle_path`, `manifest_sha256`, and
`git_commit`; the latter two are independent admission pins and must be
distributed outside the bundle. A static musl verifier admits the exact tree
through retained descriptors, rejects links, mount crossings, extra names,
metadata or content substitution, and retains the 13 content files as sealed
anonymous sources. See [deployment bundle
V1](../docs/compiler-execution-deployment-bundle-v1.md).

Compilation and CMake/CTest qualification run as the invoking non-root user.
Descriptor-relative atomic publication into an offline filesystem root is a
separate privileged phase:

```console
# install -d -o 0 -g 0 -m 0700 /var/lib/fe2o3/deployments-v1
# fe2o3-compiler-execution-deployment-install \
    /tmp/fe2o3-deployment <manifest_sha256> <git_commit> \
    /var/lib/fe2o3/deployments-v1
```

The static installer verifies the bundle into sealed anonymous custody, creates
and verifies one exact 12-directory/14-file sibling root, and publishes it with
one durable no-replace rename. Its final name is
`compiler-execution-v1-<manifest_sha256>`; an exact retry revalidates and
reacquires that root, while a conflicting root is not replaced. It never
reopens bundle content after admission. This is not a transaction over the
independent paths in a live `/`.

Build the pinned minimal systemd base used to qualify that offline root:

```console
$ scripts/build-compiler-execution-qualification-base.sh /tmp/fe2o3-base
```

The non-root builder admits exactly 71 package identities from the checked-in
version/architecture/SHA-256 lock and emits a deterministic SquashFS image plus
matching `BASE-INFO` and `SHA256SUMS`. The deployment crate can freshly
revalidate the installed tree and seal an independently digest-pinned base
image together with an empty root-owned qualification parent. It then creates
one exact descriptor-retained empty staging tree for base/root mounts and
disposable upper/work/run/state/evidence. Mount attachment, root composition,
and composed-root systemd preflight are implemented; isolated systemd boot and
service execution remain qualification gates. The transaction uses atomic loop
configuration and upstream Linux detached-mount APIs, but the current
unprivileged `mi300x` session cannot execute its root-only integration path. The fully static
`fe2o3-compiler-execution-qualification` image provides a read-only host probe
and the single exact qualification transaction for a privileged runner. Its
aggregate campaign requires one publication, 37 reacquisitions, two normal
runs, all 18 fixed mount/preflight/cleanup faults, post-fault bundle/base/lower
revalidation, and an empty qualification parent. See
[disposable-root V1](../docs/compiler-execution-disposable-root-v1.md).

With both the service and socket stopped, provision one nonzero policy
generation:

```console
# /usr/libexec/fe2o3/fe2o3-compiler-execution-provision 1
```

The command resolves the actual `fe2o3-compiler` and `fe2o3-anchor` UID/GID
values and first takes an exclusive nonblocking lifecycle lease on the exact
root-only `/var/lib/fe2o3/compiler-execution-lifecycle-v1` file through its
retained parent. The running coordinator derives that sibling from its existing
state-root descriptor and opens two additional independent shared leases before
key admission. The supervisor receives one at FD 12. The anchor helper receives
the other at FD 6 and transfers the same child lease to daemon FD 5. Each child
retains its own open file description and releases it only by last close, so a
coordinator crash cannot admit provisioning until both protected children have
exited. Provisioning and activation therefore
cannot overlap without adding another activation descriptor or conflicting with
the issuer's independent state-root singleton lock. The provisioner then
measures every fixed static image twice, creates missing 32-byte key seeds from
the kernel random source, derives the complete four-record graph, and publishes
each file with no-replace rename plus file and directory durability.
The lifecycle parent must reside on a local Linux filesystem with native
`flock(2)` open-file-description semantics; NFS and SMB/CIFS are not supported
by this reference profile.
It is idempotent for identical inputs. Existing mismatched bytes, modes,
ownership, links, ACLs, capabilities, image paths, or generations fail closed
and are never replaced. Lifecycle parent and lock-file replacement also fail
closed. Seeds use mode `0400`; public records use mode `0444`. A partial first
publication can be resumed with the same command and inputs.

Add only authorized Cargo users to the `fe2o3-compiler` group. Enabling
`fe2o3-compiler-execution.socket` starts the coordinator on first connection.
The coordinator independently validates the complete activation environment,
all 14 descriptors, every record relationship, both service identities, and
all filesystem policy before either child is released.

`StartLimitIntervalSec=0` keeps lifecycle-contention retries from exhausting
systemd start limits, while `FlushPending=no` retains queued socket traffic.
Readiness is still `Type=simple`; notification after a true worker-start barrier
and installed privileged crash qualification remain pending.

The bundled same-host external anchor remains qualification-only and carries no
production rollback authority. Production deployment still requires an
independently administered monotonic backend and separately protected key
custody.
