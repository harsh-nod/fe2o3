# gfx950 no-queue metadata registration — 2026-09-24 UTC

One root-supervised standalone process on mi350 registered the actual retained
trap, enabled the mode-3 debug runtime and published version-11 code-object
metadata. The process created no queue and dispatched no kernel. This is a
registration-only engineering result toward #281 V4, not live capture or an
accepted milestone exit.

## Implementation and lifetime

The unsafe consuming
`Gfx950DebugColdOwnerV1::enable_debug_metadata_without_queue` retains the real
checked-device, kernel, trap and metadata custody in its successor. It marks
possible exposure before SET_TRAP_HANDLER and retains all resources on failure,
unwind and Drop until process exit. It does not implement rollback, retry,
runtime disable, trap clearing or native cleanup acknowledgment.

The separate `observe_gfx950_debug_metadata_noqueue_v1` executable requires
explicit mapping, retention and isolated-activation acknowledgments. Its
environment/thread/FD/mapping checks are refusal fences, not general isolation
proofs. The reviewed standalone program and trusted supervisor must exclude
foreign runtimes, queues and external injection for the whole process lifetime.
The old cold observer remains version-zero/inactive and byte-unchanged.

Root reviewed the installed amdgpu-6.16.13-2303411.24.04 SET_TRAP_HANDLER,
runtime-enable and CWSR paths, exact 24-/16-byte UAPI layouts and mode bits.
The selected trap and ROCdbgapi source relationships remain those documented in
[the API contract](../gfx950-debug-metadata-noqueue-v1.md); they do not attest
installed binary reproducibility. Readelf showed only libc, libgcc_s and the
loader, no RPATH/RUNPATH. The supervisor pinned those bytes, used fresh exec,
an empty environment, no inherited GPU FDs, a 20-second child bound, 8-MiB
stream caps and an owned process group. The observer bounds its JSON to 4096 bytes.

## CPU and exact refusal qualification

The final KFD gate passed 444 default and 581 engineering library tests
(one ignored each), 6 old cold-example tests, 14 new observer tests and 32
engineering documentation tests. Both all-target, no-deps strict KFD Clippy
configurations passed, and the standalone binary built successfully.

An earlier strict lint run failed on duplicated test-module inclusion and one
alignment expression. Those were corrected without allowances; the failed
receipt remains retained. Duplicate retention tests are not included in the
final 581 count.

Eight separately supervised executable negatives passed exact phase/reason
checks: ambient environment, missing activation acknowledgment, artifact size,
digest, kernel, node, GPU and device profile. All refused before debug native
preparation/activation; ordinary read-only device admission can already open
descriptors. Generic crashes did not count. Post-exit signal attempts recorded
group_absent, not a descendant-quiescence certificate.

## Actual observed registration

The native fixture was `fe2o3_gfx950_observation_fixture`, gfx950:xnack-,
Wave64, node 2, GPU 39903, unique ID 16366993098680759275. It is not the new
gfx942 complete-body source kernel.

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| Standalone binary | 3672784 | 6d878b7afac83e069d3972a1f19d93229ac2f701f770827b148815ef2a717b06 |
| HSACO | 5536 | d10b592732d91cf4c0d4289890fdd0a5328e84cb8208817eaabac4c62c1a34c6 |
| Trap | 1116 | 4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe |
| Positive report | 3349 | feb5c8c215558c246b6e13bdff453090adb14941e48dcbe67d57755cb6e40b70 |
| Negative report | 15226 | 59f44122eef4097a1026b79c30a94a918f6d5615e71a25ee50230d14e7864dfd |
| Positive stdout | 1276 | 5101d4cad81b5813b3c07d0332e34fc4de97fdca495655b28f375e2b5b9004d5 |

Device-profile SHA-256:
`6f859b0a67f8ee2497393206930ff35bf1a9ae69d33f172106a790a0c9226667`.

The process reported registered_no_queue: trap registered, runtime enabled and
metadata published, with 16384 logical mapped backing bytes and 5744 metadata
bytes (not RSS). It exited 0 with empty stderr, complete drained streams and
no signal. Its 77-ms child interval is not a benchmark. No attached debugger's
acceptance was observed.

TMA remains zero and unqualified for sampling execution. Queue creation,
dispatch, trap execution qualification, physical capture and cleanup
acknowledgment are all false. Direct-child exit/reaping does not prove driver
cleanup, descendant quiescence or general external-injection exclusion.

## Retained receipts

All three passing gates froze HEAD
`24702115958e1b59c1c20c71ab75f3aeaf07d6a4` plus 7195 working-tree files,
107449373 bytes, source SHA-256
`993df5747347fc17e72b019f52ad9c910c9c1cca6016ea2b4ed1f566667082f7`.
HEAD alone did not contain the tested pending changes.

Under
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr/logs/phase28-resume-r7-`:

| Gate / receipt.json | Bytes | SHA-256 |
| --- | ---: | --- |
| compiler-debug-noqueue-cpu-r1 (failed lint) | 12818 | 15d061f95597f476d915408f64a29e68663d624b6266a5919f457bbdabf98027 |
| compiler-debug-noqueue-cpu-r2 | 21110 | 21ec4ba76dc2d9757e2404c1d2b684cb24a4bff20f90c85f68f9f6935dc6a6b4 |
| compiler-debug-noqueue-negative-r1 | 33309 | 00945e915dedd256d9308ddd1ba5a971954ecd84358609dd094dab077526e37b |
| compiler-debug-noqueue-actual-r1 | 32719 | e72382f9740fdcc76d5aa085d3378e0310c638238d0ac5b38a6a4a4997856240 |

The [companion tutorial and byte-exact reports](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/gfx950-noqueue-registration-qualification-20260924.md)
retain the registration-only boundary. Source inventory and file hashes are
observations, not portable source custody or transitive build attestations.

Still required for V4: executable trap/TMA and sampling safety, separate owned
queue/debug lifecycle with teardown acknowledgments, attached-debugger
interpretation and actual same-stop wave/register capture with stale/cancel
controls. Accepted original exits remain M1/V1/V2/U1/U2/U3 (6/18).
