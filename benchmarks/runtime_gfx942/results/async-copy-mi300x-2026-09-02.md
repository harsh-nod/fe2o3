# gfx942 Async Copy Result: MI300X, 2026-09-02

Status: `Measured` for the exact configurations below. This is hardware
evidence, not a formal proof or a general HIP/HSA parity claim.

## Provenance

- Measured source commit: `a0695a10e49ea6c4a211811e88a0f4da8ca46044`
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`
- Devices: AMD Instinct MI300X `gfx942`, indices `0,1`
- KFD unique IDs: `6ced1647a296545c`, `ab83d2ffef0d3cdf`
- Target: `gfx942:xnack-`; HSA and HIP both reported XNACK disabled
- ROCm: `7.2.4`
- Rust: `1.97.1 (8bab26f4f 2026-07-14)`
- SDMA manifest SHA-256:
  `e794d249b2d4a585a30cb4f22caa39931319784b824df344627560d4248ef914`
- Traffic: 1 MiB per copy, 10 warmups, 30 samples
- Primary run: depths 1 and 16 with directional KFD queues
- Ablations: generic queues at depths 1 and 16; engine 0 and engine 1 at
  depth 16

Raw results and SHA-256:

| Profile | Raw result | SHA-256 |
| --- | --- | --- |
| Directional | [directional](async-copy-mi300x-a0695a10e-directional.txt) | `4ff48e8f2562ba801c713c576c235728fb96310f6bd453d195d1a57f04e4539e` |
| Generic | [generic](async-copy-mi300x-a0695a10e-generic.txt) | `dcf32fd2903dde9626ba153eb3bfad3a9ec67bcc9101d62acd007f2cfff7a700` |
| Engine 0 | [engine 0](async-copy-mi300x-a0695a10e-engine0.txt) | `35c99b0e17fc869f1d4f158f8724a8f944033994237aa4f41ba13b2ab9185f8a` |
| Engine 1 | [engine 1](async-copy-mi300x-a0695a10e-engine1.txt) | `062f0c788655dad63b2e7e4a504b93a68a5cd93225abd5cf52906fc19cf0448c` |

Every phase began and ended with every GPU used by that phase at 0% load.
The runner validated a fresh per-slot pattern after every round, passed the
frozen-manifest test, matched every device identity and XNACK gate, and did not
reach a phase timeout. The checkout was clean and detached at the measured
commit. Host load was 9.15 over 96 CPUs after the runs; the runner gates GPU
load, not competing CPU work.

## Matched Throughput

The primary lane uses engine 1 for H2D and engine 0 for D2H, matching the pinned
ROCr directional policy. Throughput is GB/s from nearest-rank p50 latency.
Directions are measured serially, not overlapped.

| Scope | Depth/device | Direction | KFD split | KFD combined | HSA | HIP |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Single device | 1 | H2D | 7.663 | 8.673 | 23.128 | 22.301 |
| Single device | 1 | D2H | 7.692 | 9.652 | 40.567 | 39.391 |
| Single device | 16 | H2D | 22.201 | 21.463 | 24.522 | 31.775 |
| Single device | 16 | D2H | 26.474 | 25.498 | 37.766 | 54.332 |
| Two-device aggregate | 1 | H2D | 7.502 | n/a | 25.624 | 25.051 |
| Two-device aggregate | 1 | D2H | 7.522 | n/a | 59.796 | 65.194 |
| Two-device aggregate | 16 | H2D | 38.287 | n/a | 58.893 | 62.665 |
| Two-device aggregate | 16 | D2H | 38.278 | n/a | 68.259 | 74.545 |

At depth 16, directional KFD reaches 90.5%/70.1% of HSA H2D/D2H and
69.9%/48.7% of HIP. Two-device aggregate KFD reaches 65.0%/56.1% of HSA and
61.1%/51.3% of HIP. Copy-performance parity is therefore not established.

The combined currentness envelope improves depth-1 KFD H2D by 13.2% and D2H
by 25.5%, confirming that repeated topology/currentness validation is a
material fixed cost. At depth 16 it is 3.3%/3.7% slower in this run, so that
fixed cost does not explain the remaining saturated-bandwidth gap.

## Engine Ablation

Single-device depth-16 KFD results:

| Profile | H2D engine | D2H engine | H2D GB/s | D2H GB/s |
| --- | ---: | ---: | ---: | ---: |
| Directional | 1 | 0 | 22.201 | 26.474 |
| Generic | runtime selected | runtime selected | 22.676 | 26.827 |
| Engine 0 | 0 | 0 | 22.443 | 26.906 |
| Engine 1 | 1 | 1 | 22.203 | 24.178 |

H2D varies by only 2.1% across the four profiles. Engine 0 is 11.3% faster
than engine 1 for D2H, and the directional policy already selects engine 0.
Incorrect engine selection is not the primary cause of the remaining gap.

The most likely saturated-throughput cause is queue-level serialization. This
is an inference from the implementation and the ablation: fe2o3 places all
copies for one direction on one native queue and one engine, whereas the HIP
lane has 16 independent streams and HSA owns its internal queue scheduling.
The MI300X topology reports two ordinary SDMA engines and eight queues per
engine. Closing the remaining gap requires checked multi-queue striping and a
matched overlap benchmark, while preserving exact queue identity, completion,
and terminal-custody rules. Per-copy system fences and serial host completion
scans are secondary candidates that require separate packet-level ablations.

## Limits

- This is one sequence of runs on two devices and one ROCm/kernel stack.
- Load was gated at phase boundaries, not continuously. Competing host work and
  a competing GPU job wholly contained inside a phase are not excluded.
- The repeated HSA/HIP and two-device rows in the ablation files show run-to-run
  variance; only the primary directional file supplies the matched table.
- The harness serializes H2D and D2H on each device. It measures two-device
  concurrency but not same-device bidirectional overlap.
- It does not benchmark peer/XGMI copy. The generic runtime peer route remains
  bounded cooperative host staging, not native XGMI.
- The KFD split and combined paths differ only in currentness-envelope shape;
  HSA and HIP have no separate combined row.
- The Verus results prove their separate finite abstract model only. Rust
  validation is `Checked`; ioctl, MMIO, coherence, firmware, completion, and
  liveness assumptions remain `Contracted`; the rows above are `Measured`.
