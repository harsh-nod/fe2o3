# Fresh Link Scratch: MI300X Comparison

The candidate reduces reader allocations, but this experiment does **not**
establish a copy-latency improvement or HIP/HSA parity. Baseline and candidate
process-level latency ranges overlap. Fresh topology discovery remains dominant.

## Results

`native2` completed the declared twelve process trials on GPUs 5 and 6. All
complete-pattern/guard checks, explicit teardown paths and seventy-two endpoint
observations passed. The complete remote inventory was collected before owned
cleanup, followed by a separate successful path/process absence check.

The following ranges span each process/direction's own reported summary, with
two process invocations and both directions per row. They are not pooled sample
percentiles, confidence intervals or independent sixty-process experiments.
Only diagnostic-off results appear here. Each copy is one MiB at depth one.

| Producer | p50 Range (us) | p95 Range (us) |
| --- | --- | --- |
| KFD baseline | 14304.621-14387.585 | 14445.791-14877.988 |
| KFD scratch candidate | 14319.261-14347.637 | 14428.534-14471.969 |
| HSA | 30.155-30.806 | 30.365-31.137 |
| HIP | 38.347-38.678 | 38.908-39.789 |

The separate diagnostic population contains four transcripts, each with eighty-
two attributed records plus a summary: prime, ten warmups and thirty samples in
both directions. Across its eight process/direction cells, nearest-rank medians
of each sampled record's `(opening + closing topology discovery) / backend total`
are 94.630-94.681 percent. Diagnostic timings are not used in the table or in
HIP/HSA comparisons. No latency improvement is accepted from overlapping
summaries of this small shared-host experiment.

## Source And Validation

The signed baseline is `a852464aee976a1ff1472f2e477e25b6cf2ac933`; the signed
candidate is `1e13a90cd5e604a4612b079b48f629005a0a047b`. Its existing
[CPU packet](../dev-topology-link-scratch-cpu-2026-09-24/README.md) remains the
CPU qualification authority, with exact matching candidate source inputs.

Both cohorts were rebuilt in distinct, initially absent Cargo target directories.
Both passed the same thirteen release benchmark tests. The retained KFD ELFs are:

- Baseline: `213ff297b071b2a4e7b65e8728ed80705a10b5e36c227e00149f4579dc775c41`.
- Candidate: `58e7079bb60d2bba2aa9ba120afa76e88b77b69df9e8b23e2a20e982cf5a775f`.

The frozen prelaunch protocol qualification passes 57 tests. The replay checker
adds sixteen mutation tests. `qualification1` records the first successful
replay before this README; `qualification2` brackets the final top-level packet
files. The replay verifies twenty-five local and ninety-two remote command
receipts, signed source/archive maps, cold build environments, tool continuity,
CPU binding, protocol identity, parsed results, settling delays and cleanup.
These checks replay records; they do not rerun hardware or prove compiler output.

HIP/HSA ELF bytes are not retained. Their hashes are recorded and checked by
the native controller before/after execution; unlike the KFD artifacts, they
cannot be independently rehashed from this packet.

## Rejected History And Retention

`native1` is rejected in full. Its shared Cargo target reused the baseline ELF
for the candidate. The identical retained ELF bytes and build logs demonstrate
the error. The primary-agent rejection annotation records the targeted controller
stop. Retained receipts independently show collection, owned removal and absence.
No native1 timings contribute to any accepted comparison.

`archive-index.json` binds the compressed archives and every uncompressed file:
205 native1 files, 420 native2 files and twenty protocol-qualification files.
The archives retain the original byte streams, including full signed-source
tar outputs, payloads, logs and historical protocol copies. Duplicate extracted
source trees and Cargo caches are excluded and removed locally; their cleanup
records and runner are retained separately. Foreign files and workloads were
not removed.

## Replay And Limits

In the original workspace, with its pinned trusted SSH signer list available:

```sh
python3 -I -B docs/evidence/dev-topology-link-scratch-mi300x-2026-09-24/verify.py
```

The captured campaign retains historical absolute paths. Authenticate this
packet's signed Git commit before executing its scripts. See
[PROTOCOL.md](PROTOCOL.md) for exact controls, command bounds, comparison surfaces
and shared-host limitations. A signed packet is not a hermetic compiler,
library, driver or firmware snapshot, and idle point observations are not an
exclusive reservation.

No whole-runtime formal refinement, total retained-memory bound, A1/A2 closure,
A7 performance acceptance or general HIP/HSA parity is claimed.
