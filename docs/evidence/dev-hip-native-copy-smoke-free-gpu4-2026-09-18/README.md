# Development: Fresh GPU4 HIP Copy Smoke

Status: the separately authorized one-process native smoke completed under the
predeclared fixed-deadline protocol. Portable and independent evidence reviews
passed; this archive is sealed by `SHA256SUMS`. This is comparator qualification,
not performance or runtime milestone acceptance.

## Result

The newly built real HIP comparator ran once on MI300X GPU 4, UID
`0x54f88318ca05093d`, BDF `0000:85:00.0`, bound to CPUs 48-95 and NUMA node 1.
It copied 256 MiB H2D and D2H at depth one for three warmups and ten samples.
All 13 changing-pattern rounds validated all 268,435,456 bytes. The exact final
row records three allocation releases and one stream destruction; native exit
was zero and stderr was empty. No ratios or performance conclusions are made.

The complete runner interval was 2026-09-18 10:23:43.755690708 through
10:24:19.045048799 UTC. All 20 local command receipts closed with exit zero.
The 13 remote command receipts contain twelve zero exits and one original
immediate-observer exit one, preserved below.

| Endpoint | Start relative to native reap | Original result | GPU4 observations |
| --- | --- | --- | --- |
| Fresh preflight | Before launch | Admitted, exit 0 | No selected PID, idle, 298,647,552 B VRAM |
| Immediate | +0.033526987 s | Refused, exit 1 | Direct busy 3%, 0%, 0%; SMI busy 0%; no selected PID |
| Fixed delayed | +20.030470296 s | Admitted, exit 0 | Idle, no selected PID, 298,647,552 B VRAM |

All nine direct VRAM readings and all three selected SMI VRAM values were
298,647,552 bytes. The immediate reason was exactly `sysfs-before-busy`.
Its original `endpoint_admitted=false`, refusal footer and exit one are not
rewritten. The prospectively reviewed protocol permits only enumerated
busy-only reasons at this immediate telemetry endpoint; all identity, capture,
PID and VRAM faults would reject. The single delayed endpoint had to start in
T0+[20,21] seconds and strictly pass. There was no retry or replacement sample.
T0 is the recorded timestamp immediately after reaping the native process;
absence of its owned process group is separately required.

These sequential endpoints are not continuous monitoring, an atomic snapshot,
or a GPU reservation. They do not establish the cause of the immediate busy
reading, a firmware sampling interval, physical copy-engine selection, or
absence of unobserved interference.

## Independent History

The [earlier refused attempt](../dev-hip-native-copy-smoke-1890a64e1-2026-09-18/README.md)
remains immutable and refused: an attached PID caused preflight rejection and
no native HIP process launched there. This new attempt used fresh owned paths,
a new CPU-only build and its own fresh strict admission. No old result,
admission, or binary was reused. No previous rejection is rehabilitated.

`PROTOCOL.md` is the unchanged pre-execution review document, so its prospective
status language describes preparation, not the final outcome. `driver-source.sh`
is preserved as an inherited script but was not invoked in this attempt; the
earlier packet contains the separately scoped historical driver observations.

## Inputs And Checks

- Exact committed comparator/header/observer source:
  `1890a64e1911a2a346c5a9e70a8ff5ad45b19231`; three original Git blobs are
  retained with byte lengths, Git blob identities and SHA256 values in
  `source-export.json` and `source-files.sha256`.
- Fresh native binary SHA256:
  `207c65b8c540e28fc0c960bd962e23e6e30802132f4a68be058d22912af076e1`.
  Build uses real HIP/ROCr, explicit `--offload-arch=gfx942`, CPUs 48,49,
  core limit zero, and a 180-second build bound. No mock is linked.
- The real compiler dependency, tool and library manifest covers 350 unique
  files. Source, binary, platform and script hashes pass before and after the
  native process. The original build manifests match the returned copies.
- The independent HIP payload checker is pinned at
  `d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561`.
  Thirteen CPU checker test groups passed before launch. The identity-only
  comparison against the prior reviewed wrappers records only fresh owned
  paths and build identity pins, not protocol changes.
- Exact native command/environment are in `protocol.py` and the actual
  `returned-results/smoke/006-native.json`: TERM timeout 180 seconds, KILL
  after five more, core zero, visibility filters cleared before applying
  `ROCR_VISIBLE_DEVICES=4` and `HSA_XNACK=0`.

The mutable working tree, including later Worker and runtime changes, was not
the comparator build input. This packet does not qualify public generated
execution, R126, A1/A2, HIP/KFD parity, or any proof milestone.

## Cleanup And Audit

Original local staging is
`/home/harsh/.codex-tmp/hip-smoke-1890a64e1-new-20260918.STshT3l4`.
Only the marked remote directory
`/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.EvupxcTh` was removed,
after collection. The cleanup records 339,108 regular bytes removed and all
13 owned command PIDs/process groups absent. A separate stdin invocation
confirmed path absence; its local receipt closed at
2026-09-18 10:24:47.197265328 UTC. Accessible same-user executable/cwd/fd
references were empty. Unreadable unrelated `/proc` entries are listed;
this is not a global reference-absence proof. No other jobs or files were
signaled, reset, or removed. All execution sessions are closed.

`origin-files.sha256` binds every retained original staging artifact byte for
byte. Executables, libraries, the redundant source tar and caches are excluded;
their actual build/linkage/source identities remain in the original receipts.
No original receipt or script is reconstructed or reformatted.

Run the portable read-only audit from this directory:

```sh
python3 -B audit.py
```

It validates exact local and remote command rosters, ordered closure, committed
source blobs, before/after identities, full native payload, both actual timing
windows, the preserved immediate refusal, and exact cleanup. Original record
files are under `raw/` and `returned-results/`; archive-only review commands are
recorded separately under `review/`.

All eight archive-only review receipts, including the separately recorded root
audit, closed with exit zero. A second independent read-only source and scope
review also found no blocker. `seal.py` checks that exact review roster and the
portable audit before hashing every regular file except `SHA256SUMS` itself.
