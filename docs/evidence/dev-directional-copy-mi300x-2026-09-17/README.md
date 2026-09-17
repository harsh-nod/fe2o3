# Shared MI300X Directional Copy Diagnostic

Completed diagnostic, not HIP/HSA parity acceptance or native reader-journal
qualification. No runtime production code was changed for these measurements.
The separate [reader proof packet](../dev-v4j2-reader-preflight-2026-09-17/README.md)
does not derive a native or performance claim from this archive. A1/A2 remain open.

## Device And Workload

Runtime source: clean committed `ed5b5d64bf95116c21e9bf350c30132ff2bbb524`.
Device: MI300X GPU 1, UID `0xab83d2ffef0d3cdf`, PCI `0000:26:00.0`, gfx942,
XNACK disabled. GPU 1 was free before admission. GPU 0 had an existing active
allocation and was not used; no second GPU was measured or reserved.

The user's permission to use free devices was sufficient. Before and after each
backend process, the guard required the exact UID/BDF, zero reported utilization,
VRAM below 512 MiB and no mapped PID on GPU 1. This is point-in-time shared-host
evidence, not continuous isolation or a reservation. The final observation
returned to the original 298,647,552-byte VRAM baseline with no mapped GPU 1 PID.

The unchanged directional runtime example uses public `RuntimeContextV1` with
the optional version journal disabled. HIP/HSA use the repository's existing
`async_copy_hip.cpp` and `async_copy_hsa.cpp`. All use depth 1, three warmups and
ten samples per process. Every returned byte is checked against a per-round
uniform pattern; setup, host initialization and validation are outside timing.
Timing is host-observed submission through completion, not a device timeline.
The separate HIP/HSA allocation/free probes are outside copy timing and are not
compared to KFD.

Each complete campaign rotates backend order KFD/HSA/HIP, HSA/HIP/KFD,
HIP/KFD/HSA. Reported ratios are medians of three paired per-process p50 ratios,
not pooled samples. Producers retain p50/p95 summaries, not individual samples;
this small diagnostic does not establish statistical confidence or a release gate.

## Results

`benchmark-settled` completed nine 256 MiB processes and every postflight check.
`benchmark-numa` repeated those nine processes with requested Linux CPU affinity
0-47 and memory policy bound to node 0, selected by the recorded GPU topology.

| Placement | KFD/HSA H2D latency | KFD/HIP H2D latency | KFD/HSA D2H latency | KFD/HIP D2H latency |
| --- | ---: | ---: | ---: | ---: |
| Default | 1.215x | 1.260x | 1.055x | 1.067x |
| CPU 0-47, memory policy 0 | 1.200x | 1.256x | 1.051x | 1.066x |

KFD remains slower in this workload. The requested NUMA policy did not materially
remove the gap. `/usr/bin/numactl` is explicitly invoked and hashed; its policy
output is retained. This does not prove physical page residency, equivalent
HSA/HIP/KFD host mappings, HSA CPU-agent pool selection, or that runtime worker
threads never change affinity.

`benchmark-boundary` completed 18 processes at A=264,239,136 bytes and
B=264,239,137 bytes. KFD changes from exactly 63 packets/one window at A to
64 packets/two windows at B. All other controls are unchanged. Size order is
A/B, B/A, A/B, which is not fully counterbalanced.

| Backend | Median paired B-A H2D p50 | Median paired B-A D2H p50 |
| --- | ---: | ---: |
| KFD | +10,896 ns | +21,192 ns |
| HSA | +41,112 ns | +16,575 ns |
| HIP | -2,163 ns | -141 ns |

No approximately 1.2 ms KFD upload step was observed at the second-window
boundary. This argues against that continuation being the primary cause in this
diagnostic, but does not isolate its exact cost from drift or other effects.
Next discriminating work is matched native engine selection and host-mapping
inspection, plus separating facade CPU work from device copy time. Those remain
hypotheses and planned experiments, not measured causes or implemented fixes.

## Preserved Failure

The initial `benchmark` record intentionally remains exit 1. Its KFD and HSA
processes both passed their copy checks, but HSA postflight observed
100,961,943,552 bytes of VRAM, then 25,531,580,416 bytes at the final observation,
despite no mapped GPU 1 PID. HIP and later repetitions did not run.

A fresh read-only observation returned to the original baseline before reuse.
The successor only changed postflight cooldown from 2 to 20 seconds; guard
thresholds were not relaxed. This is consistent with deferred reclamation after
the outside-copy-timing allocation/free probe, not proof of a persistent leak or
its cause.
Initial receipts were collected before the successor overwrote remote receipt
filenames. The failed attempt is not pooled into complete campaign statistics.

## Evidence And Cleanup

`raw` contains 25 closed five-file command records. Only the initial `benchmark`
exited 1; the other 24 exited 0. `prepare.log` separately retains the clean-base
build, toolchain identities and frozen SDMA manifest unit test. Each of the four
`*-results` directories contains its collected source-before/source-after logs,
selected 5,505-file source manifest and the three benchmark executable hashes.
These selected source manifests are identical across campaigns; they are not a
complete toolchain/loader/environment closure. In particular, the root toolchain
pin file is not in that manifest; actual compiler versions are in `prepare.log`.

The initial summarizer and its output are retained as development history.
The final `summarize.py` compiles exact helper bytes bound to the archived
`check-parity.py` hash, checks context/protocol/ordered completion and rejects
incomplete campaigns. Authoritative reports are `summary-final.log`,
`summary-boundary.log` and `summary-numa.log`. Its self-test covers one settled
positive and 16 adverse records; it is not a separate exhaustive negative matrix
for each mode. The NUMA PATH-based script prototype was hashed but never run;
`numa-script-identity-final` identifies the absolute-path version actually used.

Receipts were retrieved after each campaign and before the next reused remote
receipt names. After the final retrieval, the guarded cleanup found no process
executing from or resident in the owned directory and removed only
`/home/harsh/fe2o3-copy-diagnostic-20260917.6CF3d70A` (553 MiB).
Its absence was confirmed at 18:44:48 UTC on 2026-09-17; post-cleanup GPU telemetry
again showed baseline VRAM and no mapped GPU 1 PID. No foreign jobs or files were
removed or signaled. The existing remote checkout was only read as a clone source.

`SHA256SUMS` binds the final archive. It is a development diagnostic receipt,
not formal runtime refinement, authenticated native authority, concurrency or
multi-device qualification, or a parity/performance acceptance certificate.
