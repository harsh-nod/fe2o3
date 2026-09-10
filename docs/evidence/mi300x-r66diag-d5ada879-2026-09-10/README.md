# R66 Diagnostic Campaign: Shared-GPU Admission Rejected

Signed source: `d5ada879bc88a70523a1a5b6ab97f7aa218cbc11`, pushed to both
remotes on `codex/r65-runtime-drain-versions`. This campaign built the new
diagnostic qualifier but did not execute it. It establishes neither native
coexistence acceptance nor a performance result.

## Outcome

The private musl release build passed in 1m 30s. All 24 recorded commands exited
zero; the runner then rejected GPU 1's prelaunch telemetry. The device identity
was unchanged, but GPU use was 100% with 44% VRAM allocated. All eight GPUs
reported activity. No `monitor-owner-0` command or qualifier output exists.
The runner's source orders this check before launching the owner binary.

The selected GPU had been idle at the initial read-only check. Foreign work
became active while the private build ran. The campaign did not retry, reset a
device, signal another workload, or select a different GPU. This rejection does
not diagnose or resolve the earlier native-roster failure in the
[first R66 campaign](../mi300x-r66-coexistence-2026-09-10/README.md).

## Independent Audit

- Trusted SSH signature accepted; captured commit and source archive exactly
  match fresh local Git output for the signed revision, including 7,047 file
  hashes.
- The new runner retained the actual binary before hardware admission. Its
  4,426,920 bytes were independently inspected with `nm`, `readelf` and the
  pure-Rust policy checker; symbol/header output matches the captured output.
  All 4,863 symbol names passed, with no dynamic dependencies or dynamic symbols.
- Production metadata audit passed with 43 packages and eight permitted build
  scripts. These checks establish the named closure policy, not machine-code
  refinement of native execution.
- Independent cleanup confirmed the exact private stage absent, all 26 recorded
  PIDs absent, all 24 process groups empty and no remaining process command line
  referencing the stage. Private cache/source/build files were removed.

The cleanup record intentionally reports `selected_gpu_idle: false`: the foreign
workload remained active. The audit permits this only for the recognized
prelaunch-busy rejection with no qualifier command. Owned-process/stage checks
remain strict, and no runtime or hardware-admission guard was weakened. We make
no claim to have cleaned up another user's GPU allocations.

## Retained Material

`raw/musl-rejected`, `outer`, `rejection-audit.json` and `cleanup.json` preserve
the command logs, source/signature manifests, build/ELF observations, telemetry,
rejection and cleanup. The wrapper, controller and independent audit scripts are
included. The initial auditor assumed a postlaunch rejection and rejected this
different trace shape; its log is retained alongside the final accepted audit.

The complete bounded capture, including the actual binary and source archive,
remains locally at `/home/harsh/.codex-tmp/r66diag-d5ada879-capture.tar` rather
than adding duplicate large binaries to Git. Its SHA-256 is
`c7030db589bd6f8920d2ac3540682bdcc7ddb939b79c213b0ffded6de11944dc`.
The binary SHA-256 is
`5fd183d66bf45ace4c16291a468ea03fbe142a4a34544a7fb332bda583fb7132`.

The next hardware step remains a newly staged signed R66 campaign during an
idle shared-GPU window. No eight-cell result, active-work drain, physical overlap
or HIP/HSA speedup follows from this record.
