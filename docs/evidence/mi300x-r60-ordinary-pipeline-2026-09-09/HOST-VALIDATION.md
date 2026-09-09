# R60 Host and Model Validation

This record separates affected runtime gates from broader repository gates and
abstract formal models. Hardware evidence is in [README.md](README.md).

## Affected Runtime Gates

Final runtime source: `ec5864b94518dab504d7d5a6bb6706acd6b0be25`.

| Gate | Result | Retained log |
| --- | --- | --- |
| KFD/runtime, all features and targets | 1,128 passed, 0 failed, 3 ignored; 37 targets | [tests](raw/host/fe2o3-r60-control-final-runtime-limited-tests.log) |
| KFD/runtime all-feature doctests | 34 passed, 0 failed, 0 ignored | [doctests](raw/host/fe2o3-r60-control-final-runtime-doctests.log) |
| KFD/runtime all-feature/all-target Clippy, no dependencies, warnings denied | Passed | [Clippy](raw/host/fe2o3-r60-control-retention-clippy.log) |
| R60 Python checker/runner tests | 28 passed | [benchmark tests](raw/host/fe2o3-r60-final-benchmark-tests.log) |
| Runtime formatting and whitespace checks | Passed | `cargo fmt -p fe2o3-runtime -- --check`; `git diff --check` |

The final full Rust test invocation was:

```sh
CARGO_INCREMENTAL=0 RUST_TEST_THREADS=2 \
  cargo test -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets
```

An earlier unrestricted-parallelism final run hit scheduler-tolerance assertions
in `delayed_worker_response_cannot_extend_wait_deadline` and
`v4_flush_deadline_failure_reaps_and_seals_the_worker` during concurrent local
compilation. Its [failed log](raw/host/fe2o3-r60-control-final-runtime-tests.log)
is retained. The full suite was rerun with two test threads; no assertion,
deadline, tolerance, source or feature was weakened. This establishes the
limited-parallelism pass, not immunity to host scheduling load.

New tests cover HostVisible/full-write/global-quiescence admission, mixed and
empty cache rejection, native-digest overwrite decisions, exclusive retained
control ownership, production reconciliation wiring, productive/stalled wait
backoff, and preservation of the original deadline. Policy/source-wiring tests
are not native lifecycle fault injection. The four complete guarded hardware
sets separately cover successful execution and cleanup.

## Abstract Models

The retained [Verus log](raw/host/fe2o3-r60-formal-combined-reauthed.log) passes
49 positive model files with 1,260 verification obligations and 584 expected
negative diagnostics, using the checked pinned tool release closure. The
runtime-model files are unchanged by the later runtime cache/wait edits. The
independent R60 pipeline abstraction contributes 46 obligations and 26 expected
negative cases; its executable model has 20 tests.

These are proofs of the stated abstract models, not a production Rust-to-Verus
or machine-code refinement bridge. They do not formally establish these cache
optimizations, actual device ordering, or broad HIP/HSA parity. Existing
`NotEstablished` refinement boundaries remain unchanged.

## Canonical Attempt

The canonical `scripts/ci-local.sh test` attempt was pinned to the clean signed
ancestor `df17e048792f02bafb09643c603ed1c042df650f`, not the final descendant.
It used two build jobs, incremental compilation disabled, a short private
TMPDIR, and a 7,100-second step timeout. To control storage use, outer dev/test
debug symbols were disabled; the bootstrap retained its own debug settings.

Before stopping, it passed 7,765 top-level tests, failed one, and ignored
82. The pre-CPU stages passed 995 tests; the CPU matrix passed 6,770 before
the failure. The failure was
`authenticated_verus_execution_v2::fixture_has_no_checkout_path_or_path_bearing_debug_sections`:
the fixture test expects a GDB marker in an assertion-enabled executable, but
the debug-disabled build suppressed that marker. See the
[transcript](raw/host/canonical-df17/canonical-transcript.log) and
[CPU log](raw/host/canonical-df17/canonical/cpu-tests.log).

The entire failing verifier target was rebuilt with
`CARGO_PROFILE_DEV_DEBUG=1` and `CARGO_PROFILE_TEST_DEBUG=1`, with assertions
unchanged: **8 passed, 0 failed, 6 ignored**. The
[corrected target log](raw/host/canonical-df17/verifier-limited-debug.log)
is retained. An intermediate attempt with only TEST_DEBUG unset was stopped
during compilation and ran no assertions; its log is also retained. No
compiler/verifier source was changed to satisfy the fixture.

An older attempt at `748be096` had stopped during CPU compilation with ENOSPC.
Only task-owned generated build caches were removed before the df17 retry;
the retry compiled the CPU matrix successfully. Neither unsuccessful attempt
is a canonical pass.

Canonical coverage remains **incomplete**: the remaining verifier/virtual-runtime
and `reserved-fe2o3-symbols` test suffix, 63-package doctests, post-CPU
wrapper/revalidation/PLIRON steps, 19 codegen
steps and 14 auxiliary steps were not completed. A dependency-graph-checked
suffix selection was prepared but not executed. The earlier prefix and the
corrected fixture target must not be combined into a claim that all gates ran.
The affected descendant gates above do not replace these missing repository
checks. All task-owned test processes stopped and the private TMPDIR was
removed; logs remain available for a subsequent full qualification.
