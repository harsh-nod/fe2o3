# Compiler-execution disposable root V1

Status: deterministic base construction, caller-pinned SquashFS admission,
sealed preparation custody, and the exact empty staging transaction are
implemented. Private-namespace mount composition is implemented but has not yet
run under host root. Its static probe, exact high-level mount harness, closed
mount fault set, and aggregate campaign are implemented. Root execution,
isolated systemd boot, distinct-UID execution, and lifecycle fault
qualification remain open.

## Purpose

The disposable root qualifies the exact offline deployment root without
installing it into a running host `/`. It combines three independently checked
inputs:

1. an atomically published compiler-execution deployment root;
2. a minimal, deterministic Ubuntu 24.04 amd64 systemd base image; and
3. an empty root-owned qualification parent used for descriptor-relative
   staging and cleanup.

Preparation grants no mount, namespace, service, compiler, signing,
publication, GPU, or execution authority. The public move-only value exposes
only immutable identity metadata and no descriptor.

## Base bundle

Run the non-root builder from a clean Ubuntu 24.04 amd64 checkout with
`mksquashfs` 4.6.1:

```console
$ scripts/build-compiler-execution-qualification-base.sh /tmp/fe2o3-base
base_bundle_path=/tmp/fe2o3-base
base_image_sha256=<64 lowercase hexadecimal characters>
base_image_bytes=<positive decimal length>
git_commit=<40 lowercase hexadecimal characters>
source_date_epoch=<commit timestamp>
package_count=71
```

The mode-`0700` output has exactly three mode-`0444`, single-link files:

- `qualification-base-v1.squashfs`;
- `BASE-INFO`; and
- `SHA256SUMS`.

The builder resolves the important recursive dependency closure of 12 fixed
root packages and requires its package-name set to equal the checked-in
71-record lock. It downloads every exact version, validates package name,
architecture, and SHA-256 before extraction, runs no maintainer scripts, and
replaces account and machine identity files with canonical content. The image
contains the repository-owned `fe2o3-qualification.target` and binds the Git
commit, commit timestamp, tool version, and complete package lock in
`BASE-INFO`.

SquashFS construction uses one processor, zstd, 128 KiB blocks, no xattrs, no
exports, all-root ownership, the Git commit timestamp for every object, and
reproducible ordering and timestamps. Publication is one directory rename only
after image-profile and digest checks succeed.

The lock pins package bytes, not repository availability. The builder fails
closed if the configured Ubuntu repositories no longer serve an exact version
or if their dependency closure changes. A release archive must retain or mirror
the 71 digest-pinned `.deb` inputs to guarantee rebuild availability over the
release lifetime; accepting a newer package under the old identity is never an
allowed fallback.

## Admission and custody

`prepare_compiler_execution_qualification_v1` requires effective UID zero and
four inputs:

- a move-only installed deployment retaining its sealed source bundle;
- an absolute base-image pathname;
- the expected base-image SHA-256 supplied outside that image; and
- an absolute empty root-owned, root-group, mode-`0700` qualification parent.

Preparation freshly revalidates every installed deployment object against the
retained sealed sources. It accepts only a mode-`0444`, single-link regular
base image without xattrs, bounded to 512 MiB. Builder ownership may be
non-root because the file is metadata-snapshotted, read twice, byte-compared,
hashed against the independent pin, copied into an anonymous memfd, and sealed
against write, growth, shrink, and further seal changes before use.

The SquashFS parser independently requires the V4.0 magic, zstd compression,
128 KiB block geometry, no-xattrs flag and absent xattr table, valid table
offsets, and exact zero padding to a 4 KiB boundary. Revalidation repeats the
installed-root check, sealed-image checks, qualification-parent policy, and
parent pathname-to-descriptor continuity.

## Empty staging transaction

Staging consumes prepared custody and requires effective UID zero. It creates
one random private child beneath the retained qualification-parent descriptor,
then exactly seven root-owned, root-group, mode-`0700` empty directories:

| Name | Intended ownership |
| --- | --- |
| `base` | read-only base-image mount point |
| `root` | composed root mount point |
| `upper` | disposable overlay upper layer |
| `work` | matching overlay work directory |
| `run` | disposable runtime state |
| `state` | disposable fe2o3 durable-state fixture |
| `evidence` | canonical report staging |

The move-only staged value privately retains every directory descriptor and the
complete prepared evidence. Revalidation checks exact inventory, metadata,
empty children, one-filesystem placement, descriptor-to-path identity, and
qualification-parent continuity. It still grants no mount or execution
authority. Explicit cleanup and `Drop` remove only the transaction's random
child and synchronize the retained parent.

A 21-checkpoint interruption campaign covers root creation and metadata, every
child create and metadata transition, complete-tree verification, root sync,
parent-path verification, and parent sync. Every injected failure restores the
qualification parent to empty. Separate tests reject metadata/content
insertion and parent-path replacement.

## Mount composition

The root-only mount transaction runs only after entering a new mount namespace
from a dedicated single-threaded process and making `/` recursively private.
The namespace identity is retained and rechecked before every operation.

The sealed base memfd is attached with atomic Linux `LOOP_CONFIGURE`; legacy
partial loop setup is not used. A separate narrow crate validates loop-control
and major-7 device identities, requires a completely sealed mode-`0444`
backing file, requests only read-only plus autoclear flags, and rechecks the
complete kernel `loop_info64`. The deployment crate itself continues to deny
unsafe Rust.

