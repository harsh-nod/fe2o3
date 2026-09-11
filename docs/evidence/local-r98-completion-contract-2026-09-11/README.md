# R98 Completion Contract: Local Evidence

Source parent: signed R97 `1b53ef417d0f4184e2b4e6024b37271b5f719832`.

This record accepts **CO-1**, the production-used borrowed completion classifier
and its named CPU tests, plus **VER-1A.1 contract/inventory only**. It does not
close A1/A2, issue #182, the Context journal, native generated execution, formal
adapter correspondence, live KFD qualification or HIP/HSA performance parity.

## Source Scope

The private classifier in `async_engine/generated_operation/completion_contract.rs`
borrows the original observation without bounds on the backend error. It copies
only inert failure metadata. Classes are descriptive, not decode, publication,
retry or disposal authority. Ordinary `Operation::advance` consumes the result:
one poll, query only after poll success, retain the exact rejected error and
forward the original finishing observation. Existing reply, control, registry
and Context custody semantics are preserved.

Six tests cover all thirteen prebuilt observation families, backend-code
extremes, non-Clone borrowed errors, zero classifier allocations, sticky existing
reply/credit ownership, repeated rejection with one issue, raw failure versus
quiescent-error reporting, and terminal reply with retained registry/Context
records. Zero allocation applies only to classification over prebuilt inputs,
not polling or GPU progress. The terminal fixture is not native process-lifetime
quarantine qualification.

Seven runtime source files change, including two test-only allocator-counter
visibility changes in Runtime's `kfd_backend.rs` and `kfd_backend/drain_capture.rs`.
The KFD crate, Context module, completion/accounting/model crates and Cargo input
path sets and hashes are unchanged from R97. No second allocator, completion
cell, decoder, budget or native receipt is introduced.

The [completion contract](../../runtime-completion-observation-contract-v1.md)
records remaining proof correspondence. The separately reviewed
[journal contract](../../runtime-context-version-journal-v1.md) defines identity,
capacity, mutation inventory, attempt epochs, content lineage and exact NoEffect
settlement. It implements no journal or model. Ordered writers and recovery
remain required before whole-surface reuse.

## Gates

Authoritative records are `raw/r98-final-source-gate.json`,
`raw/r98-final-complete.json`, `raw/r98-auxiliary-results.json`,
`raw/r98-auxiliary-complete.json`, frozen/restored records and four `r98-mut-*`
records. `test-summary.json` contains parsed totals.

| Check | Result |
| --- | --- |
| GNU / musl all-target suites | Each 2,537 passed, 5 ignored, 48 harnesses |
| GNU runtime/host documentation | 108 passed |
| musl runtime / host documentation | 91 / 16 passed |
| GNU host / musl host | 258 passed, 4 ignored / 141 passed |
| Generated macro fixtures | 7 passed |
| Frozen / restored CO-1 tests | 6 / 6 passed |
| Auxiliary completion / controls / owned lifecycle | 6 / 23 / 178 passed |
| Drain rejection / exact terminal retirement regression | 3 / 1 passed |
| Final source gates / auxiliary checks | 17 / 8 passed |
| Compiled behavioral mutations | All 4 fail their intended dynamic assertion |
| Non-documentation source identities | 5,653 unchanged and exactly restored |

Filtered tests overlap the full suites and are not additional unique tests.
Gates include both Clippy profiles with warnings denied, Python tests,
format/whitespace, dependency policy/tests, CI test-gate and standalone lockfiles.
Auxiliary audits include the expected-negative proof inventory, production
metadata and pure-Rust dependency closure. An inventory audit is not a solver run.

`raw/r98-environment.json` records local WSL2 and pinned nightly-2026-04-03,
including cargo/rustc binary hashes. Runs use locked/offline dependencies, four
jobs, disabled incremental compilation and no `XDG_RUNTIME_DIR`. Test wall times
are campaign metadata, not runtime or GPU performance measurements.

## Mutations And Earlier Attempts

| Mutation | Behavioral failure |
| --- | --- |
| `rejected-class` | Rejected observation promoted to a finishing class violates continued observation. |
| `pending-class` | Pending promoted to a finishing class violates continued observation after two rejections. |
| `error-precedence` | Querying after poll error overwrites the expected original BackendQuiescent reply. |
| `terminal-class` | Terminal classified Pending fails immediate delivery to the existing raw reply. |

Each mutation changes one source file, compiles, runs its exact named test and
exits 101 with zero passed/one failed. These are behavioral failures, not
source-text guards. The retainer reconstructs tested bytes from the manifest,
checks the sole changed source identity, exact command and log markers, and
requires restoration before positive final gates.

The first `r98-co1-first` run passed six tests while an earlier formatting
process was finishing; `source_unchanged` is false. It is retained as preliminary,
not accepted evidence. Formatting ended before the frozen run. Frozen tests,
Clippy, all four mutants, restored tests and final campaigns use recorded stable
source identities.

The terminal mutation's initial reverse-patch pattern matched two occurrences.
The generator rejected it before editing. Adding the adjacent terminal arm
made forward/reverse matching unique without changing the tested mutant bytes.
`raw/r98-restoration-note.json` preserves that correction. It is not a failed
test run; all 5,653 source identities are exactly restored.

The collector authenticates exact gate names/commands against R97's committed
records, auxiliary commands and totals, complete before/after path sets and
hashes, explicit completion records, all six test names, four mutation identities
and required preliminary files. Three read-only workers reviewed source,
evidence boundaries and the remaining assignments. Shared-source builds and
mutations were serialized after the preliminary formatting overlap.

## Next Swarm Wave

The [current board](../../runtime-a1-a2-swarm-current.md) assigns Native .5B-2
local platform composition, Admission CO-2A Context identity and Resources
VER-1A.2 model/proofs. CO-2B and the preissue CO-3 contract/oracle can be reviewed
independently. Native identity checks join actual DATA-ADOPT/ISSUE records and
close at CO-4. Primary owns shared integration, proof pins, builds, hardware and
signed pushes; queued assignments are not unattended background jobs.

No SSH sessions, MI300X processes or remote staging were created by this campaign;
no shared-machine cleanup was required. No new solver, live KFD or performance
result is claimed.
