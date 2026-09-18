# Native Wait Matched Campaign: Shared-Host Interruption

Source: `ecad7245fa9fa051daf3a9cb3cd82724da5446eb`.

**No accepted performance comparison.** The first legacy KFD process, A/1,
completed thirteen validated 256 MiB round trips and explicit teardown. Its
postflight guard then detected a process attached to GPU 4. The runner stopped;
no native-wait B/C or HSA D process launched. There is no HIP cell, matched ratio,
speedup, native-wait coverage or new formal qualification in this archive.

The user permitted using free GPUs on shared `mi300x`, not an exclusive
reservation. Fresh checks admitted GPU 4 (`0x54f88318ca05093d`,
`0000:85:00.0`) with zero utilization, less than 512 MiB VRAM usage and no
listed attached process. The same guard ran before and after each process.

## Observations

All times are UTC on 2026-09-18:

| Observation | Time | GPU use | GPU 4 VRAM bytes | Outcome |
| --- | --- | --- | --- | --- |
| A/1 preflight | 06:44:20.526334846 | 0% | 298,647,552 | Admitted |
| A/1 postflight | 06:45:00.246302343 | 0% | 298,659,840 | Refused: attached PID |
| Final guard | 06:45:02.475798667 | 0% | 648,527,872 | Refused: VRAM threshold |

The postflight PID roster lists PID 536718 attached to all eight GPUs. A separate
read-only follow-up names it `host_queue_publ`, with eight attached devices and
5,161,283,584 aggregate process VRAM bytes. This is not our benchmark executable.
It was not signaled, inspected beyond ROCm's process telemetry, or otherwise
modified. The observations establish attachment after A/1, not whether the
process overlapped its measured interval or caused particular timing values.

The benchmark process exited 0; the campaign exited 1. Both final source and
binary checks and source-worktree cleanliness passed. Cleanup removed only our
marked directory `/tmp/fe2o3-kfd-native-wait-matched-20260918.tS8fnkzm`, reported
as 410 MiB at removal. Its absence was confirmed at 06:46:08.244134372 after
checking for live executable/cwd references. No remote benchmark job remains.

## Protocol And Limits

The intended four-cell comparison preserves the earlier counterbalanced blocks
`ABDC`, `BCAD`, `CDBA`, `DACB`, depth 1, three warmups and ten measured samples
per process, CPU affinity 48-95 and memory node 1:

- A: legacy KFD `diagnostic-slice50us`.
- B: instrumented KFD `diagnostic-native-sleep1ms`.
- C: instrumented KFD `diagnostic-native-sleep25us`.
- D: HSA fine-grained host pool, requested mask 2 and nearest CPU agent 1.

B/C change the requested sleep ceiling within one instrumented native-wait
path. A also changes facade re-entry, wait budgeting and instrumentation.
HSA pool/mapping and engine-mask selection are not physical equivalence to
KFD's native allocation/engine route. The protocol pairs process medians by block
and does not treat forty pooled samples as independent process repetitions.
Host timings are not device DMA durations. This interrupted attempt reports
none of those performance comparisons.

The release Rust diagnostic and unchanged HSA comparator were built on MI300X
with bounded build resources and a private target directory. All 5,534 source
files are pinned to the fixed Git commit; exact compiler commands, tool versions,
source manifests, executable hashes, NUMA placement and environment observations
are retained. Remote build and local parser work overlapped; this is not a claim
that every receipt was executed serially. The native execution/inspection/cleanup
chain is ordered and independently checked.

## Verification

`summarize.py` imports the hash-pinned complete sixteen-process protocol and
changes only the campaign source and owned-path identities. It refuses this
incomplete archive. `interrupted.py` instead consumes the exact closed A/1
transcript, validates its payload, failed PID/VRAM checks and terminal tuple,
and emits no timing ratios. Twelve CPU parser tests cover inherited synthetic
full campaigns, identity substitutions, malformed counters and the actual
interruption. Synthetic fixtures are not new hardware results.

`audit.py` checks all recorded commands and dispositions, the native dependency
ordering, unchanged source/binary/harness identities, interruption report,
follow-up process observation and owned-directory removal. Python lint/format
and ShellCheck pass. The benchmark and full-summary refusal are intentional
exit-1 records; they are not converted into successful qualification.

Recheck without GPU work from this directory:

```sh
sha256sum --check --quiet SHA256SUMS
python3 -I test_reports.py -v
python3 -I audit.py
```

R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain accepted. A1/A2, #182,
protected application execution and HIP/HSA parity are unchanged. The earlier
successful native-wait smoke result remains separately scoped; neither it nor
this interrupted campaign supplies a matched performance result.
