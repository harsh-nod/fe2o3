# Producer Journal Observer Qualification

Signed candidate: `441cef8c7e49aee436f6e31b5255d461115d5559`.
Public parent: `6c0718c1d325e5c5bf5a95ea57e4cf6a842aaf2a`.

All 23 fresh qualification stages passed: three unfiltered full 165/0 positives
(including relocation), nine full 164/1 actual-body negatives, seven source and
diagnostic suites, and both signature and tool-release brackets. All 23 owned
process groups closed. Independent agent/root readbacks agree on the complete
705-file campaign, 6,537 signed source blobs, 38 proof inputs and ten projected
source trees.

Eight negatives fail actual journal result-equality contracts. The eager-status
negative fails only the wrapper ghost-trace contract; it does not establish an
inner native call count. Historical calibration captures are not qualified kills.
The campaign does not execute fresh CPU tests or prove live-allocation checks,
fresh credit locks, Arc/interior-state correspondence, machine code, hardware
ordering or HIP/HSA parity. The separate
[combined CPU regression](../dev-integrated-runtime-cpu-2026-10-01/README.md)
is accepted for the later integrated runtime.

## Public Records

- [records.tar.gz](records.tar.gz): the complete unchanged 705-file campaign,
  its original raw-helper archive, all ten source projections, independent
  readbacks and campaign/support controllers.
- [manifest.json](manifest.json): per-member original path, byte count, mode and
  SHA-256, plus source-bundle verification records.
- [signed-source.bundle](signed-source.bundle): the original signed candidate
  over its public parent, not a replacement integration commit.
- Archive SHA-256:
  `bd7ad73d9bc5d80510667628abf9c3485f37179d51100d76e20aabd46d6950fc`.
- Bundle SHA-256:
  `0bc2f762b408426cddc21f80513ed268e290c261b77e1eec0ef762d0c29e2175`.

Packaging rechecked every archived member against its immutable original,
verified the candidate signature and bundle prerequisite, and did not rerun a
proof or test. The raw campaign tree digest remains
`04bf845cee615d166142477585d14bf7af9a32353615f4cacc5ec32877ea4d16`.

This is not a self-contained portable campaign: the source bundle requires its
public parent, and the recorded CPU executable, toolchain/runtime libraries and
inherited CPU/tool/history prerequisites remain external. Controllers retain
original absolute paths and require explicit path mapping before relocation.
