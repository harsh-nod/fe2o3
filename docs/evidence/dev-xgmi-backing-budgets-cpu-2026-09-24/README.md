# Native XGMI Backing Budgets: CPU Qualification

This packet qualifies the CPU behavior of the
[endpoint-budget integration](../../runtime-xgmi-backing-budgets-v1.md).
It does not qualify native execution, physical memory bounds, formal adapter
refinement or HIP/HSA performance. The new native witness remains a separate
required campaign; the historical directed diamond cannot qualify this change.

## Accepted Campaign

`cpu3` is the complete nine-command campaign from `runner-full.py`. GNU and
musl each execute the full, unfiltered library rosters with all features and
four test-harness threads. Both packages retain the same test profile:
optimization level 1, debug assertions enabled, debug information disabled,
incremental compilation disabled and two build jobs. The pinned toolchain is
`nightly-2026-04-03`.

| Gate | Result |
| --- | --- |
| GNU KFD | 1,552 passed; no ignores or filters; 1,633.36 seconds of test execution |
| GNU runtime | 1,365 passed; twenty hardware-only ignores; no filters |
| musl KFD | 1,552 passed; no ignores or filters; 2,251.82 seconds of test execution |
| musl runtime | 1,365 passed; twenty hardware-only ignores; no filters |
| Doctests | 27 KFD plus 46 runtime passed (the runtime prints 4 and 42 separately) |
| Formatting | Both packages passed |
| Clippy | Both packages, all features and all targets, `-D warnings`, passed |
| Source and tools | All 3,910 selected source identities unchanged; before/after Rust and Cargo bytes identical |

Each command has its original argv, status, timestamps, process-group cleanup
receipt and byte-preserved stdout/stderr. `inputs-before.json` and
`inputs-after.json` bind the exact selected source and runner. The accepted
runner SHA256 is
`80afa8fcd95dc5adbcf201397fb64cf98c8d3c73e705e07d37eecb4cf848b62c`.
The source selector includes tracked and untracked Rust, Cargo, JSON, Python
and fixture inputs under the named roots; documentation is excluded. This is
host test evidence, not a hermetic toolchain or a production execution receipt.

The sixteen new regressions exercise the original lower allocation/accounting
driver with scripted native leaves and the production-shared runtime helpers:
padding, byte/record pressure, endpoint isolation, exact pre-effect rejection,
release/retry, mapping charges, failed disposal, native/currentness errors,
malformed outputs, panic custody, constructor order and fail-stop behavior,
queue-completion capacity, and source wiring. The source-wiring guard is not
native constructor execution. Queue pressure remains terminal because earlier
ring/control work may already have occurred.

`focused2` separately passes 64 KFD and 182 runtime XGMI-selected tests, including
all sixteen new tests. It supplements, rather than replaces, the full campaign.

## Retained Failed History

- `focused1` failed one test: its fake failed-VA-release selector used
  `release_va` instead of `release_va_reservation`. The label was corrected and
  three further regressions were added before the complete campaigns. Its
  original 62-pass/one-failure transcript and source brackets remain failed.
- `cpu1` used the original default-concurrency `runner-default.py`. GNU hit its
  1,200-second whole-command bound, including the cold build. No full suite
  completed; later stages were not reached.
- `cpu2` used `runner.py` with four test threads. GNU again hit 1,200 seconds.
  Its partial KFD log contains 573 successful rows, no final suite result, and
  none of the new backing tests. Runtime and subsequent stages were not reached.

Both timed-out process groups were reaped. Neither partial attempt is reused
as a passing full-suite result. `cpu3` was declared prospectively with only the
two GNU/musl command bounds changed from 1,200 to 7,200 seconds; source, flags,
assertions, four-thread concurrency and all other bounds stayed unchanged.
Historical complete four-thread KFD suites already took
[1,836.39 seconds GNU and 2,978.47 seconds musl](../dev-r126-sdma-synchronous-2026-09-16/README.md).
Those used a different optimization profile and are not a performance baseline.
Their durations establish why the earlier whole-command bound was insufficient.

No SSH, GPU workload, hardware fault injection or benchmark was run for this
packet. The task-owned build cache is removed after qualification; raw receipts
and the separate uncompiled native-witness draft remain in the private task
directory. Native R125, Admission R118B, Resources R116/V3, A1/A2 and #182
acceptance remain unchanged.
