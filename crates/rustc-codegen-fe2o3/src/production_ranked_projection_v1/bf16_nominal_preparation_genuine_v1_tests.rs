//! Genuine factory/driver hook. Caller supplies existing actual source owners;
//! this file creates no positive source fixture, semantic owner or capability.
use super::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1;
use super::bf16_nominal_dense_v1::{NominalCapabilityPassV1, run_nominal_capability_dataflow_v1};
use super::bf16_nominal_layout_return_v1::{
    NominalTensorOccurrenceV1, with_nominal_layout_return_v1,
};
use super::bf16_nominal_source_preparation_v1::with_nominal_source_preparation_v1;
use super::canonical_assertion_facts_v1::{
    with_nominal_canonical_facts_observation_v1, with_nominal_capability_consumer_v1,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
    ProductionPreRankedKirOwnerV1,
};
type Result<T> = std::result::Result<T, QueryError>;

type FinalOccurrenceVisitor<'v> =
    dyn for<'a, 'w> FnMut(&NominalTensorOccurrenceV1<'a>, &mut Budget<'w>) -> Result<()> + 'v;

pub(super) fn observe_prepared_dense(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let entry = budget.storage();
    with_prepared_dense_final(owner, source, inventory, budget, &mut |_, _| Ok(()))?;
    // SAME complete route participates in the existing exact W/P/F probes.
    final_candidate::observe(owner, source, inventory, budget)?;
    super::bf16_nominal_final_candidate_v1::retained_effects_genuine::observe(
        owner, source, inventory, budget,
    )?;
    super::bf16_nominal_source_preparation_v1::observe_rich_source_comparison_for_test_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )?;
    super::bf16_nominal_source_preparation_v1::observe_root_source_preparation_for_test_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )?;
    super::bf16_nominal_source_preparation_v1::observe_root_cfg_preparation_for_test_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )?;
    let assertions =
        super::bf16_nominal_source_preparation_v1::observe_root_assertion_preparation_for_test_v1(
            owner,
            inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
        )?;
    // Actual measured counts, not a positive-assertion coverage assumption.
    assert!(assertions.blocks > 0);
    assert!(assertions.true_decisions <= assertions.assertions);
    // Test-harness telemetry only: fixed labels and scalar fields, retained by
    // the qualification runner's capped stderr. This does not grant admission,
    // prove a positive Assert exists, or measure formatting/I/O on the compiler
    // resource ledger. All evaluator counts above came from the actual view.
    eprintln!(
        "fe2o3-root-assertion-observation-v1 root={} call_block={} permutation={:?} blocks={} assertions={} true_decisions={} logical_work={}",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        assertions.blocks,
        assertions.assertions,
        assertions.true_decisions,
        assertions.logical_work,
    );
    assert_eq!(budget.storage(), entry);
    Ok(())
}

fn with_prepared_dense_final(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    inspect: &mut FinalOccurrenceVisitor<'_>,
) -> Result<()> {
    let original = budget.work_ledger_identity_v1();
    let entry = budget.storage();
    with_nominal_source_preparation_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |inputs, budget| {
            let prepared_floor = budget.storage();
            assert!(prepared_floor > entry);
            with_nominal_canonical_facts_observation_v1(
                owner,
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |facts| {
                    with_nominal_capability_consumer_v1(facts, |site, consumer| {
                        assert!(std::ptr::eq(site.owner(), owner));
                        assert!(std::ptr::eq(site.inventory(), inventory));
                        let mut final_visits = 0usize;
                        let mut observed_passes = [0usize; 3];
                        let mut observed_permutation = [u8::MAX; 4];
                        let mut visit = |pass: NominalCapabilityPassV1,
                                         authenticated: &AuthenticatedNominalCallerV1<'_>,
                                         budget: &mut Budget<'_>|
                         -> Result<()> {
                            budget.charge_work(32)?;
                            assert!(budget.work_ledger_identity_v1() == original);
                            assert!(budget.storage() >= prepared_floor);
                            let pass_index = match pass {
                                NominalCapabilityPassV1::Initial => 0,
                                NominalCapabilityPassV1::Repeated => 1,
                                NominalCapabilityPassV1::Final => 2,
                            };
                            observed_passes[pass_index] += 1;
                            if pass == NominalCapabilityPassV1::Final {
                                assert_eq!(observed_passes, [1, 1, 1]);
                                observed_permutation = with_nominal_layout_return_v1(
                                    authenticated,
                                    budget,
                                    |occurrence, budget| {
                                        observe_exact_final_rows(occurrence, budget)?;
                                        // Candidate only. The actual final visitor lends the
                                        // same C3 occurrence and original ledger; retaining a
                                        // copied operation would not grant normal admission.
                                        super::bf16_nominal_ranked_proxy_v1::
                                            with_nominal_ranked_tensor_proxy_v1(
                                                occurrence,
                                                budget,
                                                |proxy, budget| {
                                                    budget.charge_work(64)?;
                                                    assert!(std::ptr::eq(
                                                        proxy.occurrence(),
                                                        occurrence,
                                                    ));
                                                    let super::ProductionRankedOperationV1::TensorLayout {
                                                        contract,
                                                        convergence,
                                                        active_lanes,
                                                        binding: Some(binding),
                                                    } = proxy.operation() else {
                                                        panic!("genuine nominal proxy is not a tensor row")
                                                    };
                                                    assert_eq!(
                                                        *contract,
                                                        fe2o3_kernel_ir::TensorLayoutContractV1::
                                                            gfx942_mfma_bf16_f32_m16n16k16_wave64()
                                                                .with_zero_filled_predicate_inputs(),
                                                    );
                                                    assert_eq!(
                                                        *convergence,
                                                        super::TensorConvergenceAttr::UniformSubgroup,
                                                    );
                                                    assert_eq!(*active_lanes, 64);
                                                    assert_eq!(binding.argument_count(), 4);
                                                    assert_eq!(
                                                        binding.result_root(),
                                                        occurrence.binding_digest(),
                                                    );
                                                    assert!(budget.work_ledger_identity_v1() == original);
                                                    // Keep all existing success/error/panic and
                                                    // sticky-denial/floor probes INSIDE this scope.
                                                    inspect(occurrence, budget)
                                                },
                                            )?;
                                        assert_eq!(occurrence.return_rows().len(), 4);
                                        assert_ne!(
                                            occurrence.call_coordinate().block.function,
                                            occurrence.matrix_coordinate().block.function,
                                        );
                                        Ok(occurrence.permutation())
                                    },
                                )?;
                                final_visits += 1;
                            }
                            Ok(())
                        };
                        let (run, ()) = run_nominal_capability_dataflow_v1(
                            site,
                            inputs,
                            consumer,
                            &mut visit,
                            |effects, run, consumer| {
                                consumer.charge_work_v1(16)?;
                                assert_eq!(effects.len(), site.function().blocks().len());
                                assert_eq!(run.query_visits, [1, 1, 1]);
                                assert_eq!(run.authenticated_visits[2], 1);
                                assert!(!run.array_destination_has_origin);
                                Ok(())
                            },
                        )?;
                        assert_eq!(run.query_visits, [1, 1, 1]);
                        assert_eq!(final_visits, 1);
                        assert_eq!(
                            observed_permutation,
                            owner
                                .bf16_call_instance_emission_v1()
                                .unwrap()
                                .return_permutation(),
                        );
                        Ok(())
                    })
                },
            )
        },
    )?;
    assert!(budget.work_ledger_identity_v1() == original);
    // The generalized final callback may retain its own extra storage. Every
    // production scope refunds only its separately accepted reservations.
    assert!(budget.storage() >= entry);
    Ok(())
}

