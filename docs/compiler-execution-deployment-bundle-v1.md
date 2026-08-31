# Compiler-execution deployment bundle V1

Status: implemented source-bundle admission, atomic offline-root publication,
fresh installed-root revalidation, sealed disposable-root preparation, and an
exact fault-cleaned empty staging transaction. Host-root mount execution,
isolated boot, and distinct-UID systemd execution qualification remain open. The
private-namespace mount composition and composed-root systemd preflight are
implemented but not yet host-root qualified. A static host probe and sole
verify-to-preflight-to-cleanup harness are implemented. One closed 18-point
mount/preflight/cleanup fault set and aggregate publication/reacquisition
campaign use that same transaction; every admitted fault freshly revalidates
the caller-pinned bundle, base, and installed lower after cleanup. Successful
privileged execution is not yet claimed.

## Boundary

The deployment bundle is an inert, non-root build product. Its verifier admits
one exact source tree against a manifest SHA-256 and Git commit supplied outside
that tree, then copies the admitted content into sealed anonymous files. Neither
the manifest nor any other bundle file can select its own expected identity.

Successful admission grants no installation, compiler, signing, publication,
load, launch, execution, or GPU authority. The move-only verified value exposes
metadata but no file descriptor or pathname. The root-only installer consumes
that value and its sealed source custody directly; it never reopens the original
bundle by path after verification.

## Exact inventory

The bundle has seven mode-`0700` directories, including its root, and 14
single-link regular files. Every object has the same UID/GID as the retained
bundle root and carries no extended attributes. The canonical manifest binds 13
content files; `INSTALL-MANIFEST-V1` is instead bound by the caller-supplied
manifest SHA-256.

| Bundle source | Install destination | Mode |
| --- | --- | --- |
| `BUILD-INFO` | `/usr/share/fe2o3/compiler-execution/BUILD-INFO` | `0444` |
| `SHA256SUMS` | `/usr/share/fe2o3/compiler-execution/SHA256SUMS` | `0444` |
| `systemd/fe2o3-compiler-execution.service` | `/usr/lib/systemd/system/fe2o3-compiler-execution.service` | `0444` |
| `systemd/fe2o3-compiler-execution.socket` | `/usr/lib/systemd/system/fe2o3-compiler-execution.socket` | `0444` |
| `sysusers.d/fe2o3-compiler-execution.conf` | `/usr/lib/sysusers.d/fe2o3-compiler-execution.conf` | `0444` |
| `tmpfiles.d/fe2o3-compiler-execution.conf` | `/usr/lib/tmpfiles.d/fe2o3-compiler-execution.conf` | `0444` |
| `usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator` | `/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator` | `0555` |
| `usr/libexec/fe2o3/fe2o3-compiler-execution-issuer` | `/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer` | `0555` |
| `usr/libexec/fe2o3/fe2o3-compiler-execution-provision` | `/usr/libexec/fe2o3/fe2o3-compiler-execution-provision` | `0555` |
| `usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor` | `/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor` | `0555` |
| `usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper` | `/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper` | `0555` |
| `usr/libexec/fe2o3/fe2o3-external-anchor-service` | `/usr/libexec/fe2o3/fe2o3-external-anchor-service` | `0555` |
| `usr/libexec/fe2o3/fe2o3-static-preexec-launcher` | `/usr/libexec/fe2o3/fe2o3-static-preexec-launcher` | `0555` |

No extra root entry, nested entry, non-UTF-8 name, symlink, mount crossing,
device, FIFO, or socket is admitted.

## Canonical manifest

`INSTALL-MANIFEST-V1` is mode `0444`, at most 32 KiB, UTF-8, newline
terminated, and contains no carriage return or NUL. It has exactly this record
order:

```text
fe2o3-compiler-execution-install-manifest-v1
git_commit<TAB>40 lowercase hexadecimal characters
target<TAB>x86_64-unknown-linux-musl
entry_count<TAB>13
file<TAB>source<TAB>absolute install destination<TAB>mode<TAB>decimal length<TAB>sha256
```

There is one `file` record for every row above in table order. Names,
destinations, modes, bounds, decimal encoding, target, and record count are
fixed by V1 and cannot be selected by bundle bytes. `BUILD-INFO` must repeat the
same commit and target. `SHA256SUMS` must be the canonical sorted digest list
for every content file except itself.

## Admission algorithm

