# Failed Native Copy Accounting Attempt

Preserved evidence for one explicitly authorized test on 2026-09-18. No retry,
performance comparison, successful copy validation, refund qualification, or
milestone acceptance is claimed.

## Inputs

- Frozen source base: `b87f30d1b87b2dca29e9f03e8b00f99a65b04391`.
- `source-qualified-before.log`: 5,543 source hashes, SHA256
  `d22891250b363dda63dc154cf5a3e3a66b5bd4cd110cdc7f148c02793cd7834c`.
- Original test source retained as `copy_accounting.rs.txt`, SHA256
  `f1bec38ea62c093c3b00be7511f26576213afc908154e18f18340a8c22baa6c4`.
- All-features static musl test binary (omitted from publication), SHA256
  `8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89`.
- Observer `copy-host-observe.py`, SHA256
  `5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51`.

The binary was supplied by the primary agent's frozen build, not rebuilt by this
runner. Local source-before/source-after receipts rehashed all frozen paths and
the exact binary. Remote inspect-before/after receipts checked uploaded binary
and observer bytes, static ELF, disk/memory and target topology. Uploaded runner
and inspection scripts were also compared by SHA256 before execution; collected
copies are retained under `returned/`.

## Outcome

Target: MI300X GPU4, UID `0x54f88318ca05093d`, BDF `0000:85:00.0`, NUMA1,
CPU48-95. Fresh preflight passed with no selected-GPU process attachments and
three direct sysfs VRAM samples of 298,647,552 bytes, all busy counters zero.
These are sequential shared-host observations, not an exclusive reservation.

Exactly one ignored libtest was invoked:

`kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing`

The native command ran from `2026-09-18T08:24:01.709463850Z` through
`2026-09-18T08:24:07.393068957Z` and exited 134. Its first panic was the assertion
at `copy_accounting.rs:59:5`: observed pool `retained_free_bytes` was 4,194,272;
the fixture expected page-rounded `STAGING_BYTES` of 4,194,304. This occurred
after three allocation calls and before the explicit H2D/D2H copy loop, full
readback, and shutdown-refund assertions. There is no successful libtest summary
or accounting-complete marker. Raw stderr is preserved, including timeout's
abort message. Core size was limited to zero by the recorded command.

Immediate post-observation exited 1: the first direct sysfs VRAM sample was
567,136,256 bytes, exceeding the unchanged exclusive 512 MiB limit. Its next two
samples and SMI reported 298,647,552; utilization remained zero and the complete
PID transcript showed no GPU4 attachments. The failed first sample remains
binding even though later samples passed. No cause is inferred from this
transient or from the sequential PID observation.

After the recorded 20-second delay, the final observer exited 0 with all three
VRAM samples at 298,647,552 and no selected-GPU attachments. This does not
rehabilitate either the native abort or the immediate endpoint refusal. Overall
campaign status is 1; no second native invocation was made.

## Cleanup And Records

Owned remote directory:
`/tmp/fe2o3-copy-accounting-20260918.HyOO64fz`.

Collection completed before cleanup. The cleanup scanned executable/cwd links,
found no owned process references, and removed only that marked, user-owned
57 MiB directory at `2026-09-18T08:25:23.398048341Z`. An independent absence and
executable/cwd scan completed at `2026-09-18T08:25:40.444307656Z`, exit 0.
No other jobs or files were altered. No remote build was performed.

`raw/` holds 11 original local closed receipts; only `campaign.exit` is nonzero
(1). `returned/results/` holds seven original remote closed receipts; only
`native.exit` (134) and `post-immediate.exit` (1) are nonzero. Every receipt has
the actual command, UTC start/end, status, stdout and stderr. Source-after ran
while the separate absence scan was finishing; they are not asserted to be
strictly serial. The local `audit` receipt is supplemental and checks the
already-closed evidence, not a test rerun.

Read-only verification from this directory:

```sh
python3 -B audit.py
```

The 57 MiB ELF is omitted from publication. The portable auditor verifies its
identity across the original local and remote hash receipts, source checks,
and both native inspections; it does not rehash the absent executable. The
original staging auditor is preserved as `audit-staging.py`; `raw/audit.*`
records that original audit while the ELF was available. `review/` records the
separate portable audit and static checks. Other original inputs and receipts
are unchanged. `SHA256SUMS` seals the exact published file roster.

The [initial CPU qualification](../dev-copy-host-observation-2026-09-18/README.md)
binds this rejected candidate's source and binary. Corrected-candidate attempts
are separate packets, not retries included here.
