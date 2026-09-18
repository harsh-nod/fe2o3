# KFD Copy Progress Diagnostic: CPU Qualification

Above `3d2c93a748bfbde7e37093376d121eea67258649`, the directional-window example
adds exact opt-in modes `diagnostic-slice50us` and `diagnostic-window-deadline`.
The [diagnostic contract](../../../benchmarks/runtime_gfx942/KFD-COPY-PROGRESS-DIAGNOSTIC.md)
defines the distinct schema, timer boundaries and native qualification limits.
No production runtime implementation, queue/memory profile or existing proof
changes. Native R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3 remain
the accepted checkpoints; A1/A2, #182 and HIP/HSA parity remain open.

## Results

- GNU and scoped musl example suites each pass **13 tests**, none ignored.
- GNU and scoped musl full runtime library suites each pass **1,057 tests**,
  with **17 ignored**. Exact name/status rosters agree across targets.
- The existing backend continuation/frontier regression also passes alone.
- Scoped example clippy with `-D warnings`, runtime formatting, and the
  legacy-isolation checker pass. The checker reconstructs the exact six inserted
  opt-in lines over the base source and rejects three legacy-body mutations.
- Eleven selected code/dependency-input hashes match before and after final
  qualification. Four executed example/library test binary hashes are retained
  as observations; the binaries themselves are not included in this archive.

The first `gnu-example` record passed twelve tests during development. Review
then strengthened the independent wait-budget oracle, made output fixtures vary
by direction and round, and added the sliced-configuration test. Final acceptance
uses `gnu-example-final` and `musl-example`, not that preliminary pass. The final
source-before record predates every final test/clippy command. No failed test or
workload was omitted.

The initial receipt verifier failed because two serial abort tests print child
harness banners between the parent's name and its successful status. The failed
`verify` and unsuccessful first correction `verify-corrected` receipts are
preserved. The correction initially expected an extra initial newline;
the fixture was fixed to match the actual child output. `verify-final` passes
with a parser that
accepts only the observed banner form, with regression assertions rejecting
missing, failed, unexpected or interleaved parent statuses. Test results and
raw logs were not changed.

The tests preserve original Context ownership/currentness, initial and Pending
flushes, exact submit/progress/total decomposition, deadline exhaustion, native
error propagation, checked counters and output errors. All success output is
buffered behind a completed-run value created only after full-buffer validation
and explicit allocation/stream/Context/native teardown. Two independent read-only
reviews checked implementation and test oracles. They did not run workloads.

## Limits

These are host-only scripted/control/output checks. No native timing, copy
speedup, physical engine placement, memory/cache equivalence or new formal
refinement is established. The 17 ignored runtime tests are not passes. The
scope is the runtime library and this example, not every workspace test or
device kernel. Neither the global Verus campaign nor the global unsafe-source
suite was rerun for this benchmark-only change.

The four-argument comparator source, measured timer/progress bodies and original
output schema are unchanged except for the opt-in module/dispatch outside those
timers. This source-level isolation does not claim identical machine code after
relinking. Compare the two identically instrumented diagnostic policies to each
other. A full-remaining wait is not an exact shared-Instant deadline and can
include adaptive yield/sleep; differences measure combined wait policy and
re-entry effects, not pure facade or device duration.

All Cargo qualification commands used the existing pinned lockfile with
`--frozen`, disabled incremental compilation and zero dev/test debug info. Musl
used `FE2O3_HIP_SYS_DISABLE=1`. Selected source hashes, compiler version reports,
command/exit/time receipts and binary observations do not constitute a complete
toolchain/dependency-loader attestation.

Local disk pressure was handled by removing only this scratch worktree's
completed `fe2o3-host` build outputs with Cargo, approximately 86.6 MiB. No remote
build or GPU workload was created by this CPU qualification.

## Replay

```sh
python3 -I docs/evidence/dev-kfd-copy-progress-cpu-2026-09-18/verify.py
python3 -I docs/evidence/dev-kfd-copy-progress-cpu-2026-09-18/legacy-isolation.py
```

The archive verifier checks final source identity, closed receipt intervals,
before/after boundaries, exact test rosters, the executed-binary path roster and
legacy-isolation observations. It does not execute tests or require the recorded
test binaries to remain on disk. `SHA256SUMS` freezes all archive files after
these checks; raw Cargo output is intentionally preserved byte-for-byte.
