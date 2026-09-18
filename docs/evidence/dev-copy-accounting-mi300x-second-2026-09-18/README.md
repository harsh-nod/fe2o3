# Rejected First Corrected Copy Accounting Candidate

Portable evidence for one separately authorized corrected-candidate attempt on
2026-09-18. The previous failed packet remains separately preserved.

This candidate was rejected: its single native test aborted before the explicit
copy loop, readback, or shutdown-refund assertions. No successful copy validation,
refunding qualification, performance comparison, formal result, or milestone
acceptance is claimed. No further retry was performed or authorized here.

## Frozen Inputs

- Source base `b87f30d1b87b2dca29e9f03e8b00f99a65b04391`.
- `source-before.log`: 5,543 source paths, SHA256
  `fa3dfa4fa0a5faa2c92c5f84726174f308498e208af6e11b16a6a242cb8ba008`.
- Corrected source witness `copy_accounting.rs.txt`, SHA256
  `bded7480cdf174d91756b5cbf1566aee8acd7da9af122cbc9bbea0ba5925cdf8`.
- Supplied all-features static musl `runtime-test`, SHA256
  `84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312`.
- Unchanged `copy-host-observe.py`, SHA256
  `5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51`.

The primary agent built this binary; this runner performed no local or remote
build. The 57 MiB executable is deliberately omitted from this portable packet.
Its exact SHA256 remains independently witnessed by the local input receipt,
both source-check receipts, remote input receipt and both remote inspection
transcripts. Both local source receipts rehashed the exact 5,543 frozen files,
binary, observer and retained test source. Remote inspect-before/after checked
executable and observer hashes, static ELF, memory/disk capacity and NUMA/CPU identity.
Before running, uploaded runner/inspection hashes matched local originals; the
collected script copies remain under `returned/`.

The corrected fixture distinguishes requested staging capacity 4,194,272 from
page-rounded backing 4,194,304. Its intended final pool-capacity sum is
809,500,640, but the final-pool assertion was not reached in this attempt.

## Actual Attempt

Target: GPU4, UID `0x54f88318ca05093d`, BDF `0000:85:00.0`, NUMA1,
CPU48-95. Fresh complete preflight passed immediately before the native command:
all three direct sysfs VRAM samples were 298,647,552 bytes, utilization counters
were zero, and the complete PID transcript showed no selected-GPU attachments.
These are sequential guarded shared-host observations, not a reservation or
continuous exclusion guarantee.

One exact ignored test was launched with visibility filters unset, core limit
zero, timeout TERM180s/kill-after5s and the recorded CPU/NUMA binding:

`kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing`

Native UTC envelope: `2026-09-18T08:32:56.289389736Z` through
`2026-09-18T08:33:01.840267749Z`; exit 134. First panic:

```text
copy_accounting.rs:96:9
assertion `left == right` failed
  left: (0, 3, 0)
 right: (0, 0, 0)
```

The failure is in the `(reserved_records, retained_records, quarantined_records)`
oracle after allocations and corrected byte-count checks. The raw diagnostic
does not name which loop entry produced the tuple. The explicit H2D/D2H loop,
full readback, and shutdown-refund assertions were not reached. There is no
successful harness footer or accounting-complete marker. The original early
read-only stdout/stderr receipt is retained, and exactly matches the subsequent
collected native stdout followed by stderr.

Immediate complete post-observation exited 1, retaining `sysfs-before-vram`:
567,128,064 bytes initially, then 298,647,552 in the next two samples and SMI.
Busy counters were zero and the complete PID observation showed no GPU4
attachment. After the separately recorded 20-second delay, all three samples
were 298,647,552, no selected-GPU attachment was observed, and that endpoint
exited 0. Neither later samples nor the delayed endpoint rehabilitate the failed
first sample or the native abort. The VRAM transient is not assigned a cause.

Overall campaign status is 1. There was exactly one native invocation in this
distinct attempt.

## Cleanup And Verification

Fresh owned remote directory:
`/tmp/fe2o3-copy-accounting-corrected-20260918.W7d8ptkX`.

After collection, cleanup found no owned executable/cwd references and removed
only the marked, user-owned 57 MiB directory at
`2026-09-18T08:33:59.558765454Z`. A separate absence/reference scan completed
at `2026-09-18T08:34:16.847750600Z`, exit 0. No other jobs or files were changed.

`raw/` contains 12 original closed local receipts, including `early-native`;
only campaign has nonzero status (1). `returned/results/` contains seven closed
remote receipts: native 134, immediate observer 1, all five others 0. Every
receipt retains actual command, UTC timestamps, exit status, stdout and stderr.
The independent source-after check overlaps cleanup, but finishes before the
separate absence scan begins; no broader global serial ordering is claimed. The
supplemental `raw/audit.*` receipt was produced by the preserved original
`audit-staging.py` while the executable was present. It is historical and is not
represented as execution of the portable auditor.

The portable `audit.py` requires the ELF to be absent, verifies exact identity
witnesses and commands, the complete original receipt rosters, failure ordering,
sticky endpoint refusal and owned cleanup. `portable/` records a distinct run of
that auditor plus Ruff lint/format and ShellCheck. These checks do not rerun the
native test, observer, source qualification or cleanup.

Read-only audit from this directory:

```sh
python3 -B audit.py
```

`SHA256SUMS` closes every retained file except the manifest itself. This packet
does not include a source edit, commit, push, successful native attempt or
accepted accounting result.
