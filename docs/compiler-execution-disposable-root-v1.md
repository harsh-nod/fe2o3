# Compiler-execution disposable root V1

Status: deterministic base construction, caller-pinned SquashFS admission, and
sealed preparation custody are implemented. Root composition, isolated systemd
boot, distinct-UID execution, and lifecycle fault qualification remain open.

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

The remaining production gate must consume only retained descriptors, mount or
materialize the sealed base read-only, layer the installed root without
reopening admitted content, create disposable writable state, and boot
`fe2o3-qualification.target` in an isolated namespace. It must then exercise
sysusers/tmpfiles, socket activation, distinct service and client identities,
provisioning exclusion, successful compiler execution, restart and crash
recovery, mount-crossing and hostile-parent cases, and complete cleanup.
