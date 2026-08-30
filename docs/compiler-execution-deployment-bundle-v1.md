# Compiler-execution deployment bundle V1

Status: implemented source-bundle admission. Atomic privileged installation is
not implemented.

## Boundary

The deployment bundle is an inert, non-root build product. Its verifier admits
one exact source tree against a manifest SHA-256 and Git commit supplied outside
that tree, then copies the admitted content into sealed anonymous files. Neither
the manifest nor any other bundle file can select its own expected identity.

Successful admission grants no installation, compiler, signing, publication,
load, launch, execution, or GPU authority. The move-only verified value exposes
metadata but no file descriptor or pathname. A future privileged installer must
consume its sealed source custody directly; it must not reopen the original
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

## Build and qualification

From a clean checkout:

```console
$ scripts/build-static-compiler-execution-deployment.sh /tmp/fe2o3-deployment
bundle_path=/tmp/fe2o3-deployment
manifest_sha256=<64 lowercase hexadecimal characters>
git_commit=<40 lowercase hexadecimal characters>
```

The builder qualifies all seven service images, runs the launcher's CTests,
builds loader-independent static musl manifest and verifier executables,
generates `SHA256SUMS` and `INSTALL-MANIFEST-V1`, and runs the static verifier
before publishing the output directory. The final two lines are release inputs
and must be distributed outside the bundle.

The crate tests cover wrong independent pins, malformed and trailing manifest
records, extra and non-UTF-8 entries, symlink and hardlink replacement, wrong
modes, same-length content replacement, extended attributes, inconsistent
`BUILD-INFO` and `SHA256SUMS`, root symlinks, and custody after the source tree
has been removed. Static ELF inspection rejects an interpreter, dynamic
section, runtime dependency, RPATH/RUNPATH, or undefined symbol.

The next boundary is a descriptor-relative privileged installer that creates a
new install root, copies only from sealed admitted sources, validates the final
tree, and atomically publishes or rolls it back. Root/distinct-UID systemd
qualification follows that installer.
