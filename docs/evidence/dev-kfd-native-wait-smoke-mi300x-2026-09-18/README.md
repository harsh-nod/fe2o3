# Native Wait Diagnostic: MI300X Success-Path Qualification

Source: `fcd5a89a113c6538338598bfcdb6769fb5c06042`.

The two previously CPU-qualified native-wait modes now completed an actual
MI300X success-path run. Both validated every returned byte in 13
256 MiB round trips and completed explicit resource teardown. This is native
diagnostic coverage, not a matched performance campaign, formal refinement,
fault-path qualification, R126 acceptance, or HIP/HSA parity.

| Mode | Validated rounds | Native window records | CPU observations | Largest requested sleep |
| --- | ---: | ---: | ---: | ---: |
| `diagnostic-native-sleep1ms` | 13 | 52 | 52 available | 1,000,000 ns |
| `diagnostic-native-sleep25us` | 13 | 52 | 52 available | 25,000 ns |

Every direction used two windows of 63 and 2 packets, with exact submission,
prefix, host/device offset and length matching. Each direction made two public
waits and two flushes. The report validates all 67 rows per process, scan/pause
counter identities, requested-sleep bounds, CPU-status representation, and
scan-time enclosure within the corresponding host progress interval.

These observations confirm that the real Context continuation reaches the
profiled native wait, captures all four windows per round, and permits final
diagnostic extraction after teardown. Observed requested sleeps are not actual
scheduler sleep durations or DMA timings. The host timestamps remain raw data;
the report emits no latency statistics, speedup ratios, or HSA/HIP comparison.
No production wait defaults changed.

## Shared-Host Protocol

GPU 4: UID `0x54f88318ca05093d`, BDF `0000:85:00.0`, gfx942, CPUs 48-95 and
memory node 1. The user permitted available GPUs; no exclusive reservation was
claimed. Identity, 0% utilization, VRAM below 536,870,912 bytes, and absence of
a mapped GPU-4 PID were checked before and after each process and at final
closure. These are endpoint checks, not a continuous isolation guarantee.

The run was 2026-09-18 06:02:31.865740185 through 06:03:59.157002493 UTC.
B/1 ms ran first, followed by C/25 us. Each process had three warmups and ten
samples, a 180-second timeout, and a 20-second postprocess cooldown; the complete
runner had a 480-second timeout. All five guards and all process/final exits
passed. There was no replay of the earlier interrupted matched campaign.

`prepare.sh` and `guard.sh` are reused byte-for-byte from the separately sealed
`../dev-kfd-native-wait-mi300x-2026-09-18/` archive. The actual release Rust
example was rebuilt with `hardware-diagnostic`, the frozen lockfile and recorded
nightly toolchain on ROCm 7.2.4. That shared preparation also builds the unchanged
HSA comparator, but it was **not executed** in this smoke run. No HIP process ran.
Source, executable, script and inherited-environment receipts are preserved.

Cleanup removed only the owned marked directory
`/tmp/fe2o3-kfd-native-wait-20260918.UB2Je4Dz` (409 MiB), after checking live
executable/cwd references. Absence was confirmed at 06:05:12.894639815 UTC.

## Verification

- `report.py` pins the prior native-schema validator and requires exact B/C
  ordering, five successful guards, complete payloads and terminal success.
- Five CPU-only parser tests cover both valid synthetic modes, reordered or
  incomplete events, missing final guard, unexpected output, malformed window
  identity/counters, selected-GPU occupancy, and refusal of prior interruption.
- The first test run is preserved as failed: its intended busy-GPU mutation
  changed only GPU 0, which correctly does not invalidate GPU 4 admission. The
  corrected fixture uses structured JSON to test GPU 0 acceptance separately
  from GPU 4 refusal. No runtime or parser behavior changed for this correction.
- `audit.py` binds all 5,532 source hashes to the fixed commit, verifies exact
  before/after source and binary checks, script identities, receipt commands,
  causal timestamp ordering, actual observations, test results and cleanup.

Recheck locally without GPU execution:

```sh
sha256sum --check --quiet SHA256SUMS
python3 -I test_report.py -v
python3 -I report.py
python3 -I audit.py
```

The earlier failed campaign remains separate and unaccepted. The next
performance gate is a fresh, counterbalanced, complete-output KFD/HSA comparison
with stable guarded occupancy. A1/A2, issue #182, native failure matrices,
full surface qualification and parity remain open.
