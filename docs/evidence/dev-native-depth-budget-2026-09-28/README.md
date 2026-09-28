# Native Async And Retained-Depth Qualification

All **seven exact MI300X cells pass**, and complete independent replay succeeds.
This qualifies the named opt-in fixtures, including the 2,048-retained-epoch
hardware witness. It does not complete SCALE-CAP/SCALE-2 as a whole, A1/A2,
formal native refinement or HIP/HSA parity.

## Source And Build

- Runtime source: signed `4bc0804b7f6d818e4c117b470d79b7a51d0084fb`.
- Qualification helpers: signed `9df14c700e3fd4dafa20d824a56b183d0a87e507`.
- Target: `x86_64-unknown-linux-musl`, no default features, `scale-qualification`.
- Optimized test profile with assertions and overflow checks enabled.
- Retained executed ELF: 39,250,096 bytes; SHA256
  `de591f2035bb56b63388121b4ddd6d66e0322cd4aac40a061a16aaf291632f9c`.
- Actual-ELF CPU suite: **1764 passed, zero failed, 29 ignored**; roster **1793**.
- GPU1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, NUMA0/CPUs0-47.

The build checker joins signed Git source, the exact archive, remote post-build
source inventory, Cargo's artifact, CPU-tested binary and both retained payload
copies. This is not a before/after measurement of every build-tool binary.
No new Verus or Clippy run is claimed. The preceding budget correction's separate
1817-pass all-feature CPU campaign remains in the earlier packet.

## Native Cells

| Cell | Passing scope |
| --- | --- |
| Short profile | All 384 bytes including guards, backing refund, inert shutdown |
| Long profile | Same checks, in a separate process respecting process-lifetime VM admission |
| Owner progress | Later Short completes while the exact earlier Long receipt still polls Pending on another physical queue |
| Timeout | Observer timeout recovers the same published operation and complete output |
| Dropped observer | Autonomous owner progress completes after the result observer is dropped |
| Command backpressure | Bounded command rejection refunds admission, issues no rejected launch and permits a later new launch |
| Retained depth | 1024 original native receipts per lane, separate runtime/native saturation checks, output and cleanup |

Timeout is not GPU cancellation. Command backpressure is not native-slot
saturation; the depth cell supplies a separate native saturation witness.
The mixed-duration cases do not measure simultaneous physical execution.

## Retained Depth

The opt-in Qualification1024 profile retains **2048 distinct native publication
receipts** across two native queues before any completion poll/readback. Each
receipt is associated with its exact runtime submission, lane, recipe, allocation
roster and predecessor. The exported membership joins **6179 complete profiler
events**, with no dropped events. All publication events precede the first
completion event, and release follows completion.

The fixture validates every byte of six 4 MiB buffers, then emits their full-buffer
hashes. Replay independently reconstructs the expected hashes and validates the
complete profile/receipt joins. Runtime per-allocation custody and native epoch
capacity each reject one further submission per lane without changing the
retained state. The private generation, ownership and reservation comparisons
remain assertions in the signed executable, not independently exported receipts.

The host-table payload account peaks at **4,489,216 bytes and 10 records**.
Source assertions distinguish the two remaining runtime tables after native
shutdown from zero records after backend destruction. The ordinary coherent
backing account uses the corrected **128 MiB/256-record** budget and refunds its
last 532,480 bytes/three records to zero. These accounts do not cover aggregate
process/GPU memory or every native allocation profile.

`unfinished_gpu_count` is explicitly `null`; physical overlap is `unmeasured`.
Retained publication receipts are not necessarily unfinished kernels or
simultaneously running kernels. Default64 is unchanged, and the scaled path
still admits only the exact HostVisible vecadd fixture. This run does not qualify
generated kernels, repeated scaled reuse/rebinding or high-depth async-owner
integration.

## Safety And Cleanup

All **21 strict endpoint observations** pass: three sysfs samples per endpoint,
successful raw SMI/PID capture, zero selected GPU/memory busy, no selected PID,
and VRAM below the 512 MiB limit. All 63 captured sysfs samples report
312,930,304 VRAM bytes. Postflight starts are within the fixed T0+[0,1] second
and T0+[20,21] second windows after each test group is reaped. These are endpoint
observations, not an exclusive reservation, continuous isolation or general leak
proof.

All **30 remote command groups** and eight local campaign groups close. Complete
byte-exact collection precedes marker-bound cleanup and independent path/process
absence. The owned runtime directory is removed:

```text
/home/harsh/fe2o3-native-depth-budget-20260928.f3f310d2ed9ab2d4
```

The build directory is separately removed after inventory verification, with an
independent absence check:

```text
/home/harsh/.codex-tmp/fe2o3-native-depth-budget-build-20260928.TCkCYmH5
```

No foreign process, GPU, directory or shared cache was reset or removed.

## Retention And Replay

The earlier [session re-admission failure](../dev-native-async-2026-09-28/README.md)
and [invalid depth-budget failure](../dev-native-isolated-2026-09-28/README.md)
remain unchanged. This is a newly built campaign, not a reinterpretation or
unchanged retry of either failure.

`raw.tar.gz` retains **286 files**, including the source archive, executed ELF,
build/CPU records, whole payloads, all native command/endpoint records,
calibration, cleanup and closing replay. Archive size: 47,323,366 bytes; SHA256:

```text
d292807b561cf00dcb9815288c56cf568fd6f3af2f1daa0c4d848ee8c904b45c
```

`artifacts.json` inventories every extracted file. `archive.json` records the
byte-exact restore and successful replay from a fresh temporary extraction.
From the repository root, with `RAW` naming the extraction:

```sh
python3 -I -B docs/evidence/dev-native-depth-budget-2026-09-28/verify.py "$RAW/campaign1"
python3 -I -B docs/evidence/dev-native-depth-budget-2026-09-28/test_profile.py
python3 -I -B docs/evidence/dev-native-depth-budget-2026-09-28/test_protocol.py
```

Expected replay: seven harness passes, 21 strict endpoints, 30 remote commands,
1764 CPU passes and owned cleanup. Six profile groups and eight protocol groups
calibrate complete outputs, identity/lifecycle/order mutations, full synthetic
native replay, pre-import authentication and the historical invalid 512-record
budget. Synthetic fixtures are not additional GPU executions.

Protected Worker/compiler refinement, actual Context/journal proof composition,
aggregate accounting, reuse/owner scale, measured overlap and matched HIP/HSA
baselines remain open. Issue #182 was freshly observed open. No performance
acceptance, general runtime parity or broader accepted-lane promotion follows.
