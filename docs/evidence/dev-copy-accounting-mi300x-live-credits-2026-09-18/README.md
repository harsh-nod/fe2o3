# Native Copy And Live-Credit Accounting

One separately authorized native test passed on 2026-09-18, using the final
live-credit fixture. It completed a 256 MiB H2D/D2H roundtrip, validated the full
download buffer, recycled the original allocation owners, and observed exact
host/device backing refunds through real retained primary shutdown.

This is a development result for that one guarded copy-only workflow. It is not
a benchmark, protected/generated execution qualification, full native-suite
result, formal proof, or accepted milestone. The earlier
[capacity-oracle failure](../dev-copy-accounting-mi300x-first-2026-09-18/README.md)
and [live-credit-oracle failure](../dev-copy-accounting-mi300x-second-2026-09-18/README.md)
remain separately preserved and are not reclassified as successful runs.

## Frozen Inputs

- Source base: `b87f30d1b87b2dca29e9f03e8b00f99a65b04391`.
- `source-before.log`: 5,543 source paths, SHA256
  `1a704996dc08cca9900e3034e3bed15b11ee9f5a7a6e0b049704ca8f4e0a79e9`.
- Original final fixture `copy_accounting.rs.txt`, SHA256
  `b42daaf3293afc6da164d3c0c6c5a354b12d33fb795e4317789a7938ac79a4b9`.
- Supplied all-features static musl executable, SHA256
  `f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe`.
- Unchanged observer `copy-host-observe.py`, SHA256
  `5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51`.

The primary agent built the executable; this runner performed no build. Its
57 MiB ELF is omitted from this portable packet but retained in original local
staging `/home/harsh/.codex-tmp/kfd-accounting-live-credits-20260918.oZTE2ouG`.
The exact binary SHA is witnessed by local input hashes, both source-check
receipts, remote input hashes and both remote inspections. Local source checks
rehash every frozen source path, the observer, fixture witness and executable.
Remote inspections additionally check static ELF, resource availability and
CPU/NUMA identity. Collected runner/inspection bytes match their local originals.
The separate CPU qualification/build receipt is
[here](../dev-copy-accounting-live-credits-2026-09-18/README.md).

## Native Result

Exactly one test was invoked on MI300X GPU4, UID `0x54f88318ca05093d`,
BDF `0000:85:00.0`, NUMA1, CPU48-95:

`kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing`

The actual command unsets visibility filters, supplies explicit test/device
opt-ins, limits core size to zero, and applies TERM180s/kill-after5s timeout.
Native UTC envelope: `2026-09-18T08:40:54.737843638Z` through
`2026-09-18T08:41:18.499503020Z`. Exit 0; exact harness result: one passed, zero
failed/ignored/measured, 1,118 filtered out. The reported 23.74s is libtest
duration, not copy performance or a comparison against HSA/HIP.

The whole preserved native output reports:

| Observation | Backing Or Pool Bytes | Records Or Buffers |
|---|---:|---:|
| After allocations: free staging capacity | 4,194,272 | 1 free, 3 checked out |
| After logical releases: retained pool capacity | 809,500,640 | 4 free, 0 checked out |
| After pool trim, before primary release: host backing | 532,480 | 3 live retained credits |
| Completed primary release: host backing | 0 | 0 |
| Before/completed primary release: device backing | 0 | 0 |

The fixture's passed source assertions also require host backing after
allocation to equal original baseline plus two 256 MiB owners and one 4 MiB
padded staging allocation, with exactly three additional host records; device
backing is exactly 256 MiB and one record. Healthy live backing uses Retained
credits, with reserved/quarantined counts zero. Pool capacity uses the original
requested staging size, not page padding. Logical release retains cached owners
and their original credits; shutdown performs the real pool trim before primary
release. The fixture validates zero completed usage, healthy accounts and an
inert repeated shutdown. Full readback covers all 268,435,456 bytes.

## Complete Endpoints

Fresh one-sample preflight completed immediately before launch. Immediate and
20-second-delayed post-observers both completed. Every endpoint exited 0 and
retained complete raw SMI status and PID transcripts plus three BDF-bound sysfs
samples. All nine VRAM samples and each SMI observation were 298,647,552 bytes;
busy counters were zero and no GPU4 process attachment was observed.

These are sequential guarded shared-host endpoints, not a GPU reservation or
proof of continuous isolation. The external observations neither define the
resource-accounting contract nor establish a cause for previous VRAM transients.
The test itself explicitly does not assert an external VRAM baseline. No
performance result, comparative ratio, or attribution to other jobs is claimed.

## Cleanup And Archive

Owned remote directory:
`/tmp/fe2o3-copy-accounting-live-credits-20260918.UYReiiVM`.

After collection, cleanup verified the owner marker/user and found no owned
executable/cwd references. Only that 57 MiB directory was removed, at
`2026-09-18T08:42:18.319055603Z`. A separate absence/reference scan completed
at `2026-09-18T08:42:39.930989850Z`, exit 0. No other users' jobs or files were
altered; no further native invocation was made.

All eleven original local receipts and seven remote receipts closed with exit
0. Commands, nanosecond UTC timestamps, stdout and stderr remain byte-preserved
under `raw/` and `returned/`. Source-after overlaps collection, but both finished
before cleanup. Original local staging, including its executable, remains
preserved. No staging auditor was run or reconstructed.

`portable/` contains separate recorded Ruff lint, Ruff format, ShellCheck and
portable-audit checks. The auditor requires the ELF to be absent, validates exact
unique identity witnesses, complete command/receipt rosters, the entire native
libtest and accounting output, complete endpoints, and owned cleanup. It does
not rerun a test, observer, source build, or remote action.

```sh
python3 -B audit.py
```

All four portable checks passed. `seal.sh` closes every retained file except the
manifest itself in `SHA256SUMS`. Original command trailing spaces and raw output
bytes are intentionally preserved, not normalized. The 122 original nonbinary
staging files were independently compared byte-for-byte before sealing.
