# Retained XGMI Host Diagnostic CPU Qualification

This package qualifies the reviewed twelve-file implementation signed as
`d6b1906d0d24b366ea4222fbca4ae4a6966fad6c` against CPU/static checks. It does not
qualify native GPU execution, timing attribution, formal refinement, runtime
facade parity or performance against HIP/HSA.

## Explicit Linked Record

The result combines **19 replayed stages from the rejected second build
attempt with seven fresh continuation stages**. The prior attempt remains
rejected: its next-stage 4 GiB free-space gate stopped execution. Its freshly
built no-default and all-feature test executables were retained and checked
before/after continuation. They were not rebuilt by the continuation. This is
not a single continuous process-ownership or deadline window.

All seven continuation stages passed: seven full Rust-path formatter checks,
the exact added test fragment formatter check, the remaining focused custody
test, strict all-feature Clippy, ordinary/diagnostic example builds, the full
all-feature KFD library test suite, and closing source HEAD. The full suite
reported **1,830 passed, zero failed, zero ignored and zero filtered out**. The
no-default test executable listed 1,824 tests and ran the focused diagnostic
controls; its full suite was not run. Neither example was executed on a GPU.

The 6,477-file source inventory was identical across the prior build and
continuation. Fresh Git/blob and filesystem binding after signing confirmed
that the implementation commit contains the same bytes and modes. The CPU
records retain their original unsigned-source base identity
`1c589082e8c76410ab3b5b2683468ca6a1932519`; signing does not rewrite those records.

Three earlier rejections are included without promotion:

- First build attempt: the full suite found a stale wait-body source guard.
- Second build attempt: the next-stage local disk-reserve gate stopped the run.
- First continuation: full-file formatting found inherited formatting changes.

The final formatter scope checks seven full Rust paths and the uniquely bound
added test fragment. Removing that exact fragment reconstructs the original
test-file Git blob. This does not claim full-file formatting acceptance for
the inherited test file.

## Portable Helper Controls

The separate native24 helper integration passed 26 synthetic controls in its
source worktree and the same 26 in a Git-free relocated projection with an
empty `PATH`. Six fresh owned command groups closed. The planner preserves the
existing 18 ordinary trials exactly as a subsequence; six profiled trials use
the separate strict parser. These are synthetic lifecycle/replay controls,
not native qualification or performance measurements.

## Contents And Limits

`summary.json` provides the scoped results and original executable hashes.
`raw-index.json` binds every archive member to its original path, length and
SHA-256. `raw.tar.xz` retains all four CPU attempt records, their controllers,
raw command streams and receipts, source/tool inventories, and final portable
helper controls including the source-only relocation. The archive was read
back in full and all originals were rehashed afterward.

The archive contains 410 files, 25,758,844 logical bytes and 1,102,620 compressed
bytes. Its SHA-256 is
`f2e534117fa242cf7e41da14a060d4461588736a751b2d25ca36a1713ca354e2`.
No target tree, ELF payload, full source checkout or remote directory is
included or removed. Original test/example executables remain retained.
Packaging ran no tests, solver, remote command or historical PID probe.

## Signed Source Binding

`signed-candidate.bundle` retains the original signed implementation commit
and its delta over prerequisite
`1c589082e8c76410ab3b5b2683468ca6a1932519`. The bundle contains one advertised
head, `d6b1906d0d24b366ea4222fbca4ae4a6966fad6c`, and is 22,504 bytes. Its SHA-256
is `4d7fdad71b7d0e31726f13804c1c9ff09329eeaf7a27d79c4deab778c29a5be6`.
It requires a repository containing the prerequisite commit; it is not a full
repository or source-checkout archive.

`signed-source-binding.json.xz` contains all 6,477 selected Git blob IDs, modes,
lengths and SHA-256 values, each checked against the unchanged CPU inventory
and current signed implementation checkout. `signed-source-verification.json`
records the expected SSH signer/key, signature verification, bundle validation,
source-binding hashes and the per-file index for `signed-source-records.tar.xz`.
That separate archive retains nine fresh owned Git command receipts, streams,
the controller and closed-group census. `allowed-signers` contains the public
verification key. No historical process IDs were re-probed.

The original `raw.tar.xz` and `raw-index.json` remain byte-for-byte unchanged.
`summary-before-signed-binding.json` preserves the preceding publication
summary; `summary.json` additionally binds the new signed-source artifacts.

These checks do not extend the existing Verus theorems to the diagnostic
observer. Native24 execution still requires reviewed signed source binding,
fresh four-executable builds and identities, shared-host admission, matched
ordinary/profiling separation, raw collection and verified owned cleanup.

## Integration Readback

The independent primary-agent readback on 2026-10-01 verifies all 410 raw
members against their original files, all 29 signed-source record members,
all 6,477 selected source files against both the CPU snapshot and authenticated
Git blobs, and the original signature and bundle. Its local controller and
receipt are `fe2o3-diagnostic-publication-audit-20261001.py` and `.json`; the
controller's original absolute paths describe this retained local replay,
not a portable runner.

`local-ci/` records all seventeen exact workflow commands passing at integration
commit `4ad64047b0887a747a41aa7b0b9fa2f4206279c1`. This includes 64 ordinary
series controls and 40 diagnostic parser/planner/native/transport controls.
The ordinary controls compile and execute two CPU C++ callback tests. This is
local CI replay, not hosted CI, a Rust suite, a solver campaign or GPU execution.
The included integration audit documents the four reviewed guard refreshes:
their executable proof inputs and obligation/mutation rosters are unchanged.
No historical proof campaign is relabeled as qualifying the new observer.
