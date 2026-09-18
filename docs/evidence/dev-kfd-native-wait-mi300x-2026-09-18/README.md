# Native Wait Diagnostic: Interrupted MI300X Attempt

Source: `fcd5a89a113c6538338598bfcdb6769fb5c06042`.

**Not an accepted performance campaign.** Only legacy KFD cell A/1 ran.
It validated all 13 returned 256 MiB buffers and completed explicit teardown,
then its postflight occupancy check failed. No native-wait B/C or HSA D process
launched. There are zero performance-qualified processes, no matched ratios,
and no new native-wait hardware qualification from this attempt.

The user permitted use of free GPUs on shared `mi300x`; there was no exclusive
reservation. GPU 4 (`0x54f88318ca05093d`, `0000:85:00.0`) passed fresh identity,
utilization, VRAM, and PID checks immediately before A/1. The runner stopped
on the failed postflight and did not retry or migrate the campaign.

## Observed Outcome

All times are UTC on 2026-09-18:

| Check | Time | GPU 4 use | GPU 4 VRAM bytes | Result |
| --- | --- | ---: | ---: | --- |
| A/1 preflight | 05:45:31.356058409 | 0% | 298,647,552 | Admitted |
| A/1 postflight | 05:46:11.393124220 | 2% | 633,720,832 | Refused |
| Final guard | 05:46:12.584827251 | 1% | 652,746,752 | Refused |

Admission required 0% GPU use, less than 536,870,912 VRAM bytes, and no listed
process using GPU 4. The later utilization/VRAM checks failed before PID queries.
Increased activity is visible on other GPUs too, but neither the responsible
process nor the cause of the change is established. This is not evidence of
a KFD leak or a specific external workload. A/1 timings remain raw observations
only, not isolation-qualified performance results.

The benchmark process exited 0; the campaign exited 1. Final source checks,
binary checks, and source-worktree cleanliness all passed. Cleanup removed only
our marked directory `/tmp/fe2o3-kfd-native-wait-20260918.LiBKebIz` (409 MiB),
after checking for live executable/cwd references. Absence was confirmed at
05:52:17.781388127. No other work or temporary files were removed.

## Intended Comparison

Four counterbalanced blocks: `ABDC`, `BCAD`, `CDBA`, `DACB`; depth 1, three
warmups and ten samples per process, CPUs 48-95 and memory node 1:

- A: legacy KFD `diagnostic-slice50us`.
- B: profiled native KFD `diagnostic-native-sleep1ms`.
- C: profiled native KFD `diagnostic-native-sleep25us`.
- D: HSA fine-grained host pool, mask 2, nearest CPU agent 1.

B/C would change only the requested sleep ceiling within the same instrumented
full-deadline route. A also changes facade re-entry and instrumentation.
HSA mask 2 is not established as physically equivalent to KFD engine selection.
Requested sleep and host scan/CPU measurements are not DMA durations. There is
no HIP cell in this targeted diagnostic and no formal-verification claim.

The release Rust diagnostic and unchanged HSA comparator built on MI300X with
ROCm 7.2.4. `prepare.sh`, `run.sh`, and their receipts preserve flags, toolchain,
environment, NUMA placement, bounded timeouts, and source/binary identities.
Native profiled execution remains unvalidated by this archive. Prior CPU
qualification is in `../dev-kfd-native-wait-cpu-2026-09-18/` and is not promoted
to hardware evidence here.

## Verification

`interrupted.py` checks the exact single-process interruption, three GPU
snapshots, legacy payload, and terminal failure tuple, without emitting timing
metrics. `summarize.py` requires the complete 16-process campaign and refuses
this archive. It pins its imported helper, checks 63+2 native-window identities,
counters and timing enclosure, and rejects unexpected runner output.

Eight CPU-only parser tests cover synthetic B/C success, CPU availability,
malformed metadata/counters, synthetic full-campaign parsing, and interrupted
or corrupted output. Their synthetic success is not native execution evidence.
The fixture's historical HSA payload is used only for schema testing, not for
a comparison with this attempt. `audit.py` binds receipts, source hashes to the
fixed Git commit, unchanged binaries, parser results, and cleanup.

Recheck without executing any GPU work:

```sh
sha256sum --check --quiet SHA256SUMS
python3 -I test_reports.py -v
python3 -I audit.py
```

`raw/summary-refusal.exit` and `raw/summary-refusal-final.exit` are deliberately
1. Raw receipts are preserved byte-for-byte. This archive does not close A1,
A2, issue #182, or HIP/HSA parity; the next native step is a fresh guarded B/C
qualification and matched campaign when an idle window remains stable.
