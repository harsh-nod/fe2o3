# XGMI Creation: Native Development

Rejected campaign. The first KFD depth-1 process exited zero after 43.13 seconds
and emitted two valid bidirectional rows with passing canaries and explicit
teardown. The first GPU 1 postflight sysfs sample nevertheless reported 9% busy,
so the strict campaign stopped before HSA, HIP, or depth-16 execution. This packet
is not native qualification, formal refinement, or HIP/HSA parity evidence.

GPU 1's three samples in that observation were 9%, 0%, and 0%. VRAM stayed at its
298,647,552-byte baseline, and no mapped KFD process was found. GPU 2's postflight
and both twenty-second delayed checks passed. The cause of the short activity
reading is unproven; these observations alone establish neither a production
resource leak nor external contention. The original rejection policy is unchanged.

All 48 remote result files were collected and rehashed before the owned directory
was deleted. Independent path/process absence passed, and the local source payload
was deleted. The controller retained the native failure and exited one. All three
builds, ten controller calibrations, nineteen host-observer tests and the sibling
CPU packet verification passed. Source identities matched before and after the run.

The tested source is signed commit
`1161876123a061ca0a772937795daed648b02d62`. The rejected KFD prefix recorded
persistent-hot forward/reverse p50 values of 117.731/117.659 ms and remap-per-round
values of 206.475/206.435 ms. They are retained diagnostics, not accepted comparison
measurements. No HSA/HIP ratio can be computed from this incomplete campaign.

The controller requires the sealed CPU creation-root packet, its unchanged source
cohort and an exact verified signed commit before preparing the native payload.
It hashes all selected source files, the transferred payload, and all three
optimized benchmark executables. Recorded commands include compiler versions,
environments, output hashes, statuses, timestamps and owned process groups.

## Original Workload

The public runtime KFD benchmark and existing HSA/HIP comparators use 1 MiB copies,
depths 1 and 16, ten warmups and thirty measured samples. Each process exercises
forward then reverse copies. KFD emits both remap-per-round and persistent-hot
rows; HSA and HIP were intended to emit one row each, for eight required result
rows. Only the first two KFD rows were reached; the full roster did not complete.
The runner rejects incomplete output, incorrect identities or controls, malformed
metrics, missing canary/teardown claims, and benchmark diagnostics on stderr.

The selected devices were physical GPUs 1 and 2, authenticated by unique IDs
`ab83d2ffef0d3cdf` and `d2e26fef80cf5c33`, PCI addresses `0000:26:00.0` and
`0000:46:00.0`. KFD selects the physical IDs directly. HSA/HIP mask physical
devices 1 and 2 and authenticate their visible ordinals 0 and 1 against those IDs.

## Shared Host

The user permits observed-free devices, not an exclusive reservation. The strict
host observer checks both selected endpoints before every workload, immediately
afterward, and again after twenty seconds. It requires no mapped KFD processes,
zero observed GPU use and less than 512 MiB VRAM use. These are sequential endpoint
observations, not continuous monitoring or proof of isolation from other devices,
host CPU work, or shared interconnect traffic. CPU/NUMA affinity was not pinned.
The planned backend order was fixed, not randomized.

Builds and runs use a fresh mode-0700 marked directory under `/home/harsh`, bounded
process groups, core dumps disabled, and two Cargo build jobs. Each benchmark has
a 120-second limit. No foreign process is terminated and no foreign directory is
cleaned. Remote results must be collected and match their complete remote digest
inventory before exact owned-directory deletion. A separate command checks path
and process absence afterward. Uncollected failures retain their recorded path
for recovery. Local payload cleanup occurs only after remote cleanup closure.

## Interpretation

These are not identical engine-level microbenchmarks. KFD includes facade
admission, enqueue, explicit flush, waits and bookkeeping on one ordered SDMA
engine. HSA/HIP include their API submission and completion costs but do not pin
the same physical copy engine. KFD uses a 32-byte offset inside guarded buffers;
HSA/HIP use aligned, exactly sized buffers. KFD's hot loop has one extra prime and
validates unchanged payloads after the entire hot sequence. HSA/HIP poison,
rewrite and validate payloads each round outside the timer. Consequently the KFD
hot final-byte oracle cannot detect a skipped intermediate copy by itself.

Remap-per-round is a KFD diagnostic, not a matched HSA/HIP row. Persistent-hot
timings may be compared descriptively with the stated differences, but neither
these timings nor healthy-path cleanup prove performance parity, failure-path
retirement custody, arbitrary native unwind refinement, or general runtime parity.
The separately documented retirement failure-path gap remains open.

KFD prints only after explicit context and native shutdown, and HSA after explicit
runtime shutdown. HIP frees buffers and streams but does not reset devices or
disable peer access; process exit and delayed observers are part of cleanup
acceptance. Ten CPU calibration tests cover malformed result rows, archive
members, ownership markers, receipt integrity, seals, bounded process cleanup and
collection-before-deletion ordering and replay of raw observer evidence. All
nineteen existing CPU host-observer tests also run.

```sh
python3 -B docs/evidence/dev-xgmi-creation-root-mi300x-2026-09-18/rejection.py
```

`rejection.py` checks this exact rejection and cleanup record plus archive integrity;
it does not convert the campaign to native acceptance. The frozen original
`verify.py` remains a full-campaign verifier and intentionally rejects this packet.
Neither verifier reruns native workloads. They depend on the sibling CPU archive
and SHA-pinned read-only observer parsers in the earlier LogicalMux native archive.
No performance threshold or parity verdict is emitted.
