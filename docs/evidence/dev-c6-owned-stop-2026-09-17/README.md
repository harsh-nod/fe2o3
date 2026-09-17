# Generated Graph Owned Stop

CPU-qualified development above canonical C6 commit
`c29f4f46aed32a0317d5d1cfd39dfdbaa495cf57`. Accepted milestones remain Native
R125 CPU/test, Admission R118B C1-C3 and Resources R116/V3. This is not A1/A2,
#182, protected native composition, formal correspondence or HIP/HSA parity
acceptance.

## Correction

The owner loop advances graphs before generated operations. A generated
operation can finish settlement and decoding, publish its original reply and
retire its driver just before the next command phase dequeues Stop. Previously
the graph still considered that node active and quarantined an otherwise clean
Context without observing the ready cell.

Stop now cancels its unactivated suffix, inspects each original active entry
once, and consumes only already-ready generated cells through the ordinary
success/error handling. Pending generated cells and ordinary actions remain in
the roster. It then applies any remaining terminal cleanup and tries the same
Context graph release. The scan performs no issue, native poll, ordinary
submission release, flush, settlement or decode. It neither waits for producers
nor creates a second completion observer. An ambiguous remaining entry still
retains the Context until process exit.

Stop is ordered, not callback preemption. A Stop queued during settlement or
decoding is dequeued after that callback returns; its already-published result
is preserved. Stop dequeued before C4 does not perform C4 to obtain an output.

## Coverage

Eight added tests include 52 deterministic owned-loop scenarios and a direct
three-outcome regression. One-shot channels pause existing hook control flow
without sleeps, native execution or holding fixture locks across checkpoints.
Coverage includes queued/admitted-but-unactivated graphs, readiness-false
Adopting, Adopted, Issued, physical completion, settlement, decoder success,
decoder error/panic, and replies explicitly published before Stop is queued.
Kept and dropped graph observers have the same custody behavior.

Tests check exact receipt domains, success/error separation, serial cancellation
causes, independent ready nodes with mixed results, pending generated/ordinary
companions, unrelated parked custody, owner-thread disposal, cleanup/finalizer
disposition, reply credit and graph-slot release. Mixed pending cases assert no
additional backend call at Stop; they do not return a partial graph report or
claim native quiescence.

The new suite uses the production owner loop with scripted backend/decoder
hooks. Existing constructed Context and lower native evidence remain separate;
this packet does not execute a protected native graph or measure performance.
Native Stop/failure qualification, production verifier/refinement authority,
journals, aggregate residency and matched HIP/HSA performance remain open.

## Evidence

The final archive binds all four changed source files above the canonical base,
the complete source patch, exact GNU/scoped-musl executables and their complete
matching rosters. Scoped musl uses `FE2O3_HIP_SYS_DISABLE=1`; this does not qualify
unrestricted musl/HIP linkage.

GNU and scoped musl each pass 942 runtime and 271 host tests, with seventeen and
four ignored respectively. All 61 doctests, strict all-targets Clippy, formatting,
no-default-feature checks and unsafe-source policy pass. Parser calibration
rejects eighteen malformed transcripts. Source and executable hashes match before
and after the serial runs; the final audit checks closed records, exact commands,
ordering and matching cross-target test rosters.

The complete source patch has SHA-256
`9d994d83bf9e56a66f998d81b9d16d1ee361ac62639c96e8883ca2dcec471d74`;
the source manifest has SHA-256
`7f0c4140e51ccfd0ee75f92b79631678dec703e8c421ad68c62970cb845d1664`.
These source identities apply to canonical integration. Recorded absolute paths
identify the actual qualification worktree, not fresh executions elsewhere.

Two failed exploratory records are deliberately retained outside qualification:
`exploratory-settled-stop` reproduces the original unnecessary Context retention;
`exploratory-owned-stop` contains four initial assertion failures concerning the
test's retained graph reply owner and transitive cancellation states. The latter
were corrected to drop the manual graph owner and check exact
`DependencyCancelled` causes, without changing cancellation semantics. These
exploratory binaries/sources are not claimed as the final frozen-source run.
