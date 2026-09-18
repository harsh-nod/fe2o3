# R126 Native Campaign: Historical Rejection

This packet retains one native campaign against signed source
`8b0021ba740c337b2641ecd26ed6c9375613ed32` and musl runtime harness SHA-256
`7e75d4a11011cf63705020d3b814aa25a610fe56371c1b8c5d3b5671357376f8`.
The campaign was **rejected**, not qualified. No retry is included.

| Case | Historical result |
| --- | --- |
| Ordinary typed-dispatch shutdown | Passed: complete vecadd readbacks, host accounting refund, matched primary queue/dispatch profiler history and successful backend destruction. |
| Injected-error fixture | Failed with status 101 **before injection** at `primary_envelope.rs:165`: its early `supports_retained_primary_release_v1()` assertion was false. The parent reported its isolated child failure. No successful envelope marker was emitted. |
| Injected-panic fixture | Not run: the campaign stopped after the error case failed. |

These are the exact three test names in `raw/native/collected/PLAN.md` and the
frozen protocol. A later fixture correction, rebuilt binary or CPU campaign is
outside this packet. Neither native error-envelope nor panic-envelope behavior
is qualified here. A pre-injection fixture assertion is not an injected error
result or a real KFD ioctl failure. No HIP/HSA comparison, performance ratio,
full runtime parity or kernel/driver theorem is claimed.

## Admission And Closure

The shared host was `mi300x`, GPU 4, UID `0x54f88318ca05093d`, BDF
`0000:85:00.0`, CPU set 48-95 and NUMA node 1. No exclusive reservation or
all-user isolation is claimed. Each executed test had a fresh strict admission:
three raw sysfs snapshots, complete SMI status/PID captures, busy zero, VRAM
below 512 MiB and no observed selected-GPU PID attachment. The observer started
within one second after the test parent's reap and owned process-group absence
(T0), then exactly once in the fixed T0+[20,21] second window. Both strict
post-observations passed for both executed cases; they do not repair the failed
test.

| Case | Immediate observer start | Delayed observer start |
| --- | --- | --- |
| Positive | T0+0.032366431 s | T0+20.031529134 s |
| Error | T0+0.032346231 s | T0+20.030919822 s |

Each test used timeout 180 seconds, TERM/KILL grace 5 seconds, core size zero,
16 MiB file-size limit and the pinned NUMA placement. The outer campaign used
timeout 1200 seconds with 15-second KILL grace. Complete parent/child stdout,
stderr, command, environment, exit, timing and process-group receipts remain.

All 55 remote files were collected and hashed before exact-owned removal of
`/tmp/fe2o3-r126-primary-8b0021ba-20260918.z3L69nX6`. Cleanup and a separate
absence command exited zero. All 11 recorded native/observer/outer PID/groups
were absent; no accessible same-UID exe/cwd/fd/maps references remained.
Unreadable same-UID `/proc` entries are retained explicitly. Other-user and
inaccessible references were not proven absent. The original failed controller
exit remains 1. The portable audit derives this rejection independently of the
controller's summary flags.

## Earlier Prelaunch Refusal

The separately retained `.oVqzCv3c` attempt never admitted a GPU or launched a
native test. After successful upload, approval refused the remote directory's
0755 mode. SCP had propagated the local bundle's directory mode over the
root-created 0700 directory. Separate authorized inventory/collection retained
all 22 files and established no approval or launch artifacts. The exact marked
directory was then removed and a separate absence check passed, with an empty
native PID roster. Only local bundle directory metadata changed to 0700; all
20 payload file hashes were rechecked unchanged before the fresh campaign.

The original and fresh controllers differ only in their exact `OWNED` path.
Earlier pre-execution cleanup/absence argument-wiring corrections are described
in `control/original/CONTROL.md`; only the corrected controller was executed.
An earlier root SSH quoting error happened before Python/mkdir; its raw receipt
was not retained, so this is narrative history only. The prelaunch inventory
script was subsequently formatted; `prelaunch_inventory.invoked.py` restores its
exact original bytes and matches the recorded stdin hash. The original cleanup
stdin wire, full transcripts and mode receipt are retained unchanged.

## Portable Audit

During unsealed review:

```sh
python3 -B verify.py --allow-unsealed
python3 -B test_verify.py -v
```

After sealing, `python3 -B verify.py` also requires exact manifest byte/closure
integrity. The checker never executes the runtime ELF, SSH, Cargo, observer
commands or GPU operations. It imports only the frozen protocol's pure parser
and validates raw endpoint records, actual timestamps, native transcripts,
profiler output, exact inventories, receipt hashes and cleanup PID rosters.
An audit pass means this **historical rejected outcome is faithfully retained**,
not that the native campaign passed.

Final local audit receipts are in `audit/final`: portable audit, 13 archive
calibration tests, four frozen protocol tests, four frozen controller wiring
tests, format/lint checks and a separate rehash of the retained historical ELF
all exited zero with their recorded local process groups absent. Initial audit
receipts and exact earlier checker bytes remain under `audit`; independent
review prompted the final binary-map bridge and command/timing-bracket checks.
These are CPU-only archive checks, not additional native attempts.

`retention.json` records original local paths and hashes. The 58 MB-class ELF is
not committed; its identity is retained in the original full payload/inventory
receipts. Identical prelaunch payload files are deduplicated against the fresh
collection; both distinct owner markers remain. Portable inspection cannot
rehash an omitted ELF or independently reconstruct a build. It validates the
historical CPU source/binary binding receipt, pinned CPU archive manifest and
5,553-entry source map; six relevant source snapshots are retained. Signature
verification output and the exact export recipe are included, without keys or
large tool binaries.

Optional `--source-root PATH` rehashes all 5,553 files against the **historical
8b0021ba cohort**. Optional `--binary PATH` rehashes a supplied **historical ELF**.
Both are separate from portable historical integrity. A current worktree or
new binary containing the later fixture correction is expected to differ;
do not use this packet to certify it. Without those options the output reports
current source/binary revalidation as absent/false.
