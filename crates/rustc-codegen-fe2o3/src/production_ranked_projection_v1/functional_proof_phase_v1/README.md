# Ranked Functional Proof Phases

This child owns only proof custody. It does not create a live graph owner,
admit write contracts, or grant launch authority. The pipeline is unchanged.

## Integration Hooks

`AuthenticatedRankedVerificationRosterV1` remains the carrier through lineage:

1. Call consuming `prepare_final_graph_functional(runtime, semantic_kir,
   final_canonical, final_epoch, target_contract, timeout_seconds)` before the
   mandatory final schedule. This executes the final theorem for each existing
   authenticated source root, retaining the original source execution. Every
   source root, original roster identity/order, source owner, evidence and
   induction binding is checked before execution. Immutable theorem subjects
   are constructed before execution; they are not verified graph owners.
2. Borrow each pending execution through
   `root.verification().prepared_final_execution()` for the parent's checked
   final-contract transport. The exact canonical bytes, epochs, all target
   records and signed receipt remain retained. Do not reinterpret this final
   `SafeReferenceMirToLivePliron` receipt as a legacy per-effect receipt.
3. Run the actual mandatory final schedule independently. Then call consuming
   `complete_final_graph_functional(&mut verified_graph)`. It preflights every
   pending subject against the borrowed owner and consumes each pending proof
   via `complete_with_verified_graph`. No runtime or theorem execution occurs
   in this transition. Failures return no partially completed roster.
4. Keep borrowing the same roster for semantic lineage and the exact protected
   reference roster check. `aggregate_verus_execution()` returns the original
   source execution in Source, Prepared and Completed phases. Middle-end,
   induction, semantic-contract and parallel-contract payloads never move out
   during phase transitions.
5. At the existing late functional-custody boundary, consume
   `into_completed_final_graph_functional_roster()`, then `into_parts()`.
   Roots expose the existing identity metadata, `verified()` for the original
   source derivation, and `execution()` / consuming `into_execution()` for the
   completed V2 result. Preserve `require_exact_protected_reference_roster_v1`,
   exact per-root/reference matching, `validate_final_graph_functional_root_v1`,
   canonical order checks, live-owner revalidation and final custody identity
   construction. Remove only the now-redundant late runtime execution.

The legacy `into_final_graph_functional_roster()` entry remains Source-only.
It rejects Prepared/Completed roots rather than discarding final proof custody.
Do not fall back to it after a failed phase transition.

The original canonical roster hash recipe and ordering are unchanged. Phase
transitions recompute that recipe from retained evidence; descriptor order is
kept separately from source storage order. Source-root and canonical-kernel
joins are metadata validation, not a replacement for protected reference proof
or the final schedule. Completed terminal roots retain the remaining lineage
payload until `into_execution()`; lineage must borrow the roster before then.

## Central Verification

Test filter: `production_ranked_projection_v1::functional_proof_phase_v1`.
Also retain existing ranked-roster, execution-view and semantic-lineage tests.

The new offline tests use actual semantic/ranked/lowered owners without
functional staging to check identity/order, complete kernel/source joins,
evidence preservation and rejection of structurally checked but unproved
roots. Homogeneous-phase tests exercise only non-authoritative preflight
markers. They do not manufacture executed proofs or signed receipts.

A receipt-bearing Source -> Prepared -> Completed success test still needs
the protected runtime. It must assert unchanged source receipt bytes and
obligation identities, unchanged original roster identity/order and lineage
evidence, and unchanged pending/final receipt bytes across completion. Final
graph, source, epoch and target-record substitutions must fail. No compiler
test execution was performed by this worker; the parent owns central builds.
