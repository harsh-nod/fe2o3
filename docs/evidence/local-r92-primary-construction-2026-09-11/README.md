# Local R92 Primary Construction Evidence

Implementation baseline: signed R91
`a04060137e8df20f434682f7ee4dd6f06fb10174`, on both topic remotes.
This packet implements the
[NATIVE-2A.2 primary-owner contract](../../runtime-primary-queue-construction-custody-v1.md).
The containing signed commit records source and evidence together; topic-branch
publication is not a main merge.

## Status

All seventeen final frozen-source gates and eleven auxiliary checks pass.
The final suite contains 911 KFD tests. Thirteen new CPU functions cover the
primary settlement/capture/token helpers, ordinary preparation and local
gate/shadow changes. Three compiled mutations reject at their intended
assertions; all 5,635 source identities were exactly restored before the final
gates and remained unchanged through acceptance.

| Accepted check | Result |
| --- | --- |
| Runtime all-feature/all-target tests, GNU and musl separately | 2,491 passed, 5 ignored, 48 harnesses per target. |
| GNU runtime/host doctests | 108 passed. |
| Musl runtime doctests and separate host doctests | 91 and 16 passed. |
| Host library, GNU / musl | 258 passed with 4 ignored / 141 passed. |
| Typed macro fixtures / benchmark checker tests | 7 / 151 passed. |
| All-feature/all-target and production Clippy | Both pass with warnings denied. |
| Formatting, whitespace, dependency policy/tests and local CI test gate | Pass. |
| Standalone lockfiles | 32 checked. |
| Focused primary / ordinary / lower construction / Linux helpers | 8 / 1 / 6 / 20 passed. |
| Focused initialization / transitions / preparation / bind | 4 / 20 / 23 / 4 passed. |
| Production dependency audit | Pass: 43 packages, 8 allowed build scripts. |
| Unchanged proof-negative inventory | 686 files checked; no solver run. |

Focused tests are subsets of the full suites, not additional unique coverage.

Three read-only reviews found no remaining blocking issue after the cleanup-
panic correction. Primary owns implementation, integration, tests and publication.
This packet does not close NATIVE-2A.3's full same-session per-stage campaign,
NATIVE-2B/C auxiliary/replacement custody, Linux qualification or new refinement.
No SSH, GPU workload, remote staging or solver was started. No shared-machine
cleanup was necessary, and unrelated local build artifacts were not removed.

## Validation Scope

Eight primary-helper tests include public error auto-traits, exact boxed-owner
retention, pre-control/terminal classification and poison-before-cleanup-before-
retention order. Error/panic work is crossed with cleanup return/panic; the first
work panic survives even when a secondary payload's destructor would panic.
Success returns the original root once without cleanup or poisoning.

The production returned-preparation capture is tested from an empty slot with
an actual charged token, borrowed text and a non-`Send` drop-counted owner.
Later rejection/panic preserves exact token/root/pointer identity and performs
no native cleanup; an occupied output does not reinvoke the callback.
Mutable control/completion handoffs cross ten R88 failure modes per profile,
checking exact precursor/successor identity, native progress and existing Host/
Device accounts. Foreign/closed/occupied preflight and successful transfer are
tested separately. These are actual R88 fake-native owners, not Linux sessions.

One ordinary-constructor test retains real R89 packets/data/accounts through
prevalidation and post-preparation rejection/panic. Existing R89 stage/native
matrices now call the production ordinary/new-generation forwarding helper.
Four local Linux-helper tests cover checked gate success/rejection, exact disposed
shadow metadata, publication rejection, cleanup-once and subprocess abort on
payload-cleanup failure. Their mapped pages and mutexes are real local fixtures;
they do not execute KFD queue creation or GPU CWSR.

Source guards check primary/auxiliary publication placement, runtime-admission-
before-arm ordering, rooted assembly and both doorbell currentness checks.
They do not supply a full one-session allocation/initialization/CREATE/doorbell
fault matrix. That integrated production-path acceptance remains NATIVE-2A.3.

## Attempts

- `r92-first-check`: all-target check passed with one unused old-wrapper warning;
  that wrapper was removed and its regression now calls production settlement.
- `r92-first-tests`: five passed, one failed because the new test expected a
  mapped successor on closing-currentness failure. R88 retains the CPU precursor
  until the map call fully returns. The corrected test checks native progress
  independently, including a successful map prefix with precursor custody.
- `r92-kfd-local`: test compilation failed on a fixture's relative module path;
  the explicit `crate::queue::live` path corrected it.
- `r92-kfd-corrected`: 911 KFD tests passed.
- `r92-clippy-preflight`: all-feature/all-target KFD Clippy passed with warnings denied.

Formatting and patch-context corrections are present in the execution transcript,
not additional acceptance runs. No preliminary attempt substitutes for final gates.

## Mutation Oracles

Each mutation compiles, exits Cargo with 101 and fails exactly one selected test.
No mutation remains; restored source matches the entire pre-mutation inventory.

| Mutation | Intended rejection |
| --- | --- |
| Run cleanup outside its unwind catcher | Cleanup panic loses the root before terminal retention. |
| Omit final global-gate health checks | Checked finalization accepts a poisoned/unavailable runtime. |
| Discard unpublished metadata after payload cleanup | The disposed wrapper no longer retains its exact shadow descriptors. |

The [summary](test-summary.json), [source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[final gate record](raw/r92-final-source-gate.json),
[auxiliary record](raw/r92-auxiliary-results.json) and
[complete source inventory](raw/r92-final-source-inputs.json) preserve exact
commands, results and identities. Minimal mutation patches reproduce recorded
source differences. Raw logs/helpers retain their executed Unicode and whitespace;
final code/documentation whitespace checks exclude the `raw/` archive.

## Open Boundaries

The new local terminal guard starts before USERPTR-control entry; for the
diagnostic USERPTR-ring profile it now starts before ring allocation itself.
Default and executable-probe pre-control classifications remain unchanged.
Unpublished payload cleanup preserves the frozen contract while retaining its
disposed metadata; published shadows receive no implicit cleanup. Failure roots
are retained for process lifetime, not recovered into usable public authority.

Overlapping queue lifetimes do not imply concurrent-bootstrap support. Competing
constructors can enter runtime admission before either acquires the creation arm;
the losing attempt has crossed USERPTR effects and its failure is terminal, not
retryable `Busy`. Concurrent bootstrap remains unsupported pending a separate
pre-effect reservation protocol. Existing runtime construction is sequential.

Memory-session acquisition before return, generic callback-internal unreturned
prefixes, private source-complete preparation, auxiliary construction and early
recycled-replacement preparation remain outside this primary envelope. No new
ring/control/EOP/context-save backing charges or aggregate budget bounds are added.

Model/proof, resource-accounting, completion and manifest/lockfile inputs remain
unchanged from R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`.
The 686-file negative inventory is not a new solver run or proof of this adapter.
Native generated adoption/issue/completion, new executable refinement, Linux
qualification, matched HIP/HSA performance and full A1/A2/parity remain open.