1. Parse the external manifest digest and commit before opening the bundle.
2. Open the root with `openat2`, rejecting symbolic and magic links.
3. Enumerate the exact root and nested inventories through retained directory
   descriptors. Open every child with `RESOLVE_BENEATH`,
   `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`, and `RESOLVE_NO_XDEV`.
4. Require exact type, mode, owner, link count, length bound, descriptor flags,
   and absence of extended attributes. Snapshot metadata around enumeration,
   two independent bounded reads, hashing, and canonical-path reopen.
5. Hash the raw manifest and compare it to the external digest before parsing
   its authority-bearing fields. Require its commit to equal the independently
   supplied commit.
6. Rehash every content file and cross-check `BUILD-INFO` and `SHA256SUMS`.
7. Copy each admitted content file into a mode-preserving anonymous memfd, add
   complete write/grow/shrink/seal locks, and revalidate every sealed object.

Any unsupported filesystem behavior, metadata race, malformed record, changed
pathname, or inconsistent identity fails closed.

## Atomic offline-root publication

The privileged installer requires effective UID 0 and a root-owned,
root-group, mode-`0700` install parent without extended attributes. It derives
the sole final name from the admitted manifest digest:

```text
compiler-execution-v1-<manifest_sha256>
```

The caller cannot choose the name or destination inventory. The installer
creates one random private sibling staging directory through the retained
parent descriptor, creates the fixed 12-directory hierarchy one component at a
time, and copies the manifest plus 13 content files only from sealed memfds.
Every file is create-new, root-owned, single-link, hashed while copying, assigned
its fixed mode, and synchronized. The complete 14-file root is then
descriptor-relatively reverified, including the manifest, `BUILD-INFO`, and
`SHA256SUMS`; its directories are synchronized bottom-up.

After revalidating the install-parent pathname against the retained descriptor,
the installer publishes the whole root with one
`renameat2(RENAME_NOREPLACE)` and synchronizes the parent. An existing final
name is reacquired only after complete identity and tree verification. A
conflicting tree is never replaced. Pre-publication failures remove only the
fixed inventory created beneath the retained staging descriptor. Failures after
the rename report `PublicationAmbiguous`; a retry either reacquires the complete
content-addressed root or rejects it.

This transaction publishes an **offline filesystem root**. It does not claim
atomic replacement of the independent `/usr` paths in a running host, which
Linux cannot provide as one filesystem transaction.

## Build and qualification

From a clean checkout:

```console
$ scripts/build-static-compiler-execution-deployment.sh /tmp/fe2o3-deployment
bundle_path=/tmp/fe2o3-deployment
manifest_sha256=<64 lowercase hexadecimal characters>
git_commit=<40 lowercase hexadecimal characters>
```

The builder qualifies all seven service images, runs the launcher's CTests,
builds loader-independent static musl manifest, verifier, and installer
executables, generates `SHA256SUMS` and `INSTALL-MANIFEST-V1`, and runs the
static verifier before publishing the output directory. The final two lines are
release inputs and must be distributed outside the bundle. The non-root builder
does not invoke the privileged installer.

Install a qualified bundle into a private offline-root parent with the two
out-of-band pins printed by the builder:

```console
# install -d -o 0 -g 0 -m 0700 /var/lib/fe2o3/deployments-v1
# fe2o3-compiler-execution-deployment-install \
    /tmp/fe2o3-deployment <manifest_sha256> <git_commit> \
    /var/lib/fe2o3/deployments-v1
installed_root_name=compiler-execution-v1-<manifest_sha256>
installed_file_count=14
installed_publication=created
```

Repeating the exact command reports `installed_publication=reacquired` after a
fresh complete verification.

The 16 crate tests cover wrong independent pins, malformed manifests, hostile
source and installed inventories, wrong parent policy, symlinks, hardlinks,
wrong modes, same-length substitutions, extended attributes, inconsistent
identity files, source-tree removal after verification, and install-parent
pathname replacement during copying and after publication. A 99-checkpoint
fault campaign covers every staging create, directory create, file
create/write/mode/sync, tree sync, rename, parent sync, and final verification
boundary. Before the rename the parent is empty after cleanup; after the rename
the only possible result is a complete, reacquirable 12-directory/14-file root.
Static ELF inspection rejects an interpreter, dynamic section, runtime
dependency, RPATH/RUNPATH, or undefined symbol.

The next boundary composes the retained installed root with the deterministic,
independently pinned base described by [disposable-root
V1](compiler-execution-disposable-root-v1.md), followed by real
root/distinct-UID systemd execution and crash qualification. Those gates must
exercise the static installer under host root and do not inherit authority from
the non-root test-only owner parameter.
