# Rejected Guarded Four-Cell Campaign, 9b9265c69

This development attempt is **rejected as a matched campaign**. It is not a
performance comparison, a retry authorization, or milestone acceptance.

The single campaign ran on MI300X GPU 4 from 2026-09-18 09:15:28.014724241 UTC
through 09:17:27.812780076 UTC. It stopped after D/1's immediate endpoint
refused admission. The delayed endpoint's success did not rehabilitate it.

| Cell | Native Exit | Validated Rounds | Pre / Immediate / Delayed |
| --- | ---: | ---: | --- |
| A/1, KFD slice50us | 0 | 13 | 0 / 0 / 0 |
| B/1, KFD profiled sleep1ms | 0 | 13 | 0 / 0 / 0 |
| D/1, HSA fine/mask2/CPU1-nearest | 0 | 13 | 0 / **1** / 0 |

C/1 and all subsequent blocks were not launched. There were three native
processes, 39 validated round trips, and nine complete endpoint observations.
The strict matched summary exits 1 with no stdout and emits **no ratios**.

## Exact Refusal

D/1's output reports 13 fully checked 256 MiB round trips, signal destruction,
three successful allocation frees, and successful HSA shutdown. The process
exited 0 and its owned process group was absent by
`09:17:03.217572657Z`. The output does not individually timestamp those frees.

The first direct BDF sysfs sample began at `09:17:03.251308578Z`, about 33.736 ms
after that command's recorded finish. It reported:

- GPU busy: **46%**; memory busy: 0%.
- VRAM and visible VRAM: **298,696,704 bytes**, below the 536,870,912-byte limit.
- GTT: 25,239,552 bytes.

The only refusal reason is `sysfs-before-busy`, not a VRAM-threshold failure.
Later sysfs samples within that same endpoint and SMI report GPU busy 0%, VRAM
298,647,552 bytes, and GTT 25,231,360 bytes. The complete PID capture spans
`09:17:04.294594393Z` through `09:17:05.330909846Z` and identifies **no GPU 4
attachment**. Its only nonempty attachment is PID 3161403 on GPU 0. The delayed
endpoint also reports no GPU 4 PID, zero busy, and the baseline values.

These observations do not establish the cause of the busy value or continuous
isolation. The sysfs, SMI and PID captures are sequential. No competing process,
residual telemetry mechanism, physical cleanup latency, or accounting fault is
inferred. No guard was relaxed and no native process was retried.

## Immutable Inputs

Source commit: `9b9265c6919cb8dff9506f2c6ffa7b7f2538905f`.
The export contains exactly 5,543 scoped source files and two committed,
historical inner-payload validators. `source-git-audit` and its final repeat
match every exported byte, executable mode and path to that commit's Git blobs.
No uncommitted HIP or Worker changes were inputs.

- Source tar SHA256: `b88884491326ade19a864f5538bf1f806f069eb2951ac134f9f6d094238ca1cd`.
- Source manifest SHA256: `9d15e3613cc69f3a2f1437ec23c48788248b9244e81f2953da6d9db46e86c16d`.
- KFD executable SHA256: `8cc0a27ff79a55a6141f61d9fd7e52476ef39421dbc7d6dbd1b8f8aafc14a182`.
- HSA executable SHA256: `5d7e0578ea8078bf10066bbd7f36fe78608449af6fb6d647f2ec4503e86d9e33`.

The remote CPU-only build used two Cargo jobs, core limit zero, the frozen
lockfile, and nightly-2026-04-03. Build receipts, source checks, ELF/header
inspection, linked-library lists, 25 library/header/tool identities, uploaded
script hashes and downloaded executable hashes are preserved. All four campaign
identity checks pass both before and after execution. These are bounded identity
observations, not continuous checks or a census of every possible dynamic load.
The source tar and executable files are excluded from this portable archive;
their original hashes and build/export witnesses remain. Complete original
staging is retained at
`/home/harsh/.codex-tmp/kfd-matched-9b9265c69-20260918.vIUYFo8l`.

## Protocol And Scope

The reviewed protocol was sixteen serial processes in balanced orders
`ABDC / BCAD / CDBA / DACB`, with depth one, 256 MiB, three warmups and ten
measured samples per process. UID `0x54f88318ca05093d`, BDF `0000:85:00.0`, KFD
node 6, CPU list 48-95, and NUMA node 1 were revalidated. Every executed cell has
a fresh complete preflight, immediate complete postflight, a recorded 20-second
delay, and a complete delayed postflight. Native commands use TERM after 180
seconds, KILL five seconds later, and core limit zero. Any failure is sticky.

B/C would vary only the requested sleep ceiling within the same profiled native
wait route. A also changes instrumentation and re-entry. HSA times host submit,
wait and signal reset; KFD does not include submission release in its interval.
HSA mask 2 in both directions is not authenticated as physically equivalent to
KFD H2D engine 1 and D2H engine 0. The selected HSA pool/engine is not a default
policy or best-baseline claim. **There is no HIP cell**, overlap claim, DMA timing,
parity claim, formal qualification or accepted milestone change.

## Cleanup And Audit

The original cleanup attempt failed before deletion with a permission error
reading `/proc/1444849/fd`; its original script, exit 1 and traceback are retained.
A separately authorized helper verified all **29 recorded owned PIDs and process
groups absent**, scanned accessible same-UID executable/cwd/fd references, and
explicitly reported unreadable unrelated entries. It removed only the exact
marked directory `/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD`, containing
462,964,656 regular-file bytes. An independent check confirmed path absence at
`09:20:50.306344557Z`, again with all recorded owned PIDs/groups absent.
The accessible reference scan found none; this is **not a complete global
reference-absence proof**. No other job or artifact was modified.

Run `python3 -B audit.py` from this directory for the portable audit. It checks
24 closed original command receipts, all 29 ordered remote command records,
exact arguments/environments, raw output hashes, complete endpoints, original
inner-payload validators, source/build identities, sticky refusal, and both
cleanup paths. The three expected exit-1 receipts are the rejected campaign,
initial permission-limited cleanup, and strict matched-summary refusal.

The six prelaunch checker CPU tests use synthetic receipt envelopes around
immutable historical A/B/C/D payloads; they are not new native evidence. They
cover malformed commands/endpoints, full-roster closure, and refusal of ratios
after a failed immediate observation despite a successful delayed observation.
`review/` holds separate portable-audit and lint receipts. Executed scripts and
all raw records retain their original bytes; source.tar, binaries, caches and
unrelated process arguments are not published.
The format check names its scope explicitly: it does not rewrite the executed
fallback cleanup or original source-export helper. Both are included in lint.
