# Multi-GPU Admission Unblocking

This checkpoint prioritizes one ordinary admitted two-GPU application. It is
not an A3 completion, GPU execution, formal theorem or HIP/HSA parity claim.
Native agents independently reviewed mount semantics, application admission
requirements and the narrowed filter; the primary integrated the changes.

## Changes

- Shared the sequential measured-deployment Cargo target directory while
  retaining all child-builder tests and static-image checks. The observed cold
  build took 17:54.96; warm builds took 46.66 and 40.44 seconds. These are local
  build timings, not controlled runtime performance comparisons.
- Added only `pidfd_getfd` to the qualification machine's default nspawn syscall
  filter for root-side original descriptor observation. Startup alone would not
  qualify the cross-UID operation.
- Corrected the SquashFS absent-xattrs flag from `0x0100` to `0x0200`. The former
  means uncompressed xattrs. A real reproducible image now anchors the positive
  parser test; recomputed-digest mutations exercise actual profile rejection.
- Set the SquashFS filesystem context read-only before creating its superblock.
  Setting only the later mount attribute had requested write access to an
  intentionally read-only loop device. Image seals and loop protections remain.
- Admitted production-auditor inspection syscalls in the permanent application
  filter, with argument-specific filesystem-ID queries and exact memfd flags.
  Process creation, new sockets, namespace changes and replacement exec remain
  prohibited. This does not solve the separate proof-custody boundary.

## Measured Startup

The successful static build at `8266028a310e8401115c434ed7613c5f07bbaa65`
produced deployment manifest
`218a2c7e2289a12f12960c0f0916cdd1ac080020934d8e974c8acae88ca76423`.
It used the independently pinned base built at
`41a56d2dc106e388c96743ab311a26a60b8f96d5`, SHA-256
`b05a6f5cc63894e66dcb449b409dbdca9e064f4f5d2090b24e9cd514103b26e5`,
31,657,984 bytes. These are deliberately recorded as distinct source commits.

All 99 locked package versions and SHA-256 values were preserved. Superseded
packages were retrieved using private apt configuration and the signed
[Ubuntu snapshot service](https://snapshot.ubuntu.com/) at
`20260831T050000Z`; host apt configuration, package state and indexes were not
changed. Existing TLS trust was used, not certificate verification bypass.

The first root run rejected the genuine base due to the incorrect xattr flag.
After correction, the next root run reached superblock creation and failed with
`EACCES`. The read-only context fix removed that failure in the focused mount
test, exposing `EINVAL`: local kernel `6.6.87.2-microsoft-standard-WSL2` has
SquashFS but `CONFIG_SQUASHFS_ZSTD` is disabled. The nonmutating host probe's
advertised prerequisites do not establish compression support.

The separate gzip fixture qualifies the read-only mount mechanism only. The
production parser explicitly rejects it, and the zstd mount test remains
separate. No base-format fallback was added. MI300X's kernel advertises zstd
support, but noninteractive sudo and root SSH were unavailable to this account.
No remote files, services, allocations or GPU workloads were created.

## Validation

- Deployment: 80 library tests and four qualification-command tests pass; both
  explicit privileged mount tests are ignored by the ordinary run. Strict
  Clippy and both deployment/base shell contracts pass.
- The gzip root mount test passes independently, including real marker readback,
  read-only/nodev/nosuid flags and rejected writes. The zstd root mount attempt
  fails on the unsupported local kernel; it is not counted as a pass or skip.
- Seven application-sandbox unit tests pass, including direct evaluation of
  generated BPF argument decisions independently of outer host restrictions.
- Three actual-filter application tests pass: required auditor operations,
  process/session escape rejection, and static/dynamic exec rejection. They
  use the existing static test-signed handoff fixture, not genuine issuer proof.
- Strict Clippy passes for the changed Cargo binary, runner fixture and vertical
  test (`--no-deps`). A broader attempt stopped on the pre-existing
  `derivable_impls` warning in `fe2o3-semantic-import`; that unrelated file was
  not modified. The initial test-fixture type-complexity warning was fixed and
  the actual-filter tests rerun successfully.

For repeated sequential vertical runs, `FE2O3_V3_STATIC_FIXTURE_TARGET_DIR` may
name an absolute private Cargo target directory. Without it, the existing
per-process temporary-directory behavior is unchanged. It caches builds, not
test results; static binaries are rebuilt as needed and every selected case
still executes.

## Evidence And Cleanup

`qualification.tar.xz` contains build/base logs, exact input manifests, private
apt configuration, failed startup and recovery reports, host observations,
test/lint output, reproduction scripts and relevant source SHA-256 values.
Archive SHA-256:
`9e57227f0c662691746387cb5aba20a57e6b325922665b5249ad8cba259f0808`.
The auditor change is `3cfddb119`; the read-only mount change is `1423c46f1`.

`cleanup.log` records removal of the owned root installation/qualification
scratch parents, temporary source worktree and test/package scratch data. The
independently pinned bundle/base and useful build cache remain under ignored
`target/qualification-startup-20261004-8266028a3` and
`target/qualification-static-deployment`. No shared MI300X resources need cleanup.

## Remaining Gate

Measured systemd startup, genuine compiler acquisition/current-record auditing,
and ordinary application proof custody remain unqualified. The no-fork filter
is installed before application exec; there is no pre-ACK local proof hook or
existing transfer of the concrete retained proof owner. A separately
authenticated retained-proof contract must preserve original owners through
both devices' invocation settlement. Serializing signed receipts is not that
contract. See the [critical path](../../runtime-multi-gpu-critical-path.md).
