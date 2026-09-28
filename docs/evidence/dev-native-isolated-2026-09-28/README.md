# Isolated Native Async Cells: Six Pass, Depth Setup Fails

The complete seven-cell campaign is **not qualified**. Six exact MI300X cells
pass; the final depth cell fails before publishing its dispatches. This packet
preserves that failure and the subsequent CPU-tested fixture correction. No
milestone, formal native refinement or HIP/HSA performance claim is promoted.

## Source And Build

- Runtime source: signed `dda812658d8827e7d508389c9650228844d0f503`.
- Frozen qualification helpers: signed `d65a15d4b9d3366a810a9859f4f7afe506979b24`.
- Target: `x86_64-unknown-linux-musl`; no default features; `scale-qualification`.
- Optimized test profile, with assertions and overflow checks enabled.
- Retained ELF: 39,105,264 bytes, SHA256
  `0ac04fa80c5a042670a6aa40d190d4dd7f287c56a2ba7c21d213f01241e76767`.
- Actual-ELF CPU suite: **1763 passed, zero failed, 29 ignored**; roster 1792.
- GPU1: UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, NUMA0/CPUs0-47.

`verify_build.py` joins the signed Git archive, observed post-build source,
Cargo artifact, CPU-tested executable and whole retained ELF. This is not a
before/after measurement of every build-tool binary.

## Native Results

| Exact cell | Result and scope |
| --- | --- |
| Short profile | Pass; complete 384-byte output, backing refund and inert shutdown |
| Long profile | Pass in its own process; same output and cleanup checks |
| Owner progress | Pass; later Short completes while the exact earlier Long receipt still polls Pending on another physical queue |
| Timeout | Pass; observer timeout recovers the same operation and its full output |
| Dropped observer | Pass; owner progress completes after its result observer is dropped |
| Command backpressure | Pass; host admission rejection refunds resources, issues no rejected launch and permits a later new launch |
| Retained depth | Exit 101 during setup; zero passing depth tests and no depth publication/output markers |

The six passing cells independently replay complete bytes and, where emitted,
publication/completion/release profiles. They do not establish physical GPU
overlap, execution cancellation, native-slot saturation or broad kernel support.

The final cell fails at the original `native_depth.rs:261:75`:

```text
called `Option::unwrap()` on a `None` value
```

Its fixture requested a 128 MiB backing budget with **512 allocation records**.
`Gfx942HostVisibleBackingBudgetV1::new` admits at most **256**, so setup fails
before streams, runtime allocations or dispatches are created. This is not evidence of a
2048-dispatch runtime capacity failure. The intended depth counts retained
receipts, not kernels known to be simultaneously unfinished.

## Observation And Cleanup

All **21 strict endpoint observations** pass, including both postflights after
the failed depth cell. Raw sysfs GPU/memory activity is zero, the selected GPU
has no attached PID, and VRAM stays below the fixed 512 MiB admission limit.
Observed VRAM ranges from 312,819,712 to 312,844,288 bytes; exact baseline
restoration is not claimed. Endpoints are not an exclusive reservation or a
continuous isolation measurement.

All **30 remote command groups** and eight local command groups close. Complete
results, executed ELF and helpers are collected byte-exactly before marker-bound
cleanup and independent path/process absence. The owned runtime directory is
removed:

```text
/home/harsh/fe2o3-native-isolated-20260928.d00e126a8f598349
```

The owned build directory is separately removed after source/results inventory
verification, with an independent absence check:

```text
/home/harsh/.codex-tmp/fe2o3-native-isolated-build-20260928.oNijXsc5
```

No foreign process, GPU, cache or directory was reset or removed.

## Correction And Next Run

Signed `96adb54316ac5b1038cfc9ce3736cd60dea475f8` changes only the fixture's
backing configuration and its CPU regression. A typed, compile-time-checked
128 MiB/256-record budget replaces the invalid literal. Six shared data buffers
do not require one backing allocation per retained receipt. The production
256-record limit is unchanged.

All ten focused depth-checker tests pass, including exact configuration and
original-512-refusal checks. The corrected all-feature runtime suite passes
**1817 tests, zero failures, 29 hardware ignores**; formatting and diff checks
pass. No corrected-source native depth pass, new Verus proof or new Clippy run
is claimed.

Next build and qualify the corrected source with a fresh ELF/roster binding and
new owned paths. The new protocol and its synthetic shutdown fixtures must
expect 256 records, not the preserved historical 512. Keep these signed helpers
unchanged. A1/A2, actual unfinished-operation depth, aggregate accounting,
Context/journal proof composition, protected Worker execution, same-process
fresh native readmission and matched HIP/HSA performance remain open.

## Retention And Replay

`raw.tar.gz` contains all **302 files**, including source, build/CPU records,
complete payload copies, every native command/endpoint, cleanup, calibrations,
correction tests and the fresh issue #182 API observation (still open).
Archive size is 46,506,901 bytes; SHA256:

```text
6a15bb07d680104ac51bf4fd8c77f19c499171190dfa228e36bd0c13b65d4225
```

`artifacts.json` inventories every extracted file. `archive.json` records the
successful byte-exact restore and stopped-run replay from a fresh temporary
directory. With `RAW` naming that extraction, run from the repository root:

```sh
python3 -I -B docs/evidence/dev-native-isolated-2026-09-28/verify_stopped.py "$RAW/campaign1"
python3 -I -B docs/evidence/dev-native-isolated-2026-09-28/test_stopped.py "$RAW"
```

Expected classification: `qualified: false`, six harness passes, one failed
cell, 21 strict endpoints, 30 remote commands and owned cleanup. Seven resealed
negative controls reject altered completion, status, output, panic location,
process custody and absence claims. The full success verifier rejects this run.
Synthetic positive transcripts remain parser calibration, not hardware evidence.
