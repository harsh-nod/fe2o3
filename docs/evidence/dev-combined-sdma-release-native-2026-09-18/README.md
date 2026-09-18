# Combined SDMA Native Teardown: Partial Campaign

**The seven-case campaign was rejected, not accepted.** Four packetless public
combined-release cases fully passed. A fifth native command passed, but its
required delayed observation detected a newly attached process on the selected
GPU. The frozen stop-first-failure policy prevented the remaining two cases.
No gate was relaxed and no case was retried within this campaign.

## Results

The public `--retained-release-combined-sdma` path creates two directional
queues and N striped queues through the actual public constructor. It then
uses public selector/preflight/retained release, rejects an inert retry and
drops completed custody before reporting success. Counts refer to striped
queues, not the total N+2 SDMA owners.

| Striped Count | Native Command | Strict Post-Observations | Resources Returned |
| --- | --- | --- | --- |
| 2 | Passed | Immediate and delayed passed | 17 |
| 4 | Passed | Immediate and delayed passed | 23 |
| 6 | Passed | Immediate and delayed passed | 29 |
| 8 | Passed | Immediate and delayed passed | 35 |
| 10 | Passed | Immediate passed; delayed refused | 41 |
| 12 | Not executed | Not executed | Not observed |
| 14 | Not executed | Not executed | Not observed |

All five native commands exited zero with empty stderr and exact complete
two-line stdout. They reported distinct non-primary IDs across both groups,
directional engines 1/0, alternating striped engines, capacity 2/8/14,
4096*(N+2) host bytes/N+2 records, exact 11+3*N resource disposal, configured
account refunds, rejected retry and completed root Drop. The directional engine
marker prints previously asserted literal values; the striped cursor is
source-qualified initial state, not an independent public cursor observation.
No copy/compute packet or MMIO store was submitted.

## Shared-Host Refusal

The campaign ran on MI300X GPU 1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`,
with CPUs 0-47 and NUMA node 0. Exact topology was checked and full idle
admission was required immediately before every invocation. This implements
permission to use currently free devices, not an exclusive reservation.
Native runner execution spanned 2026-09-18 17:38:14-17:40:37 UTC.

The fifth delayed observation began at T0+20.031 seconds. Its ROCm status and
PID captures completed normally. Sysfs GPU busy was 5/5/4%, with VRAM rising
from 298,725,376 through 484,745,216 to 918,433,792 bytes. SMI reported 4% busy
and 648,368,128 VRAM bytes. PID 486770 was attached to GPU 1 and absent from
the exact recorded owned PID/group roster. The archived pinned observer
rederives all seven refusal reasons from its original raw captures.

There were 15 observations: five preflights and ten post-observations, of which
14 admitted and one refused. The fifth probe's successful stdout does not
override that refusal. This is evidence of shared-device contention, not a
runtime teardown failure or a demonstrated retained-memory leak. The other
process's workload was not inspected, stopped or removed.

## Setup And Cleanup

An earlier setup could not create
`/tmp/fe2o3-combined-sdma-20260918.47b2755b`: the root filesystem was byte-full
(100%), not inode-full (15%). No payload was uploaded or executed there.
The failed command and independent exact-path absence check are retained,
along with read-only filesystem receipts. No uncertain stale files were deleted.

A newly bound payload used the private mode-0700 directory
`/home/harsh/fe2o3-combined-sdma-20260918.d3b7bc43`. Creation verified a writable
executable `/home` filesystem and sufficient free space. All campaign writes
were beneath that owned directory; remaining `/tmp` references were working
directories only. Both setup attempts and their distinct payload identities
are preserved without relabeling the failed attempt.

All 96 remote files were collected and hash-checked before exact-directory
removal. All 23 recorded owned PIDs/groups and accessible same-user references
were absent before cleanup; a separate subsequent invocation confirmed path
and PID/group absence. Proc visibility limitations remain in the raw records;
this is not an all-user or inaccessible-reference absence claim. No remote
build was performed. Earlier collection failures would have retained files
for controlled recovery rather than deleting uncollected evidence; this
campaign's collection and cleanup both completed despite native rejection.

## Binding And Audit

- Signed containing source commit: `c19dd3adf33402a63bdcc0404effed39c2f33b75`.
- Complete source cohort: 5,557 inputs, SHA-256
  `9f5447bdbffb3277ff3750f2747a5300d7d69b05065bee149c297a5f4abe08cb`.
- [CPU evidence](../dev-combined-sdma-example-cpu-2026-09-18/README.md) seal:
  `529398ddbab4825c6847e918f26e1a0cc6b4554963f20fc37b7b2d9cbb155477`.
- Static musl ELF SHA-256:
  `08e0ed94feac79cb8126e536a3926994ebda3c4eff43e3edc3276faf11a1586d`.
- Executed payload: 25 entries, SHA-256
  `1aed06aa903f7131a8d7d64249ebf516e6dd6f042f4c42f5002cb3a7e47a30e0`.
- Twenty archive calibrations cover raw refusal replay, exact case prefix,
  transcripts, command/timing constraints, calibration rosters, inventory,
  cleanup, source/binary receipts and preserved setup rejection.
- Four protocol and four controller calibration tests passed inside the native
  controller before upload; their whole transcripts are audited. Separate
  preliminary `raw/qualification` receipts (including those under setup-rejected)
  are retained for provenance and hash integrity, not used as acceptance gates
  by the historical verifier.

`python3 -B verify.py` is read-only: it audits the exact sealed archive and
preserves the campaign's failed disposition. `--source-root PATH` rehashes all
historical source inputs, and `--binary PATH` rehashes a supplied ELF. The ELF
is intentionally omitted from Git; its hash, original complete inventory and
collection receipt remain. Signature output records the earlier cryptographic
check; the historical verifier does not itself re-run Git signature verification.
`--seal` is a one-time manifest creation operation, never a runtime invocation.
Archived control scripts document execution; do not rerun them.

This advances R126 development only. A complete clean seven-case native
campaign remains outstanding, as do submitted-work/native-fault qualification,
full runtime-facade integration, LogicalMux/terminal-creation retained teardown,
formal production correspondence and matched HIP/HSA benchmarks. A1/A2, #182
and full HIP/HSA parity remain open. No performance claim is made.
