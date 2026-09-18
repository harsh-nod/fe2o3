# Direct Destination SDMA Readback Native Plan

Correctness qualification only, from signed source
`fd1cf3dd05e691797533b1beab1f015c4b2d8fad` and the fresh full GNU/musl
runtime CPU packet `docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18`.
The exporter compares every selected source hash against the signed commit and
pins the exact tested musl ELF. Old native bundles and sealed archives are not
edited or reclassified.

Run the two existing ignored `cold_allocation` tests separately, device then
host, with exact test identity and one passing harness each. The tests check
capacity rejection/refund, successful retry, 4-KiB native zero/pattern/XOR bytes,
native-versus-shadow routing, final accounting, shutdown and inert retry. The
readback SHA-256 must be
`2e7d5db52627eb273e9c3837f94bdc7bff796711c2e0960f3c0416d2cb9868d9`.
No compute dispatch, production verifier, native fault injection, native
allocation count, subrange/multi-packet coverage or performance claim follows.

The transcript checker independently validates the exact test identity, harness
success, kind, byte counts, single marker and readback digest. Accounting and
shutdown debug fields are opaque; their guarantees come from assertions in the
pinned test source and executable, not independent parsing of those fields.

Reuse the previous reviewed runner/controller protocol. Candidate GPU 4 is
UID `0x54f88318ca05093d`, BDF `0000:85:00.0`, CPUs 48-95, NUMA 1. This is
not a reservation. Require complete fresh idle/PID/VRAM admission before EACH
test and actual launch within one second. Reject missing metrics, PID attachment,
nonzero GPU use or VRAM at/above 512 MiB. The minimal process environment adds
`FE2O3_TEST_NATIVE_ISOLATED=1`; that acknowledgement is not exclusivity proof.

Retain unchanged per-test 180-second timeout, 200-second outer wait, zero core
limit, 16-MiB output limit, CPU/NUMA placement, and owned-process-group cleanup.
After the entire test group is absent, require immediate T0+[0,1] and exactly
one delayed T0+[20,21] strict complete observer endpoint. Preserve both results
even on rejection; stop before any later test after a failure. No retry,
relaxed threshold, GPU reset or foreign-process signaling is authorized.

Only after review, create a fresh 0700 directory matching
`/tmp/fe2o3-sdma-readback-fd1cf3dd-20260918.XXXXXXXX`, bind its exact path,
commit and payload hash in the owner/approval markers, upload and reverify the
complete payload. Never reuse an older directory or build on the shared host.
After any outcome, collect all files, compare hashes with the remote inventory,
confirm all recorded groups and visible same-user path references absent, then
delete only this exact marked directory. A separate command must confirm path
and recorded PID/group absence. Inaccessible processes remain visibility limits.

The previous HIP/native handoffs are closed. A new read-only SMI selection
snapshot is not launch admission. Root must review the frozen bundle and
controller and explicitly authorize the one fresh campaign before execution.
