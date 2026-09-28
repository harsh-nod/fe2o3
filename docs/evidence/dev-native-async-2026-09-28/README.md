# Native Async Qualification: Stopped At Session Re-Admission

This packet is **not qualified**. It preserves a real MI300X failure rather than
reclassifying Short's partial success as a passing test or retrying unchanged
source until it passes. No milestone or HIP/HSA parity claim is promoted.

## Attempt And Result

- Runtime source: signed `89c16e1e484d1af8e6d7233c33d0660aeaa2f219`.
- Qualification helpers: signed `6dd11486f26bfef77ba3033a3bd35c5e6676af9d`.
- Target/features: `x86_64-unknown-linux-musl`, no default features,
  `scale-qualification`, optimized test profile with assertions/overflow checks.
- Actual retained ELF SHA256:
  `ec7a39fe08d4b84c00ba2e85e027db8f347005fd2aae78ed2965862224199ee6`.
- The fresh ELF's complete CPU suite passed **1763**, with zero failures and
  **28** hardware ignores; its exact roster contained **1791** tests.
- GPU1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, NUMA0/CPUs0-47.
  Other GPUs had active work and were not used.
- The first of six planned cells exited **101**, with **0 passed, 1 failed**.
  The owner, timeout, dropped-observer, command-backpressure and retained-depth
  cells were **not run**.

The first cell's Short dispatch produced all 384 expected bytes, including both
guards. Its ordinary native shutdown refunded backing to zero. The same test
then attempted to open a fresh backend for Long in the same process and failed:

```text
model generation admission failed: ActiveDeviceExists(
    DeviceKeyV1 { physical: PhysicalDeviceIdV1(12358953799103888607),
                  generation: DeviceGenerationV1(1) })
```

Successful Linux `ACQUIRE_VM` deliberately disables device-model retirement on
Drop. Queue destruction and backing refund do not undo that process-lifetime
admission. The fixture's loop violated the current native-session contract;
clearing the registry or forcibly retiring that generation would weaken the
safety boundary, not fix the qualification.

## Safety And Evidence

The runner stopped at the first failure. Its admitted preflight and both fixed
postflights passed independent raw sysfs/SMI/PID checks. GPU and memory busy
were zero; selected VRAM remained 312,733,696 bytes; no selected GPU PID was
present. Immediate/delayed observations began about 0.031/20.030 seconds after
the failed test process group was reaped. These are endpoint observations, not
an exclusive reservation or continuous isolation claim.

Six remote command groups and eight local command groups closed. The complete
remote results, whole executed ELF and helpers were collected byte-exactly
before marker-bound cleanup and independent path/process absence. The exact
owned runtime directory was removed:

```text
/home/harsh/fe2o3-native-async-20260928.7dd73f7238bb4f04
```

The distinct owned build directory was also removed after complete collection.
Three conservative build-cleanup refusals are retained alongside the successful
fourth cleanup and the separately observed absence. No foreign process, GPU,
cache or directory was reset or removed.

`verify_build.py` joins the retained archive to signed Git source, constrains
Git signature verification, checks build argv/environment, joins Cargo's exact
artifact to the compressed retained ELF, and checks the complete actual-ELF CPU
roster. This is not a before/after measurement of every build-tool binary.

The six-cell success checker was calibrated with full synthetic transcripts,
6179-event depth profiles and a 26-command native replay. Resealed mutations
exercise lifetimes, identities, ordering, full output bytes, endpoint activity,
process custody and deadlines; archived code must authenticate before execution.
Those fixtures are not hardware evidence. `verify_stopped.py` independently
replays this actual failure and requires `qualified: false`; `verify.py` must
reject it as an incomplete success campaign.

## Correction And Remaining Work

The fixture correction splits Short and Long into distinct ignored tests that
share the same implementation and retain every output, release, refund and
inert-shutdown assertion. The runner must select each test in a separate process,
making the next matrix **seven cells**. The original failing source, binary and
records are retained unchanged.

Focused CPU regressions additionally require exact `ActiveDeviceExists` refusal
without history mutation and preservation of an active VM after full allocation
release. They do not constitute a Linux VM-retirement proof.

The correction is signed as `e5e18c44d76287816d35e266c988310ae693ef83`.
Four focused model/memory/VM-registry tests and the all-feature runtime library
suite pass: **1816 passed, zero failed, 29 hardware-only ignores**. Workspace
formatting and diff checks pass. No new Verus, Clippy or corrected-source GPU
qualification is claimed by these test-only changes. The stopped verifier
passes twice on the preserved records, rejects five independently resealed
mutations, and the success verifier rejects the failed native command.

Fresh native validation of the corrected seven-cell matrix remains required.
Same-process native-session reinitialization is still unsupported and remains a
parity gap; process isolation of a qualification fixture does not implement it.
Worker/compiler production refinement, native high-depth/out-of-order execution,
aggregate accounting, measured overlap and matched HIP/HSA performance gates
remain open. The 2048-depth fixture, when run, counts retained receipts, not
2048 simultaneously unfinished kernels.

## Retention And Replay

`raw.tar.gz` contains all 220 retained files, including the original source
archive, the CPU-tested ELF, both complete payload copies, build and command
records, calibration logs, failure transcripts, cleanup records and correction
qualification. Its SHA256 is:

```text
de8fc2a7e81aba3223c3a1eb6294e702e3e801529a6f4e57a7063c044b08596c
```

`artifacts.json` inventories every extracted file; `archive.json` records the
archive identity and successful replay from a fresh temporary extraction. With
`RAW` set to that extraction directory, run from the repository root:

```sh
python3 -I -B docs/evidence/dev-native-async-2026-09-28/verify_stopped.py "$RAW/campaign1"
python3 -I -B docs/evidence/dev-native-async-2026-09-28/test_stopped.py "$RAW"
```

Expected classification is `qualified: false`, one failed cell, five unrun
cells, three strict endpoint observations and owned cleanup. The synthetic
success checker remains available for a future complete, newly bound campaign;
this historical six-cell protocol must not be used unchanged for the split
seven-cell test roster.
