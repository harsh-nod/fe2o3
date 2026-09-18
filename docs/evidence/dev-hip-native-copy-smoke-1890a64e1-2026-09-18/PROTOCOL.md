# Prospective One-Process HIP Smoke

Status: prepared for review, not authorized for native execution until the
primary gives READY for the frozen runner/checker hashes. This is not a matched
benchmark and does not change any prior rejected campaign.

## Immutable Inputs

- Source commit: `1890a64e1911a2a346c5a9e70a8ff5ad45b19231`.
- Exactly three exported Git blobs: `async_copy_hip.cpp`,
  `native_benchmark_args.hpp`, and `copy-host-observe.py`. Their complete paths,
  Git identities and SHA256 identities are in `source-export.json`.
- Real HIP native executable: SHA256
  `d2c012774e2321266d2708d7d7f40745d332ebbebf9d26c9c5f61c4bc57ad976`.
- Real build used explicit `--offload-arch=gfx942`, CPU affinity 48,49,
  core limit zero and a 180-second build deadline. No GPU executable was run.
- Actual compiler dependencies, dynamic libraries and tools: 350 files in
  `prepared-results/platform.sha256`. No mock is linked.
- Separately overlaid HIP payload validator: SHA256
  `d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561`.

## Native Command

The exact command and environment are constants in `protocol.py`. One process
only, 256 MiB, depth one, three warmups, ten measured rounds. GPU 4 is bound to
UID `0x54f88318ca05093d` and BDF `0000:85:00.0`; effective CPU affinity must be
48-95 and memory policy node 1. The comparator sees HIP device zero under
`ROCR_VISIBLE_DEVICES=4`, with `HSA_XNACK=0`. Other visibility filters are removed.
The native timeout is TERM after 180 seconds, KILL after five more seconds, with
core size zero and an additional owned process-group custodian.

## Predeclared Observation Schedule

Fresh complete preflight must satisfy every original observer criterion.
The runner will not launch if more than one second elapses after preflight
completion before its native launch decision. There is no retry or fallback.

T0 is the parent's monotonic timestamp immediately after reaping the native
process, not an inferred timestamp of its last GPU operation or individual free.

1. The immediate observer's own `started.monotonic_ns` must fall in T0+[0,1]
   seconds. The full capture is retained even if refused. Only
   `sysfs-before-busy`, `sysfs-between-busy`, `sysfs-after-busy` and `smi-busy`
   are allowed as immediate telemetry reasons. Original observer exit 1 and
   `endpoint_admitted=false` remain unchanged. Every identity, capture, PID,
   invalid-data or VRAM fault still rejects the smoke.
2. One delayed observer is scheduled at T0+20 seconds using the monotonic clock.
   Its actual internal start must fall in T0+[20,21] seconds. All original
   strict criteria must pass. A missed window or nonzero busy result rejects.
   No later replacement observation is permitted.

Immediate and delayed observations are attempted after native failure as well.
An endpoint is a sequential set of observations, not an atomic snapshot.
The unchanged VRAM limit is exclusive 536,870,912 bytes. Selected-GPU PID
attachments always reject. No reservation or continuous isolation is claimed.

## Native Payload and Outcome

Require exit zero, empty native stderr, one exact config row, thirteen indexed
rounds with changing patterns and full-buffer checks, and one completion row
showing three explicit frees and one stream destruction. Expected target is
`gfx942:sramecc+:xnack-`. Output is only printed after cleanup in the comparator.
The observer protocol, source/binary/tool identities and process-group closure
are separate required checks. No throughput ratios or performance acceptance
will be produced, even if the smoke passes.

## Custody and Cleanup

All raw outputs, failures, status and timestamps are retained. Only the exact
marked owned directory may be removed, after recording collection and absence
of all recorded owned PIDs/process groups. Accessible same-user executable,
cwd and fd references are checked; unreadable unrelated entries are explicitly
reported, not claimed to prove global reference absence. An independent stdin
invocation of the cleanup helper's `--absence` mode follows removal.

## Separately Scoped Driver Diagnosis

`raw/driver-source.*` records only installed package/module/source identities and
read-only source excerpts. It supports the interpretation of busy percentage as
firmware aggregate activity, not PID ownership. It does not identify the cause
of any previous busy reading, qualify firmware timing, or rehabilitate prior
failed observations. No source or driver settings were changed.
