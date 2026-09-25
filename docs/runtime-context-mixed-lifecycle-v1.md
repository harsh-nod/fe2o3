# Constructor-Origin Mixed Input Lifecycle

This extends the bounded owner-journal lifecycle with one `AcquireMixed` event.
It does not close A1/A2, issue #182, protected Worker execution, Context
reconciliation, native refinement or HIP/HSA parity.

## Composition

The actual and independent logical event records carry the same consumer, both
request classes, both original output rosters, both final output rosters and
one result. Admission matches only inputs. Correspondence derives the result,
both final outputs and the represented final owner.

The logical acquisition's intermediate stable-reader commit remains ghost-only.
The constructor-origin trace records one event and one final state, not two
separately visible acquisitions. Successful acquisition appends both typed
reference histories; rejection appends neither, including when original output
buffers already contain references. Neither path creates writer issuance.

Preservation composes the existing stable and producer histories, incarnation
frontiers, shared-capacity accounting and journal frames. The reached-state
theorem derives the actual mixed operation's storage preconditions. Individual
request lengths remain machine-bounded without assuming that their sum cannot
overflow; combined overflow remains a modeled rejection.

The existing reader trace datatype and constructor are reused. Its phase-specific
append wrapper retains its previous contract; a shared append helper supports the
new event without introducing another trace model. The earlier mixed development
root is a one-line alias of the canonical lifecycle root.

## Executable Witnesses

The setup executes real paired constructors and operations, not manually
populated represented storage. It first acquires/releases stable references twice
and a producer reference once. A second allocation is then enrolled, and a new
writer Begin advances the original allocation to attempt epoch 2.

One mixed call reuses slot zero in both arenas with stable incarnation 3 and
producer incarnation 2. Its consumer has no writer slot. The witnesses cover:

- Valid stable preflight followed by invalid pending input, with unchanged owner
  and both output buffers.
- Shared-capacity rejection after mixed success, preserving occupied output
  buffers and both issuance histories.
- Success and NoEffect settlement, reuse of the producer's writer slot for a
  different writer, and correct status of the retained original producer read.
- Release of both reader classes without resetting incarnation frontiers.
- Unknown producer status retaining the member and both reader classes while
  the unrelated stable input remains valid.

Constructor allocation/storage observations remain explicit external inputs.
Release witnesses construct identity-matched quiescence evidence: they prove
the journal's response to that evidence, not device quiescence or its production
origin. The Unknown witness does not release potentially live resources.
The split release helper preserves the exact constructor origin, both state
prefixes and prior event sequence while appending the paired stable release.
The final producer release checks that this complete incoming prefix survives.

## Qualification Boundary

`check-owner-mixed-lifecycle.py` reconstructs and authenticates the frozen
459-input mixed-acquisition baseline at signed `3db455f03`. It admits only the
reviewed eight modified files, the new witness and its two qualification
controls, for 462 inputs. Historical checkers, pins and evidence are unchanged.
They are replayed on their original inputs, not repinned against this extension.

The campaign requires whole-root positive brackets, nine logically rejected
mutations, frozen mixed/stable regressions, pinned tool-closure brackets and
checker calibration. Fixed projection conclusions test missing result/output
agreement and missing storage admission; simply weakening those predicates
would not otherwise guarantee a failing proof. Frontend errors, timeouts and
solver exhaustion are not accepted as logical negatives.

The checker calibration has eight CPU-only groups. Its optional `--campaign`
mode adds relocated read-only replay and four rehashed-receipt corruptions;
those two groups require an already complete campaign and never launch Verus.

Source-bound campaign acceptance is pending. Scoped development results do not
replace that gate. Global proof-inventory registration, Context maps and original
binding authentication, writer/read transaction composition, panic/unwind,
async carriage and producer-first completion reconciliation remain separate
obligations.

The 2026-09-24 whole-root development run verifies 1,271 obligations with zero
errors and no diagnostics using pinned Verus `0.2026.08.09.92f466f`, four threads,
the default SMT limit and a 1,200-second wall bound. Eight checker groups and the
190-file tool-closure check pass. This measured count is the campaign's expected
positive count, not a substitute for its signed-source positive brackets,
logical negatives, frozen regressions and retained replay.

Development failures remain retained: the original combined release witness
exceeded the solver limit, the extracted helper initially named the logical
instead of concrete frame predicate, and stronger trace continuity needed
explicit roster equalities and reached trace lengths. One exploratory selector
used an unsupported multi-wildcard pattern. None is a qualifying negative.

## Next Production Boundary

Refine the existing retained-completion gate in `context/peer_reconciliation.rs`
and `RuntimeContextV1::settle_terminal_submission_v1`, without adding a second
scheduler. Physical consumer success is retained before producers are reconciled;
logical success requires every explicit dependency and producer-read status to
succeed. Both reader releases and optional writer settlement must precede public
status/callback publication. Later failure may follow an earlier successful
release, so this needs a failure-prefix proof, not a claim of atomic settlement.
Context map/custody validation, unwind behavior and backend observation contracts
must remain explicit until their separate correspondence is established.
