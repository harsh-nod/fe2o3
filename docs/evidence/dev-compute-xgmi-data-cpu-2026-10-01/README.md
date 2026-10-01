# Compute-XGMI DATA and PUBLIC Allocation CPU Checks

## Source and Scope

Source base: `ccf5267eb680963a0e06532d5e9aaa201d8ffd46`.
The packet's `source.patch` records all 31 changed source/guard paths, including
new modules. Its SHA-256 is
`3986b3a203c1041ba103b270c65750f94a8352e2d6cb8ab2c7336b0f7b67b9f9`.
Documentation and evidence are not part of that source patch.

The implementation adds a rooted, synchronous, one-packet transfer of recycled
PUBLIC fixed-dispatch DATA between two existing compute VMs, explicit PUBLIC
SDMA allocation and exact-flag pool checkout, and an opt-in R57 multi-device
qualification constructor. Ordinary private allocation and HostVisible defaults
and the exact launch authorities are unchanged.

## Checks

Strict combined all-feature library/test Clippy and the runtime no-default
library check pass, each with exit 0. The full KFD run is incomplete.
The archived command scripts specify the exact locked/offline commands and
clean environment. The broad
library test run uses test optimization level 1 with debug assertions and
overflow checks explicitly enabled, one build job and one test thread. It is
not an unoptimized-debug acceptance result.

Final-source focused KFD runs all exit 0: `compute_xgmi` selects 19 passing
tests, `model_pair_loan` selects seven, and `public_` selects 53 (258.84s,
within its original 300-second bound). None fail or are ignored. These filtered
results are not a full KFD-suite pass. Every before/after ELF hash matches
`e444496b0c4976d4ec35790a062466e85459d14cb70a112ddc545c5a295ce561`.
The exact commands, outputs, exits and hashes are in `kfd-focused-final/`.

The exact emitted runtime ELF was also run independently, unfiltered and with
one test thread: **1,929 passed, 3 failed, 32 ignored**, exit 101, in 131.23s.
Both new PUBLIC-allocation tests passed. The three failures are the existing
authorized-execution telemetry tests, all at `authorized_execution.rs:1317`:
`InspectSocket(Os { code: 1, kind: PermissionDenied, ... })`. Socket creation
succeeds, but subsequent descriptor admission/inspection is denied in this
environment. That fixture and its inspection code were not changed. No check
was skipped or weakened, and no permission workaround was attempted. This is
not a full runtime-suite pass. `runtime-direct/` retains the exact command,
complete output, exit and before/after executable identities.

All 32 existing source-CI workflow commands pass in `attempt-05-after`, with
7,690 recorded inputs unchanged across the run, no timeouts, and all owned
process groups absent. This workflow ran no build, GPU command or solver.
Exactly 19 hash constants across 12 guard/test scripts changed. AST audits and
hash readback preserve all 75 associated executable proof files, proof contracts
and proof counts. Updating these identity pins does not verify the new code.

Earlier attempts remain separate and unaccepted:

- `attempt-01-before`: input inventory rejected an unrelated historical symlink
  before any workflow command. The corrected recorder inventories tracked
  source/tool/workflow inputs, directly referenced evidence dependencies and
  new source modules, excluding unrelated historical archives and user evidence.
- `attempt-02-before`: all 32 commands completed; 16 passed and 16 rejected
  predecessor source hashes.
- `attempt-03-after`: interrupted with exit 130 for a test-only Clippy correction.
- `attempt-04-after`: 29 passed; three rejected remaining duplicate/helper hashes.
- Initial combined strict Clippy: exit 101 for an indexed test loop; the loop
  now uses `enumerate()`. Production behavior did not change in that correction.
- Initial combined library test command: exit 124 at its 1,800-second bound.
  Compilation completed successfully in 27m 33s, followed by an incomplete KFD
  test run. No test failure was reported before timeout. This is not acceptance;
  the separate final command reruns both complete suites with cached artifacts.
- The separate final combined command also exits 124 at 1,800 seconds. Its
  cached build finishes in 0.99s, but KFD's full fault-injection suite does not
  finish within the bound. No failure is reported before timeout; the runtime
  suite is not reached by this command. The independent runtime result above
  is separate, and neither KFD attempt is a full-suite pass.

Rust formatting and staged whitespace checks pass. The historical attachment
checkpoint's 900-second unoptimized KFD timeout remains incomplete; these
checks do not turn that earlier attempt into a pass.

## Limits

The mapping-driver and model-restoration fault tests are separate; there is
not yet a composed CPU invocation of the complete two-session public adapter.
The runtime PUBLIC factory tests select the private opt-in flag directly;
successful native construction still requires hardware.

Runtime-produced persistent SDMA allocations still need their own peer adapter
and routing through the existing cooperative-copy scheduler. Logical extents,
pooled physical extents, retained compute references and both-child terminal
custody must be preserved. The qualification constructor still uses staged
runtime peer copies. This is not end-to-end native multi-GPU qualification,
formal machine-code refinement, HIP/HSA parity or a performance result.

The latest MI300X attempt failed at SSH hostname resolution before any remote
command. No GPU workload, remote staging or scratch cleanup was performed.

## Raw Packet

`raw.tar.gz` contains the source patch, commands, compiler/OS versions, complete
stdout/stderr and exits, test executable identities, source-CI records and
metadata audits, and the separate runtime and focused KFD checks. Failed and
interrupted attempts are retained. Executable files and the Cargo cache are
not bundled; replay requires the recorded toolchain and locked dependencies.
`SHA256SUMS` identifies the archive. The native admission record is included
separately inside the archive; it records no remote execution.
