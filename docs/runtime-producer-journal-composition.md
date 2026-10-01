# Runtime Concrete Journal Composition

This component connects the actual journal query implementation to
the actual producer-input validator and fold. Its qualified source is signed
`fef0f92cb6385c57d1b77c2eff41db967490beb1`, based on signed integration
`6793b910c2aa662df20763fbbd349cec0994677f`. Qualification uses explicitly composed
109+3 evidence, not success of the original rejected 112-stage campaign.
Historical calibration captures remain unqualified. Native implementation bytes
are unchanged by this component, which adds no CPU or GPU run.

## Composed Qualification

The original signed campaign accepted 109 stages, including all 89 distinct
actual-body mutations: 38 leaf, 21 conditional-composition, and 30 concrete
cases. It was rejected by its 16 GiB available-RAM gate before the closing
concrete positive, tool release, and signature checks. Its original 109 child
groups were all closed; the rejected packet and missing closing-input record
remain unchanged. Independent readback confirmed the resource-only stoppage.

A separate completion executed exactly those three missing checks against the
same signed source, tools, and prepared commands. Its cumulative 300-second
admission-wait budget spent 12.007302188 seconds; each command launched above
the unchanged 16 GiB floor. The four-thread, 120-second inner/130-second owned
proof bounds were unchanged. All three new child groups closed, and both
packets have member-verified, directory-fsynced durable custody.

The composed result has 89 unique qualified prefix negatives, zero new
negative executions, and three full positive brackets per root: 42/0 for the
six-input leaf, 64/0 for the eight-input conditional composition, and 214/0 for
the 44-input concrete composition. The original campaign is still rejected;
neither calibration observations nor duplicate cases are counted as new kills.
The independent prefix readback SHA-256 is
`c5b3bda8d16123440f8a960a6fd8df25222be1d9fa9f616381349515e315ee1d`;
the composed readback is
`b120471f71930fecff00108532d9d5f1919639cd17b1b32f1037f9289f8ccec4`.
The [public evidence packet](evidence/dev-producer-journal-composition-2026-10-01/README.md)
preserves both original archives, the thin signed-source bundle and both audit
records. Full local replay still has explicit external prerequisites. This
closes the stated component qualification, not A2, another milestone, or
HIP/HSA parity.

## Executed Path

`context_producer_journal_composition_v1.rs` includes the complete existing
36-input journal query closure and the actual enrollment declaration. The
concrete closure has 44 unique Rust inputs. It uses the real journal types, not
layout-matched replacements or executable type conversions.

Its observer borrows the runtime context, complete journal-bearing versions,
and retained input root. All unread owner fields remain opaque and the entire
borrowed binding is framed. Four production forwarding macros call the actual
journal methods. The per-input callback invokes the actual validation macro,
and reconciliation invokes the actual fold macro.

Journal observations are derived only in specification code from the actual
journal and incoming active or queued reference cursor. In particular, status
uses the same reference selected before the production cursor increment.
Active lookup retains its intentional lack of a disposal-terminal pre-gate;
active status and both queued queries retain their existing gates.

The callback records result, cursor movement and ghost call trace before
returning an error to the fold. The fold therefore retains the reached error
prefix, stops at the first error, and continues validation after an `Unknown`
status. Receipt advancement is derived from actual cursor changes, not from
whether the result is successful.

## Remaining Boundaries

Only live-allocation and credit results are supplied independently per reached
input. The proof does not equate these observations with native
`validate_live`, a fresh credit-account lock, Arc identity, shared interior
state, or persistent account snapshots. A wrapper call trace is not an exact
count of calls inside a journal query.

The runtime context declaration projection, comparison derives, standard
library contracts, Verus/Rust lowering, allocation, unwinding, and machine
execution remain explicit trust or refinement boundaries. This candidate does
not establish protected launch authority, GPU correctness, performance parity,
or completion of A2.

## Refactoring Scope

