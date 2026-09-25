# Completion Journal Effects

Status: qualified shared control/journal-effect composition from signed
`957c5ce64`, not Context executable refinement. The
[38-stage packet](evidence/dev-completion-journal-effects-2026-09-25/README.md)
passes; A1/A2 and HIP/HSA parity remain open.

## Production Boundary

The completion suffix shares its input-release/writer-settlement prefix with
Verus. Input release still validates both complete retained rosters before
releasing either class, releases stable readers first, and skips producer
release when the prevalidated producer root is absent. Writer settlement uses
the production outcome enum and its exact Success/NoEffect/Unknown dispatch.
No Context precheck, map update, quarantine action or unwind boundary is moved.

The extracted producer-release helper is private and is called only after the
existing complete-roster validation and successful stable-reader release.
Dependency release and status publication remain after the prefix. They are
not assumed to be journal-neutral: a failed custody check may quarantine the
Context and mark writers Unknown.

## Proof Boundary

`context_completion_journal_paired_v1.rs` extends the existing constructor-origin
owner lifecycle proof environment. `CompletionJournalPairV1` is a proof adapter,
not a RuntimeContext. It executes real shared journal methods and a separate
logical implementation, starting with represented storage and its invariant.

The prefix consumes explicit, paired consumer/reference identities, optional
stable/producer/writer presence, and stage-entry capacity observations. Missing
input roots are no-ops, not empty-roster journal releases. A writer, when present,
belongs to the same consumer. The journal theorem does not establish that these
values came from the actual Context maps or allocator observations.
Constructing a matching journal quiescence-evidence value does not prove device
quiescence; that observation remains a Context/backend premise.

The intended correspondence covers:

- exact actual/logical result correspondence and final journal representation;
- preservation of the logical journal invariant and immutable input identities;
- stable-release failure retaining the initial journal contents;
- producer-release failure retaining the successful stable-release prefix;
- writer failure retaining the successful input-release prefix; and
- successful Success, NoEffect or Unknown writer effects, including absent-writer
  completion.

Every error-prefix state is **pre-quarantine**. Context can subsequently change
the journal through error translation and quarantine. These intermediates are
not asserted to be the state returned by a production Context method.

The shared outer prefix is instantiated with these journal adapters only.
There are no dependency/publication success stubs and no claim about a final
Context return, callbacks, map/root removal, retained backing, unwinding,
physical GPU completion or eventual progress.

## Qualification Boundary

The historical completion-control packet authenticates its original source;
it does not qualify the nested-prefix delta. Its textual adjacent-call mutation
must not be silently dropped or repinned. The successor campaign covers both
macro scopes, the independent effect oracle, optional branches, each failure
prefix and outcome substitution, with positive brackets and retained replay.

Whole-root development proof results and CPU regression results are separate
from that authenticated campaign. Global proof registration, Context map and
quarantine correspondence, producer-first reconciliation, protected generated
execution, native/resource qualification and matched performance remain open.

## Campaign Design

Keep historical controllers and pins unchanged. Authenticate the inherited
462-input owner closure and completion-control sources, then require the exact
new root extension, production delta and unchanged inherited contracts.
Qualification needs whole-effect and control positive brackets, not only the
eleven-obligation completion module. Record the exact verifier command and
selection; the development JSON reports `is-verifying-entire-crate: true` even
for `--verify-only-module production::completion` and is not scope authority.

Preserve all 22 historical control mutations. Input/writer mutations belong in
the new prefix; dependency/publication mutations remain in the outer body. The
writer/dependency swap must move only the dependency block into the prefix
between input release and writer settlement, removing its outer occurrence.
Moving the whole prefix after dependency release tests a different defect.

Add shared-body effect mutations for skipped stable release, continued stable
error, skipped/inverted producer presence, swallowed producer error, swapped
Success/NoEffect, Unknown routed through settlement, and discarded writer
results. Freeze logical-oracle bytes and calibrate mutation names, file sets,
selectors and commands. Reuse the authenticated owned runner, strict logical
negative classifier, tool/source brackets and retained replay. Resource-limit
failures are rejected attempts, never accepted negatives.

## Implemented Qualification Gate

`check-completion-journal-effects.py` authenticates 477 exact inputs, including
the two historical closures, the signed `024f2f78f` production integration and
the new checker/calibration sources. Historical checkers are unchanged. Each
of the 22 adapted control mutations must flatten to its exact historical defect;
eight additional shared-body mutations leave the logical oracle unchanged.
The 38-stage campaign requires whole-effect and control positive brackets,
all 30 logical negatives, the frozen 1,271-obligation owner regression and
source/tool continuity. Its eight CPU calibration groups pass.

The extended development root initially exceeded the solver resource limit in
`owner_constructor_unknown_disposal_witness_v1`; preserving the old crate name
did not resolve it. One local `hide(logical::producer_invariant_v1)` prevents
unnecessary invariant expansion in that caller. The callee already exports
invariant preservation. All old assertions and contracts remain unchanged,
and no solver limit or trusted premise changes. The new gate permits exactly
this inherited proof-body delta, not a historical evidence repin. With that
change, the full extended development root verifies 1,282 obligations with zero
errors and no diagnostics. The earlier failures remain diagnostic evidence.

The signed-source campaign now passes both 1,282-obligation whole-effect
brackets, both eight-obligation control brackets, all 30 logical negatives and
the frozen 1,271-obligation owner regression. Retained replay, six packet
calibration groups and five inherited cleanup tests pass. Both owned scratch
trees are fully retained before exact removal and independent absence. A
passing whole-root development run alone was not used as their substitute.
