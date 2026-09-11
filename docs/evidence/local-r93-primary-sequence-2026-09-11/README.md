# Local R93 Primary Sequence Evidence

Implementation baseline: signed R92
`777cbefae2721bb2edd60187a666e8d84dc80c91`, on both topic remotes.
This increment extends the
[primary-construction contract](../../runtime-primary-queue-construction-custody-v1.md)
with one shared production sequence and initial NATIVE-2A.3 integration.
The containing signed commit records source and evidence together; topic-branch
publication is not a main merge.

## Status

All seventeen final frozen-source gates and eleven auxiliary checks pass with
5,639 unchanged source identities. The final KFD suite contains
916 tests, including five new CPU functions covering 121 same-session cases.
All three compiled sequence mutations reject. Full NATIVE-2A.3 is not closed.

| Accepted check | Result |
| --- | --- |
| Runtime all-feature/all-target tests, GNU and musl separately | 2,496 passed, 5 ignored, 48 harnesses per target. |
| GNU runtime/host doctests | 108 passed. |
| Musl runtime doctests and separate host doctests | 91 and 16 passed. |
| Host library, GNU / musl | 258 passed with 4 ignored / 141 passed. |
| Typed macro fixtures / benchmark checker tests | 7 / 151 passed. |
| All-feature/all-target and production Clippy | Both pass with warnings denied. |
| Formatting, whitespace, dependency policy/tests and local CI test gate | Pass. |
| Standalone lockfiles | 32 checked. |
| Focused primary / ordinary / lower construction / Linux helpers | 13 / 1 / 6 / 20 passed. |
| Focused initialization / transitions / preparation / bind | 4 / 20 / 23 / 4 passed. |
| Production dependency audit | Pass: 43 packages, 8 allowed build scripts. |
| Unchanged proof-negative inventory | 686 files checked; no solver run. |

Focused checks are subsets of the full suites, not additional unique coverage.

## Implementation And Scope

Private `PrimaryMemoryV1` and `PrimaryEnvironmentV1` interfaces forward primitive
operations. Ordering, token handoffs, resource admission, CREATE, assembly and
final settlement remain one production-used algorithm. The Linux specialization
preserves the existing native calls and error classifications. Its actual
`QueueResourceAuthorityV1` and foundation authentication are unchanged. The
completed bundle stays rooted through both engine currentness checks and checked
gate completion; conversion to the unchanged public session then performs only
field moves and inert defaults. No public backend-selection API is added.

The integrated fixture uses the original R89 preparation memory session and
configured Host/Device accounts through the R88 allocator/transitions, R91 lower
handoffs, actual queue resources and existing queue engine. It does not stitch
together replacement sessions or reconstruct usable authority from observations.

| New matrix | Cases and assertions |
| --- | --- |
| Success | 6: three ring backings crossed with owned/external runtime; exact original foundation authentication, queue outputs and final sequence. |
| Borrowed boundary failure | 72: 36 allocation/initialization/platform/preflight/foundation/doorbell/gate boundaries crossed with error and panic. |
| Actual fake-native failure | 22: 11 ALLOC/seal/map boundaries crossed with error and panic in the existing native backend, not a consuming wrapper that loses the input. |
| CREATE uncertainty | 5: genuine failed-no-effect, unknown/known-ID indeterminate, successful return with mutated immutable input and native panic; retained actual authority and published shadows. |
| Currentness | 16: eight fixed constructor occurrences crossed with error and panic; CREATE opening cannot publish, pre-doorbell failure has no doorbell, post-doorbell failure retains it. |

Each case checks original root/session identity, exact GTT token partition across
all prefixes, dispatch, engine, completion and R88 terminal custody, plus the exact
Device leases/facts. `InSession` markers must match terminal tokens and are not
counted as additional owners. Original native data records/bytes and N2 usage are
unchanged. N1 usage equals the actually charged records plus any charged pending
ordinary Host allocation; USERPTR control is not an N1 debit. Pending allocation
is separate from the installed-record/token partition. No native unmap/free/VA
release or platform owner drop occurs before assertion. Unpublished cleanup runs
once where applicable; published shadows do not receive that cleanup.

Native failure tests require the exact injected error or panic payload. Borrowed
error cases check the public error type; their panic cases require the exact
boundary/occurrence payload. All CREATE cases check failure; the four nonpanic
cases also check model phase. They do not yet assert every exact public error
classification. The eight currentness
occurrences are a fixed expectation, not derived from a potentially broken trace.