fn observe_exact_final_rows(
    occurrence: &NominalTensorOccurrenceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Use,
    };
    use fe2o3_kernel_ir::{Terminator, Type};
    // Fixed four-row observations; all inventory reads use qualified indices.
    budget.charge_work(256)?;
    let authenticated = occurrence.authenticated();
    let site = authenticated.site();
    let checked = authenticated.candidate().call();
    let emission = checked.emission();
    assert!(std::ptr::eq(site.owner(), emission.owner()));
    assert!(checked.belongs_to(site.inventory()));
    assert_eq!(occurrence.call_coordinate(), checked.call().coordinate);
    assert_eq!(occurrence.matrix_coordinate(), checked.matrix().coordinate);
    assert_eq!(occurrence.permutation(), emission.return_permutation());
    assert_ne!(
        occurrence.call_coordinate().block.function,
        occurrence.matrix_coordinate().block.function,
    );
    let returned_at = occurrence.return_block();
    let helper = checked.helper();
    let inventory_index = helper.blocks.start + returned_at.coordinate.block as usize;
    assert!(std::ptr::eq(
        &site.inventory().blocks()[inventory_index],
        returned_at,
    ));
    assert_eq!(returned_at.coordinate.function, helper.coordinate);
    let Terminator::Return { values } = returned_at.terminator else {
        panic!("actual qualified helper Return absent")
    };
    assert_eq!(values.len(), 4);
    let permutation = occurrence.permutation();
    for (index, row) in occurrence.return_rows().iter().enumerate() {
        let producer = permutation[index] as usize;
        assert_eq!(
            row.caller_definition(),
            Definition::Result {
                operation: checked.call().coordinate,
                result: index as u32,
            }
        );
        assert_eq!(
            row.helper_return(),
            Use::TerminatorOperand {
                block: returned_at.coordinate,
                operand: index as u32,
            }
        );
        assert_eq!(
            row.matrix_definition(),
            Definition::Result {
                operation: checked.matrix().coordinate,
                result: producer as u32,
            }
        );
        assert_eq!(
            row.caller_value(),
            checked.call().operation.results[index].id
        );
        assert_eq!(row.caller_value(), emission.call_results()[index]);
        assert_eq!(row.return_value(), values[index]);
        assert_eq!(row.return_value(), emission.helper_return()[index]);
        assert_eq!(
            row.matrix_value(),
            checked.matrix().operation.results[producer].id
        );
        assert_eq!(checked.call().operation.results[index].ty, Type::F32);
        assert_eq!(checked.matrix().operation.results[producer].ty, Type::F32);
    }
    // The final array components are ordinary f32 values, not a result
    // accumulator; the unchanged dense run asserts no destination origin.
    Ok(())
}

#[path = "bf16_nominal_final_candidate_genuine_v1_tests.rs"]
mod final_candidate;

#[path = "bf16_nominal_dense_final_genuine_v1_tests.rs"]
mod final_controls;

pub(super) fn inspect_dense_final_controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    final_controls::inspect(owner, source, inventory, inventory_storage, budget)?;
    final_candidate::controls(owner, source, inventory, inventory_storage, budget)?;
    super::bf16_nominal_final_candidate_v1::retained_effects_genuine::controls(
        owner,
        source,
        inventory,
        inventory_storage,
        budget,
    )?;
    super::bf16_nominal_source_preparation_v1::root_source_preparation_controls_for_test_v1(
        owner,
        inventory,
        inventory_storage,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )?;
    super::bf16_nominal_source_preparation_v1::root_cfg_preparation_controls_for_test_v1(
        owner,
        inventory,
        inventory_storage,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )?;
    super::bf16_nominal_source_preparation_v1::root_assertion_preparation_controls_for_test_v1(
        owner,
        inventory,
        inventory_storage,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
    )
}