Mount creation uses upstream kernel `fsopen`, `fsconfig`, `fsmount`, and
`move_mount` APIs. SquashFS is detached-created read-only, nodev, and nosuid,
then attached to the exact retained `base` mount point. OverlayFS is
detached-created nodev and nosuid with this fixed lower order:

```text
installed deployment root : sealed SquashFS base
```

Only the staged `upper` and `work` descriptors provide writable overlay state.
After attachment, the transaction checks SquashFS and OverlayFS magic,
mountpoint-to-retained-descriptor identity, loop status, qualification-parent
continuity, and every installed manifest/content file against sealed deployment
custody. The move-only mounted value still grants no boot or execution
authority. Cleanup unmounts overlay first, SquashFS second, releases the
autoclear loop device, and then preflights and removes the exact staging tree.
The recursive removal is descriptor-relative, follows no symlink, crosses no
mount, and enforces the same 64-level and 131,072-entry bounds as interrupted
worker recovery. This permits later systemd steps to populate only the
disposable overlay while retaining deterministic fail-closed cleanup.

The fully static `fe2o3-compiler-execution-qualification` image exposes seven
commands. `probe` observes effective UID, task count, procfs, loop-control
identity, filesystem support, new mount API recognition, isolation namespaces,
cgroup V2, and fixed systemd tool paths without creating a namespace, mount, or
service. `run` is the sole high-level path through bundle verification,
installation, base preparation, staging, private namespace entry, mount
attachment, revalidation, explicit cleanup, and a canonical completion report.
`fault-points` lists the closed set. `fault` interrupts one exact point and
accepts success only after the root-owned qualification parent is empty.
`campaign` starts from an empty install parent and requires one publication,
nine exact reacquisitions, two normal runs, all eight faults, stable identities,
one exact installed-root child, and complete staging cleanup. These commands
grant no boot or service authority.

`recover` accepts only an empty qualification parent or one canonically named
qualification transaction. `recover-install` additionally requires the
out-of-band expected manifest SHA-256. It admits only that digest's deterministic
final-root name plus at most one canonical installer transaction. It validates
the final-root metadata without changing it, preflights the entire staging tree
under fixed depth and entry bounds, removes only staging through retained
descriptors without following symlinks or crossing mounts, syncs the parent, and
revalidates both pathname identity and exact final inventory. Unknown siblings,
multiple transactions, noncanonical digests, and substituted roots fail before
deletion.

`run`, `fault`, and `campaign` are process-supervised. Before recovery or
launch, the parent acquires nonblocking exclusive advisory locks on retained
install- and qualification-parent descriptors in stable device/inode order.
The hidden single-threaded worker binds itself to the exact parent PID with
Linux parent-death `SIGKILL`, waits for the same dual-parent lease, repeats
recovery under that custody, and holds it throughout namespace and transaction
mutation. Every worker is the leader of a dedicated process group. The parent
retains a pidfd and observes exit with `waitid(..., WNOWAIT)`, then kills the
entire group before reaping the still-unreused leader PID. This closes both
timeout and apparently successful exits over descendants introduced by later
systemd tooling. `run` and `fault` have 120-second deadlines; `campaign` has a
20-minute deadline. `SIGTERM`, `SIGINT`, `SIGHUP`, and `SIGQUIT` are recorded by
async-signal-safe handlers. Timeout or signal handling kills the group and
reaps the worker before reacquiring the lease and recovering both parents.
Worker stdout and stderr are captured independently in anonymous memfds under
one-MiB bounds.
Success evidence reaches caller stdout only after the worker exits zero, emits
no stderr, and post-worker recovery reports that no staging was present. A
successful worker that leaves recoverable staging is still a failed
qualification. The standalone recovery commands do not terminate workers and
must not be used concurrently with a supervised command.

This implementation currently has compile, unit, custody-doctest, strict
Clippy, strict rustdoc, static-musl, ELF loader-independence, and live read-only
probe evidence. The current `mi300x` SSH identity has effective UID `1002` and
no mount capabilities, so no successful kernel mount is claimed yet. The live
root harness, live timeout/signal recovery, and live execution of the implemented
mount fault campaign remain required before this boundary is
production-qualified.

## Qualification

The source-only contract runs on any generic CI host:

```console
$ scripts/tests/compiler-execution-qualification-base.sh
```

On Ubuntu 24.04, build twice from the same clean commit and validate exact
reproducibility:

```console
$ scripts/build-compiler-execution-qualification-base.sh /tmp/fe2o3-base-a
$ scripts/build-compiler-execution-qualification-base.sh /tmp/fe2o3-base-b
$ scripts/tests/compiler-execution-qualification-base.sh \
    /tmp/fe2o3-base-a /tmp/fe2o3-base-b
```

The full check validates exact inventory, modes, links, `SHA256SUMS`, checkout
commit and epoch, all package records, SquashFS profile, embedded metadata and
target bytes, and byte equality of every published file.

The remaining production gate must execute the static harness and its complete
fault campaign under real host root, then boot `fe2o3-qualification.target` in
isolated PID, network, IPC, UTS, cgroup, and mount namespaces. It must then
exercise
sysusers/tmpfiles, socket activation, distinct service and client identities,
provisioning exclusion, successful compiler execution, restart and crash
recovery, mount-crossing and hostile-parent cases, and complete cleanup.
