# Issue 271 CPU Reference WIP

This branch is work in progress, not accepted tutorial qualification or a
release pin. The compiler integration and issue 272 semantic-MIR work remain
on their separate WIP branches.

## Implemented

The manifest-selected standalone CPU observer and generic `--batch` mode use
the managed `cargo-fe2o3 test` path. Batch mode builds one private driver and
uses fresh, isolated suite build directories. It charges the complete common
setup cost against every suite's existing timeout. Drift, incomplete evidence,
timeouts and cleanup failure prevent successful batch qualification. All
authority flags remain false.

The executable observer streams artifacts within the managed runner's existing
512 MiB bound. Its digest describes the post-execution on-disk artifact, not a
sealed executed image. No compiler qualification, simulator, formal proof,
hardware, publication or launch authority follows from these observations.

## Verification And Remaining Work

- The final helper and test bodies passed 44 component tests, including 20 new
  batch tests. Cargo, driver and harness execution in these orchestration tests
  is doubled; this is not a real managed batch result.
- The parent CPU WIP passed the CLI unit target: 397 passed and 5 ignored. Its
  verifier integration passed 8 tests with 6 runtime-dependent tests ignored.
- The actual standalone artifact-fix pilot exceeded its unchanged 1,200-second
  deadline before completing tests. It is invalid, not a passing observation.
  Source/helper stability checks passed and private build/driver scratch was
  removed. The earlier pilot completed six tests but its observer rejected the
  artifact; neither pilot is accepted tutorial evidence.
- A real managed batch run, successful standalone observer run, compiler and
  simulator corpus qualification, and applicable hardware checks remain open.
- The explicit bootstrap-host experiment is not included. No profile, job,
  deadline, parser or verification-gate exception was introduced.
- The approved Verus runtime qualification remains deferred, not passed.

All issue 271 milestone acceptance boxes remain open. This branch is shared
for collaboration; it must not be used as the tutorial website's accepted
compiler pin.
