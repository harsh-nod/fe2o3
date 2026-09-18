# HIP Copy-Only Smoke: Preflight Refused

Development evidence only. The single authorized attempt was refused before
launching the HIP executable. This packet does not qualify native HIP copying,
the new post-exit schedule on hardware, performance, or any accepted milestone.
There was no retry, replacement device, fallback or threshold change.

## Actual Outcome

The runner started at `2026-09-18T09:57:20.399396406Z` and finished at
`09:57:23.165658321Z`, exit 1. All four source/binary/platform/script checks before
and after the attempt passed. Exact device topology and CPU/NUMA placement passed.
The fresh complete preflight at `09:57:20.781264557Z` through
`09:57:22.857900509Z` refused:

| Observation | GPU4 busy | GPU4 VRAM bytes |
| --- | ---: | ---: |
| Direct sysfs before SMI | 1% | 734,560,256 |
| Direct sysfs between status and PID capture | 1% | 633,724,928 |
| Direct sysfs after PID capture | 0% | 633,724,928 |
| ROCm SMI status | 2% | 633,724,928 |

PID `1661511` was explicitly listed as attached to all eight GPUs, including
selected GPU4, UID `0x54f88318ca05093d`, BDF `0000:85:00.0`. Every VRAM sample
exceeded the unchanged exclusive 536,870,912-byte limit. The captured endpoint
retains all eight refusal reasons, original observer exit 1 and failed admission.
These sequential observations do not establish continuous ownership or causality.

There are ten closed remote command records. No native command exists in that
roster, and `native_launched=false`. Consequently no HIP copy, payload, native
T0, immediate post-native observation or delayed post-native observation occurred.
No timing ratios or performance results were computed. The strict
`--require-success` check correctly exited 1; the evidence-integrity audit exited 0.

## Source and Build

The only comparator/build source inputs were three exact Git blobs exported from
commit `1890a64e1911a2a346c5a9e70a8ff5ad45b19231`: the HIP comparator, its argument
header and the existing observer. `source-export.json` contains complete paths,
Git blob identities, SHA256 identities and the original tar hash. No working-tree
comparator changes or CPU mock were included. The independent payload checker was
overlaid separately at SHA256
`d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561`.

The CPU-only native build passed using real HIP/ROCr, explicit
`--offload-arch=gfx942`, CPU affinity 48,49, core limit zero and a bounded compile.
The resulting unexecuted native binary SHA256 is
`d2c012774e2321266d2708d7d7f40745d332ebbebf9d26c9c5f61c4bc57ad976`.
All 350 actual compiler dependency/library/tool files were pinned. ELF metadata,
compiler commands, dynamic dependency resolution, before/after checks and binary
hash receipts are retained; executables, libraries, the source tar and caches are
not archived. The three exported source files remain available as portable inputs.

## Protocol and CPU Checks

`PROTOCOL.md` is the pre-execution proposal approved specifically for this
one-process smoke. Its immediate endpoint permits only enumerated firmware-busy
reasons as telemetry while retaining the original refused observer result; its
fixed delayed endpoint requires all strict criteria. The actual observer's
internal monotonic start must be in T0+[0,1] seconds and T0+[20,21] seconds,
respectively. No poll-until-success or replacement endpoint is allowed. These
post-native rules were not exercised by this hardware attempt.

Thirteen final CPU test groups passed, including a complete synthetic 13-record
smoke audit, ten whole-envelope corruption cases, native failure/endpoint
retention, exact timing boundaries and no retry. An earlier eleven-group run is
also retained. A preliminary lint run failed only for an unused Python import;
the corrected final lint, format and shell-syntax checks passed. CPU fixtures do
not establish native HIP execution or production runtime authority.

There are 24 original local command receipts. Exactly three have nonzero status:
the preliminary lint failure, the actual preflight-refused attempt, and its strict
qualification-refusal check. Original bytes, command paths and timestamps remain
unchanged. `origin-files.sha256` binds all copied original staging files; later
portable audit/capture records are separately under `review/`.

## Cleanup

Only the marked owned directory
`/tmp/fe2o3-hip-smoke-1890a64e1-20260918.JA7uyMoS` was removed, after collection.
Cleanup reported 321,997 regular-file bytes removed. All ten recorded owned PIDs
and process groups were absent. The independent path-absence receipt closed at
`2026-09-18T09:58:35.638479505Z`, exit 0.

Accessible same-user executable/cwd/fd references were empty. Unreadable unrelated
`/proc` entries and their non-owned process groups are explicitly listed; this is
not a proof of complete global reference absence. No other workload was signalled,
stopped, reset, modified or cleaned up. All agent execution sessions are closed.

## Separate Driver Observation

`raw/driver-source.*` records read-only installed driver/source identities and
relevant source excerpts. The loaded/installed module srcversion matched; this is
not a reproducible-build proof. The inspected MP1 13.0.6 path returns rounded
firmware `SocketGfxBusy` through a jiffies-based metrics cache. Linux documents
busy percentage as firmware aggregate activity; ROCm documents generation-specific
sampling windows. Neither identifies the cause of any earlier transient or changes
the outcome of an earlier rejected campaign.

Sources: [Linux busy-percent documentation](https://docs.kernel.org/gpu/amdgpu/thermal.html#busy-percent)
and [ROCm SMI sampling documentation](https://rocm.docs.amd.com/projects/rocm_smi_lib/en/latest/how-to/use-python.html).

## Portable Audit

From this directory, run `python3 -B audit.py`. It reads archived files only and
does not launch tests, native executables, builds or SSH commands. `check.py` and
the original scripts remain byte-identical to their reviewed execution versions.
Build/reproduction scripts contain historical owned paths and must not be rerun
as cleanup or authorization instructions.

Independent review is complete: the primary independently ran the portable
auditor and checked the raw all-eight-GPU attachment; its successful receipt is
`review/independent-root-audit.*`. All seven review records closed with exit 0.
The archive is sealed by `SHA256SUMS`, covering every regular file except the
manifest itself. No original staging bytes were changed. This remains a refused
attempt and grants no native, performance, formal, R126, A1 or A2 acceptance.
