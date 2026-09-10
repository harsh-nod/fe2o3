# Owned Host Drain Capture

Local DRN-1A implementation and regression evidence. Baseline:
`0d5a2a931dbe94c9432691bd1643c2803482bde3`, branch
`codex/r65-runtime-drain-versions`. Changed source identities are in
`source-files.sha256`; retained logs have a separate manifest.

## Scope

DRN-1A integrates a sealed logical HostVisible source, optional per-engine
capture-byte admission, private conclusive-drain authority and currentness-checked
native coherent read-into. The complete owned result keeps its byte charge after
reply extraction and owner shutdown until its known storage disposal. Native
backing is resolved after the accepted prefix drains, allowing completed D2H
to materialize backing without relying on a stale host shadow.

Cross-review corrected an already-poisoned native queue being reported as an
ordinary rejection; it now produces terminal uncertainty before copying. Fixed
errors, rejected registration recovery, partial-result disposal and native panic
containment are included. The [contract](../../runtime-host-drain-capture-v1.md)
describes the public surface and narrower allocation/proof boundaries. This is
not whole-executor refinement, aggregate memory-budget closure, hardware
qualification or a performance result.

## Final Local Gates

| Gate | Result |
| --- | --- |
| GNU runtime all-feature/all-target tests, five crates | 2,106 passed; zero failed; five existing ignores; 47 harnesses |
| musl same runtime gate | 2,106 passed; zero failed; five existing ignores; 47 harnesses |
| GNU runtime plus host doctests | 81 passed |
| musl runtime doctests | 70 passed |
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
runtime; lint also covers host and macros. The 41 new tests comprise six range
model, twelve owned-storage, twelve native/helper, eight async integration, two
Context registration and one configuration test. Four new compile-fail doctests
cover sealed source/request construction and charged-result ownership.

The matrix covers exact token coordinates, late backing, stale/released/foreign
sources, default unsupported SPI, admission races, result retention, abandoned
observers, pre-entry rejection, terminal partial writes, panic and exhausted
drain. Allocation-counting coverage uses production adapter helpers with injected
readers, not live Linux syscalls or the complete owner loop. Ordinary allocator
failure recovery, kernel allocations and panic machinery are not proved bounded.

All sixteen final gate commands and statuses are retained in
`raw/r69-final-source-gate.json`. The harness uses pinned Rust, locked/offline
dependencies, four Cargo jobs, disabled incremental compilation and an unset
stale `XDG_RUNTIME_DIR`. No Cargo manifest or lockfile changed in this packet.
Raw logs retain their original terminal whitespace and final blank lines;
source whitespace checks exclude these verbatim `.log` artifacts.

Earlier attempts are retained, not promoted. The initial filtered capture run
passed eighteen helper/storage tests but did not select the eight new async
integration tests. The first full GNU gate exposed a missing HostVisible
capability in its existing test backend; the fixture was corrected without
changing production capabilities, and all eight `drn1a` tests plus both full
suites then passed. The initial Verus attempt found ambiguous cast syntax in
the new positive source; parentheses and its authenticated source pin were
corrected without changing the theorem. An initial metadata audit omitted its
required root argument; the corrected audit passed. These failed or narrower
attempts are not substitutes for the final gates.

## Proof And Remaining Gates

The full authenticated Verus run passed: 58 positive sources, 1,338 obligations
and 650 distinct expected-negative rejections. Pre/post source, inventory, exact
transcript and pinned release-closure checks passed (190 files, 129,019,839
bytes). The process completed with exit status zero. The exact command was:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus sh crates/fe2o3-runtime-model/verus/verify-verus.sh
```

R69 adds four range-acceptance/exclusion obligations and five named mutations.
Its production predicate and corresponding Verus source have reviewed, not
mechanically linked, correspondence. Native currentness, witness issuance,
result ownership, mutex/arena ownership and the whole executor remain separate
refinement boundaries. Existing credit proofs do not prove the native adapter
or aggregate byte-budget closure.

No SSH query, remote stage, GPU build or hardware workload was started for this
packet. Shared-machine resources and foreign work were untouched. DRN-2 signed
outstanding-work capture, broader DRN-3B composition, generated production
authority, native accounting closure and matched HIP/HSA performance remain
open. A1/A2 and runtime-wide parity are not closed by this record.