Runtime declarations, compared conditional journal declarations, per-input
outcome formulas, and pure prefix composition logic are separate shared files.
The conditional root still supplies independent typed helper returns. The
concrete root supplies no executable journal returns.

Each root defines its own concrete ghost `Source`, `source_len`, and
`source_answers`. There are no abstract assumed contracts or new axioms.
Source-only inverse controls reconstruct the exact previously qualified
definitions and conditional composition from the split files.

The conditional leaf closure changes from three to six inputs; the conditional
composition changes from four to eight. Their previous 42- and 64-obligation
results do not qualify these new closures. All 38 leaf and 21 composition
mutation names and source deltas remain represented. The changed closures have
their own discovery, diagnostic calibration, and composed qualification above;
the earlier results and the calibration described below are not substituted
for that qualification.
The standalone three-input fold and 38-input journal-wrapper closures remain
byte-identical to the integrated base.

Retained diagnostic fixtures are replayed only against exact reconstructed
predecessor source bytes. They are not promoted to new-root mutation kills.
The new guard intentionally exposes `expected_verified: null` and
`qualified: false`.

## Discovery and Calibration

Initial unsigned discovery measured 42/0 for the six-input leaf, 64/0 for the
eight-input conditional composition, and 213/0 for the original concrete root.
The initial 89-case observation campaign was rejected after its Pending-return
fault hit the unchanged command timeout. Its 64 normally completed observations,
partial timeout diagnostic, unrun roster, closing release, and all 74 launched
group closures remain retained. That campaign is not complete or qualified.

The concrete proof now makes the defined `journal_answers` specification opaque
to generic prefix reasoning, explicitly reveals it in the actual validator,
and proves a separate credit-field projection lemma. This changes neither the
formula, contracts, native calls, nor the original mutation edits. The other
proof closures are unchanged. No axiom or assumed executable contract is added.

A bounded full-root experiment measured 214/0, the original Pending fault at
213/1, then 214/0 under the same four-thread, 120-second inner/130-second owned
command limits. A separate complete 34-stage capture then obtained two full
214/0 positive brackets at different projection paths and all 30 concrete
observations at 213/1, with tool-release brackets and all 34 groups closed.
Both packets have complete per-member durable archive readback and independent
audits. This supports the proof decomposition; it does not establish the exact
cause of the earlier timeout or a runtime performance improvement.

The portable fixture artifact preserves all 38 leaf and 21 conditional cases
against their identical captured closures, plus all 30 new concrete cases.
Its SHA-256 is
`2891b763f597b1b3b936509153377a8c2c55fdd4d0fa1e6cd80fdd2e6d980e3a`.
Only diagnostic `file_name` fields and path substrings in `rendered` text are
normalized, with reversible checks; stdout and original raw hashes are retained.
The tests reconstruct each complete mutated source projection from repository
sources and compare every source hash before replaying diagnostics.

Eight forwarding faults fail their actual result-equality postconditions. The
eager-status fault fails only the wrapper ghost-trace postcondition at an early
`?` exit; it does not prove a failed result equality or an inner query-call count.
The five credit-argument faults concern the external-credit argument ghost
trace, not native credit locking or freshness. Counts are per-root verifier
results, not additive runtime properties. The exact 38/21/30 signed roster is
covered by the composed qualification above. Complete historical replay also
requires the separately retained raw packets, tools and native CPU artifacts,
not just these fixtures.

## Source Checks

```sh
python3 -I -B crates/fe2o3-runtime-model/verus/check-producer-journal-composition.py
python3 -I -B crates/fe2o3-runtime-model/verus/test-producer-journal-composition.py
python3 -I -B crates/fe2o3-runtime-model/verus/test-producer-journal-composition-mutations.py
python3 -I -B crates/fe2o3-runtime-model/verus/test-producer-journal-composition-diagnostics.py
```

The guard binds the complete runtime/model inventory, both conditional roots,
the standalone wrapper source binding, and the exact recursive concrete proof
closure. These checks construct and reject source controls only; they do not
run a compiler, solver or runtime.
