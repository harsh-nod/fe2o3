# MI300X rocprof wrapper host-wall comparison, 2026-09-03

This record is one bounded observation on host `mi300x`. It compares an exact
caller-declared direct-KFD target process with the same target launched through
the sealed ROCm 7.2.4 rocprofv3 kernel-trace wrapper. The canonical machine
record is
[`mi300x-rocprof-wrapper-host-wall-2026-09-03.json`](mi300x-rocprof-wrapper-host-wall-2026-09-03.json).

## Inputs

- target: release `gfx942-runtime-vecadd-benchmark`, arguments `auto 1 1 1`;
- target dynamic dependencies observed by `ldd`: `libgcc_s.so.1`, `libc.so.6`,
  and the ELF loader; no HIP or HSA runtime library was in the application
  dependency path;
- collector: `/opt/rocm-7.2.4/bin/rocprofv3`, recognized exact
  ROCProfiler SDK 1.1.0 release closure, git revision
  `97f5574fe2fdc7bef44fb01545347912ee9f1779`;
- interpreter: CPython 3.12.3;
- policy: 5 warmup pairs, 30 measured pairs, alternating order, 30-second
  per-process timeout, one-hour harness bound, no outlier removal;
- caller candidate wrapper-path budget: 1,000 basis points (10%).

The plan and a separate plan-derived repeated-target-execution acknowledgement
were supplied back to `cargo fe2o3 profile --collect`. The canonical record
contains content identities rather than executable paths, raw addresses,
handles, tokens, or captured stdout/stderr.

## Result

All 70 raw and wrapped processes exited successfully without stream
truncation. Both warmup and measured wrapped inventories were empty. The exact
summary and durable artifact digest are filled from the final post-change run
below:

```text
raw median: 819,180,977 ns
wrapped median: 1,075,406,076 ns
raw p95: 853,701,111 ns
wrapped p95: 1,120,130,018 ns
median paired wrapper delta: +3,135 basis points (+31.35%)
candidate result: exceeds_candidate_budget
canonical JSON sha256: 1a87c01fd0521a819cd6936df2ffab57090644eac797e5ee2ddfb7f7e159b9cd
manifest sha256: d4ab98529632700351b2f579385103a6ff1b4a52b29c41cec01b35ecf99ffc39
measurement harness sha256: e11f19144e066d8aae066cfa0c8b119ae37858a44af039095480a2a4bc6f238d
target sha256: 3ea65bf68139ebf0d563e5d74f1e9de35bfe2fb93a1a019afa85c935d2cebb8b
```

This is host-observed rocprof wrapper/process overhead for this exact target,
not GPU kernel-trace capture overhead. No collector artifact was admitted, so
kernel-trace capture overhead and loss/completeness are unavailable rather than
zero. Counter, PC-sampling, ATT, and debugger overhead were not measured. The
record grants neither collection authority nor production qualification and
does not prove universal ROCProfiler behavior.
