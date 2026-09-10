# Session-Local Native Backing Credits

Local MEM-2A implementation and regression evidence. Baseline:
`d65245cb362a796d737131c6fa63f65a61ec3c2c`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are in
`source-files.sha256`; retained logs have a separate manifest.

## Scope

An optional immutable native-session budget now charges canonical padded N2
backing bytes and allocation records before native entry. Exact private
session/device/VM/allocation identity accompanies the retained charge. Mapping,
initialization, lease drop and uncertain results do not refund it. Complete
backing free, VA release, closing currentness and checked accounting are
required before disposal refunds the charge.

Cross-review found and corrected configured-path panic continuation in native
mapping, CPU initialization/access and paired XGMI operations. The guards
preserve original panic payloads, quarantine affected sessions and perform no
cleanup. Ordinary partial-prefix recovery and both-unconfigured behavior remain
unchanged. This is not a global budget, pool qualification, runtime constructor
forwarding, hardware acceptance or a performance result.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,065 passed; zero failed; five existing ignores; 47 harnesses |
| musl same runtime gate | 2,065 passed; zero failed; five existing ignores; 47 harnesses |
| GNU runtime plus host doctests | 77 passed |
| musl runtime doctests | 66 passed |
| musl default direct-KFD host doctests | Ten passed |
| GNU all-feature host library | 207 passed; four existing ignores |
| Generated macro fixtures | Seven passed |
| Runner/checker Python suites | 134 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed, warnings denied with existing scoped custody policy |
| Formatting and source whitespace | Passed |
| Production musl metadata audit | 43 packages; eight permitted build scripts |
| Workspace dependency policy and tests | 141 members, eight layers, 476 declarations; eight tests passed |
| CI-local test-gate regression and standalone lockfiles | Passed; 32 standalone manifests checked |

Runtime gates cover completion, runtime-model, resource-accounting, KFD and
runtime; lint also covers host and macros. New coverage consists of 20 native
N2 tests (19 fake-backend tests and one public-wiring source check), eight
private adapter tests and three production-model tests. The matrix includes
padding, independent capacity dimensions, stale identity, pre-entry rejection,
malformed native outcomes, disposal failures, slot reuse, panic custody and
paired XGMI prefix recovery. It does not supply live native evidence.

All sixteen final gate commands and statuses are retained in
`raw/r68-final3-source-gate.json`. The harness uses pinned Rust, locked/offline
dependencies, four Cargo jobs, disabled incremental compilation and an unset
stale `XDG_RUNTIME_DIR`. Structured lockfile comparison confirms that all 22
changed lockfiles add only KFD's shared-credit dependency edge; package records,
including all registry identities, are otherwise unchanged.
Raw logs retain their original terminal whitespace and final blank lines;
source whitespace checks exclude these verbatim `.log` artifacts.

Earlier attempts are retained, not promoted. The initial host build hit disk
exhaustion, which also prevented its harness from finishing the JSON record;
the truncated record is preserved. The first Verus run failed its exact
negative-output guard during the same disk-full interval. Targeted Cargo
cleanup removed only this worktree's generated outputs for four crates on GNU
and musl (34.8 GiB reported); no source was removed. A subsequent test compile
found missing Debug bounds on opaque recovery owners, and the next lint run
found two large-error warnings in the recovery test. Assertions were corrected
and the test received the same scoped inline-custody allowance as production.
The complete final local gates then reran successfully.

## Proof And Remaining Gates

The full authenticated Verus run passed: 57 positive sources, 1,334 obligations
and 645 expected-negative rejections, including R68's four obligations and five
named mutations. Pre/post source, inventory, exact transcript and pinned release
closure checks passed (190 files, 129,019,839 bytes). The exact command was:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R68 proves the bounded cost projection and algebraic conservation under R67's
postconditions. Rust/Verus correspondence is reviewed, not machine-refined.
Native extraction/disposal, mutex/arena ownership, parent/global budgets and
whole-executor refinement remain open. Public Linux XGMI wiring is source-tested,
not live-qualified; configuration transfer/loan/retake lacks an integrated
round-trip regression.

A read-only MI300X query exited successfully and showed GPU1 at 100% utilization
and 44% VRAM allocation. No new remote stage, build or qualifier was started;
foreign work was untouched. Signed hardware acceptance, pool-path qualification
and matched HIP/HSA performance remain separate. A1/A2 and runtime-wide parity
are not closed by this record.
