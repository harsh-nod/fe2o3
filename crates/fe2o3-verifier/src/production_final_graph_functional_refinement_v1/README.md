# Prepared Final Functional Checkpoint

`ProductionPreparedFinalGraphFunctionalSubjectV1::try_new` takes borrowed
source/final canonical owners, the final epoch, and the checked target contract.
It retains independently validated canonical snapshots, both original kernel
rosters, every target decision, and the generated output-equivalence theorem.
It does not acquire the protected runtime, execute a proof, construct PLIRON,
run the mandatory schedule, or assert that the generated theorem is true.

`execute_effect_ir_derived` consumes this preparation and the exact V3 source
proof. It replays the semantic KIR owner, compares its exact source canonical
bytes, joins the source proof's MIR/contract/parallel subjects, then executes
and imports the generated theorem once. The pending execution retains the
original source proof, final receipt, staging policy, canonical subjects and
target records. Receipt access and `require_exact_subject` grant no final-graph,
artifact, or launch authority. The existing verified-owner APIs use the same
theorem construction, source-proof join, obligation identity and execution.

`complete_with_verified_graph(self, &mut ProductionVerifiedFinalGraphV1)`
consumes the pending execution and returns the existing
`ProductionFinalGraphFunctionalRefinementExecutionV2`. It checks the actual
owner-held target contract, exact final bytes and epoch, retained source bytes,
source/receipt subjects and the recomputed obligation. It revalidates the live
owner before these checks and again before moving the original receipts into
final custody. It accepts no runtime and neither constructs a graph nor reruns
the theorem. The graph remains borrowed; root-roster completion is still the
production caller's responsibility.

## Parent Integration

1. Before the mandatory schedule, prepare from the retained neutral/source KIR
   and exact optimized final KIR plus `final_graph_target_contract_v1` output.
   The contract's neutral graph must be that source canonical identity.
2. Preserve the existing authenticated root roster and its semantic-root,
   kernel-binding and reference identities. Execute against each addressed
   move-only V3 source owner; do not treat a module-wide KIR theorem as a
   complete roster of safe-reference proofs.
3. Retain each pending execution beside its original roster entry. Use the
   checked subject and proof bindings when integrating the final ranked views;
   do not remove the mandatory write-contract or ownership checks.
4. Pass the original final canonical owner and checked target contract into the
   normal sole final-graph owner and execute the complete mandatory schedule.
5. Consume each pending execution with `complete_with_verified_graph`, then
   reconcile the original authenticated root roster and retain the resulting
   existing execution type beside the sole final-graph owner. Do not call an
   executing legacy API again. The completion method uses the verified owner's
   immutable `target_contract()` accessor, not a caller-supplied contract.

The existing V1 obligation identity remains the semantic theorem identity; it
is not a digest of the checked target decisions. Exact target binding is
retained separately and must participate in the consuming custody join.

## Tests

The local unit tests require no runtime, process spawning, network, or GPU.
They cover canonical/epoch/target substitution, detached mutation, retained
kernel rosters, shared and nonidentical theorem recipes, and preparation without
execution. The completion comparison is tested against actual mandatory-schedule
owners, including changed source/final identities, actual epochs, target models,
neutral epochs, closure identities and extra target decisions. No-write fixtures
exercise the live-owner check without inventing a functional receipt.

The fixtures use the parent-added `fe2o3-target-spec` dev-dependency. Production
code requires no new dependency. A successful receipt-bearing completion still
requires a protected-runtime integration test; these tests neither synthesize
source proofs nor claim that recipe construction executes the theorem.