Runtime, event, CWSR shadow, doorbell and gate leaves are scripted and drop-counted.
Actual ring/control/completion initialization runs against fixture storage;
completion storage is genuinely aligned for the existing initializer. CWSR BO
header validation and platform descriptors are not emulated as Linux evidence.
Earlier local Linux-helper tests remain separate checks, not composed acceptance.

## Attempts

- `r93-first-check`: compilation failed on a missing private authority import.
- `r93-first-integration`: missing fixture envelope/geometry imports prevented compilation.
- `r93-second-integration`: attempted access to private opaque identity fields
  failed to compile; exact set membership and cardinality replaced field sorting.
- `r93-third-integration`: private environment associated-type visibility and a
  test-only private bound required corrected internal visibility and a narrow allowance.
- `r93-fourth-integration`: all five tests failed because fake completion storage
  did not satisfy the real initializer's alignment. This was a fixture defect,
  not a production completion failure. An unrelated panic could not satisfy the
  required native-panic identity.
- `r93-aligned-integration`: all five tests passed after retaining aligned storage
  inside the original fake mapping; no alternate initializer was introduced.
- `r93-kfd-first`: 915 passed, one failed after stronger accounting assertions
  incorrectly treated USERPTR control as charged ordinary Host memory.
- `r93-kfd-corrected`: all 916 KFD tests passed after matching the full canonical
  Host layout and extending Device facts checks.
- `r93-clippy-preflight`: failed the manual-inspect lint; the Linux error callback
  now uses `inspect_err` without altering the returned error.
- `r93-clippy-corrected`: all-feature/all-target KFD Clippy passes with warnings denied.

No preliminary attempt substitutes for the final frozen-source gates. Formatting
and patch-context corrections are execution details, not extra acceptance runs.

## Mutation Oracles

Three mutations of the shared production algorithm compiled, each exited Cargo
with 101 and failed exactly the selected integration test at its intended
assertion. The complete 5,639-file source inventory matches the pre-mutation
baseline after restoration; no mutation remains.

| Mutation | Intended rejection |
| --- | --- |
| Remove pre-doorbell currentness | Occurrence 7 now retains an incorrectly installed doorbell. |
| Remove post-doorbell currentness | Occurrence 8 is never reached, so construction wrongly returns success. |
| Publish before CREATE opening checks | Occurrence 5 has published shadows despite rejecting before native CREATE. |

These are executable sequence-sensitivity checks, not formal proofs or GPU fault
injection. Their minimal patches and exact source snapshots are retained with
the final acceptance artifacts.

The [summary](test-summary.json), [source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[final gate record](raw/r93-final-source-gate.json),
[auxiliary record](raw/r93-auxiliary-results.json),
[restoration record](raw/r93-mutation-restoration.json) and
[complete source inventory](raw/r93-final-source-inputs.json) retain exact
commands, outcomes and identities. Raw logs/helpers preserve their executed
Unicode and whitespace; final code/documentation whitespace checks exclude the
`raw/` archive.

## Open Boundaries

NATIVE-2A.3 still needs integrated preparation-prefix failure, initialized-content
descriptor/premise comparisons, generic borrowed/non-`Send` returned preparation,
early external-runtime rejection with retained caller slots, exact platform-owner
identity, late output/ID/dependency failures, remaining projection/retain/partial-
map variants and local Linux gate/shadow composition. The old projection-rejection
constant is inside this fixture's larger aperture and must not be reused as a
supposed rejection. These gaps are tracked in the
[current swarm dispatch](../../runtime-a1-a2-swarm-dispatch-r83.md).

Unreturned callback prefixes, auxiliary/replacement custody, native generated
adoption/issue/completion, aggregate backing budgets, version journals, new
executable refinement and hardware/performance qualification remain open.
Terminal process-lifetime retention is not bounded resource closure. Concurrent
bootstrap remains unsupported; overlapping queue lifetimes do not imply it.

No SSH, GPU workload, remote staging or solver was started. No shared-machine
cleanup was necessary; unrelated local artifacts/processes were not removed.
Model/proof, resource-accounting, completion and manifest/lockfile inputs remain
unchanged from R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.
An unchanged negative-proof inventory is not a solver rerun or adapter proof.
No full A1/A2 completion, HIP/HSA parity or performance claim is made.
