use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

const FLOOR: usize = 23;

fn source_inputs() -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let source = super::production_borrowed_aggregate_replay_v1_tests::owner_source();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        source.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "borrowed_replay_root",
            [90; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (source, launch)
}

fn materialize(
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionPreRankedKirOwnerV1, ProductionPreRankedKirErrorV1> {
    let (source, launch) = source_inputs();
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
}

fn with_owner(next: impl FnOnce(&ProductionPreRankedKirOwnerV1, &mut ArgumentBudgetV1<'_>)) {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(&mut budget).expect("actual borrowed source must materialize");
    assert_eq!(budget.storage(), FLOOR);
    let retained = owner.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let floor = budget.storage();
    next(&owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn borrowed_source_materializes_only_a_sealed_pre_ranked_owner() {
    with_owner(|owner, budget| {
        assert_eq!(
            owner.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::Borrowed
        );
        assert!(!owner.grants_artifact_or_launch_authority());
        assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
        let rows = &owner.helper_memory;
        let seal = rows
            .borrowed_source
            .as_ref()
            .expect("mandatory replay seal");
        assert_eq!(seal.source_identity, owner.semantic_ssa.identity());
        assert_eq!(&seal.graph_identity, owner.executable.canonical().identity());
        assert_eq!(
            seal.functions().len(),
            owner.executable.module().functions.len()
        );
        assert_eq!(seal.functions().len(), 3);
        assert_eq!(owner.correspondence.borrowed_aggregate_fields.len(), 2);
        assert_eq!(owner.correspondence.borrowed_aggregate_calls.len(), 6);
        assert!(rows.unit_source.is_empty());
        assert!(matches!(rows.capture, HelperOccurrenceCaptureV1::Absent));
        assert!(rows.allocations.is_empty() && rows.accesses.is_empty());
        assert!(rows.control.is_empty() && rows.edge_bindings.is_empty());
        assert_eq!(
            rows.functions
                .iter()
                .filter(|kind| **kind == RetainedHelperKindV1::Borrowed)
                .count(),
            2
        );
        let correspondence_bytes =
            borrowed_aggregate_correspondence_bytes_v1(&owner.correspondence, budget).unwrap();
        assert!(correspondence_bytes > 0);
        let helper_bytes = std::mem::size_of::<SealedHelperMemoryV1>()
            + rows.functions.capacity() * std::mem::size_of::<RetainedHelperKindV1>()
            + rows.associations.capacity() * std::mem::size_of::<RetainedHelperAssociationV1>()
            + seal.retained_storage()
            - std::mem::size_of::<SealedBorrowedAggregateReplayV1>()
            + correspondence_bytes;
        assert_eq!(rows.storage.retained_storage(), helper_bytes);
        assert_eq!(
            owner.retained_analysis_storage_v1(),
            owner.executable_storage().retained_storage()
                + owner.assert_origin_storage().payload_storage()
                + helper_bytes
        );
        let (inventory, receipt) =
            CanonicalKirInventoryV1::derive(owner.executable(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        owner
            .with_checked_helper_memory_v1(&inventory, budget, |memory, budget| {
                for (index, row) in owner.correspondence.lowered_functions.iter().enumerate() {
                    if row.role == SemanticKirFunctionRoleV1::InternalHelper {
                        assert!(matches!(
                            memory.local_frame(index, budget),
                            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                        ));
                    }
                }
                Ok(())
            })
            .unwrap();
        drop(inventory);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn borrowed_owner_keeps_legacy_verification_ranked_and_source_output_closed() {
    with_owner(|owner, _| {
        for consumer in [
            "materialized ranked receipt",
            "ranked attachment",
            "source-ranked projection",
        ] {
            assert!(matches!(owner.require_legacy_helper_policy_v1(consumer),
                Err(ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable { consumer: actual })
                    if actual == consumer));
        }
        assert!(matches!(
            source_output_replay_v1(owner),
            Err(
                ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                    consumer: "source/output replay",
                }
            )
        ));
        assert!(
            lower_module(&owner.semantic_ssa, owner.limits, Some(&owner.launch_roots)).is_err()
        );
    });
}

#[test]
fn missing_borrowed_candidates_cannot_fall_back_to_raw_or_local_helpers() {
    with_owner(|owner, budget| {
        for mutation in 0..5 {
            let mut correspondence = owner.correspondence.clone();
            match mutation {
                0 => correspondence.borrowed_parameter_bindings = Box::default(),
                1 => correspondence.borrowed_aggregate_fields = Box::default(),
                2 => correspondence.borrowed_aggregate_calls = Box::default(),
                3 => {
                    correspondence.borrowed_parameter_bindings = Box::default();
                    correspondence.borrowed_aggregate_fields = Box::default();
                    correspondence.borrowed_aggregate_calls = Box::default();
                }
                _ => {
                    let mut calls = correspondence.borrowed_aggregate_calls.to_vec();
                    calls.pop().unwrap();
                    correspondence.borrowed_aggregate_calls = calls.into_boxed_slice();
                }
            }
            let subject = CanonicalCallSubjectV1 {
                semantic_ssa: &owner.semantic_ssa,
                executable: owner.executable(),
                correspondence: &correspondence,
            };
            let floor = budget.storage();
            assert!(
                SealedHelperMemoryV1::derive_with_source_requirements_v1(
                    subject, None, true, budget
                )
                .is_err(),
                "mutation {mutation} issued helper custody"
            );
            assert_eq!(budget.storage(), floor);
        }
        let floor = budget.storage();
        assert!(
            SealedHelperMemoryV1::derive(CanonicalCallSubjectV1::owner(owner), budget).is_err()
        );
        assert!(
            SealedHelperMemoryV1::derive_with_source_requirements_v1(
                CanonicalCallSubjectV1::owner(owner),
                Some(&owner.assert_origins),
                true,
                budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn borrowed_requirement_cannot_use_the_no_helper_shortcut() {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = super::assert_origins_v1_tests::materialize(
        super::assert_origins_v1_tests::Fixture::Literal(true),
        false,
        &mut budget,
    );
    let retained = owner.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        SealedHelperMemoryV1::derive_with_source_requirements_v1(
            CanonicalCallSubjectV1::owner(&owner),
            None,
            true,
            &mut budget
        ),
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        ))
    ));
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn borrowed_coverage_binds_identities_and_unique_source_physical_rows() {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let foreign = super::assert_origins_v1_tests::materialize(
        super::assert_origins_v1_tests::Fixture::Literal(true),
        false,
        &mut budget,
    );
    let foreign_source = foreign.semantic_ssa.identity();
    let foreign_graph = *foreign.executable.canonical().identity();
    drop(foreign);
    assert_eq!(budget.storage(), FLOOR);

    for mutation in [
        "control",
        "foreign source",
        "foreign graph",
        "duplicate physical",
        "row root",
        "row function",
        "row role",
    ] {
        let mut owner = materialize(&mut budget).expect("actual borrowed source must materialize");
        let retained = owner.retained_analysis_storage_v1();
        budget.reserve_storage(retained).unwrap();
        assert_ne!(owner.semantic_ssa.identity(), foreign_source);
        assert_ne!(owner.executable.canonical().identity(), &foreign_graph);
        let mut helpers = owner
            .correspondence
            .lowered_functions
            .iter()
            .enumerate()
            .filter(|(_, row)| row.role == SemanticKirFunctionRoleV1::InternalHelper);
        let first = helpers.next().unwrap().0;
        let second = helpers.next().unwrap().0;
        let first_function = owner.correspondence.lowered_functions[first].semantic_function;
        let second_function = owner.correspondence.lowered_functions[second].semantic_function;
        match mutation {
            "control" => {}
            "foreign source" => {
                owner
                    .helper_memory
                    .borrowed_source
                    .as_mut()
                    .unwrap()
                    .source_identity = foreign_source;
            }
            "foreign graph" => {
                owner
                    .helper_memory
                    .borrowed_source
                    .as_mut()
                    .unwrap()
                    .graph_identity = foreign_graph;
            }
            "duplicate physical" => {
                owner.correspondence.lowered_functions[second] =
                    owner.correspondence.lowered_functions[first].clone();
                owner.helper_memory.associations[second] = owner.helper_memory.associations[first];
                let seal = owner.helper_memory.borrowed_source.as_mut().unwrap();
                seal.functions[second] = seal.functions[first];
            }
            "row root" => {
                owner
                    .helper_memory
                    .borrowed_source
                    .as_mut()
                    .unwrap()
                    .functions[first]
                    .root = first_function;
            }
            "row function" => {
                owner
                    .helper_memory
                    .borrowed_source
                    .as_mut()
                    .unwrap()
                    .functions[first]
                    .function = second_function;
            }
            "row role" => {
                owner
                    .helper_memory
                    .borrowed_source
                    .as_mut()
                    .unwrap()
                    .functions[first]
                    .role = SemanticKirFunctionRoleV1::KernelEntry;
            }
            _ => unreachable!(),
        }
        let (inventory, receipt) =
            CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let mut functions =
            helper_memory_vec_v1(owner.helper_memory.functions.len(), &mut budget).unwrap();
        functions.extend(
            owner
                .helper_memory
                .functions
                .iter()
                .map(|state| match state {
                    RetainedHelperKindV1::Borrowed => RetainedHelperKindV1::Pending,
                    _ => *state,
                }),
        );
        let function_storage = functions.capacity() * std::mem::size_of::<RetainedHelperKindV1>();
        let floor = budget.storage();
        let before = budget.work();
        let result = check_retained_borrowed_coverage_v1(
            CanonicalCallSubjectV1::owner(&owner),
            &inventory,
            owner.helper_memory.borrowed_source.as_ref().unwrap(),
            &owner.helper_memory.associations,
            &mut functions,
            &mut budget,
        );
        if mutation == "control" {
            result.unwrap();
            assert_eq!(functions, owner.helper_memory.functions);
            let names = owner
                .helper_memory
                .associations
                .iter()
                .zip(owner.correspondence.lowered_functions.iter())
                .map(|(association, source)| {
                    inventory.functions()[association.physical]
                        .function
                        .id
                        .as_str()
                        .len()
                        + source.kernel_ir_function.as_str().len()
                })
                .sum::<usize>();
            assert_eq!(budget.work() - before, 11 + 15 * functions.len() + names);
            assert_eq!(budget.storage(), floor);
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionPreRankedKirErrorV1::Lowering(
                        ProductionSemanticKirErrorV1::CorrespondenceMismatch
                    ))
                ),
                "{mutation}: {result:?}"
            );
            assert!(functions.iter().all(|state| matches!(
                state,
                RetainedHelperKindV1::NotHelper | RetainedHelperKindV1::Pending
            )));
        }
        // Production derive owns cleanup around this private attachment check.
        budget
            .release_storage(budget.storage().checked_sub(floor).unwrap())
            .unwrap();
        drop(functions);
        budget.release_storage(function_storage).unwrap();
        drop(inventory);
        budget.release_storage(receipt.retained_storage()).unwrap();
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn borrowed_correspondence_reservation_precedes_canonical_admission() {
    let (required_work, required_storage) = {
        let (source, launch) = source_inputs();
        assert!(source.occurrence_storage().is_none());
        let roots = materialization_launch_roots_v1(&source, &launch).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(2).unwrap();
        let mut emission = AssertOriginEmissionV1::new(&mut budget);
        let pending = lower_pending_module_with_assert_origins_v1(
            &source,
            ProductionSemanticKirLimitsV1::default(),
            &roots,
            &mut emission,
        )
        .unwrap();
        assert!(pending.requires_source && pending.requires_borrowed);
        let bytes =
            borrowed_aggregate_correspondence_bytes_v1(&pending.correspondence, emission.budget)
                .unwrap();
        assert!(bytes > 0);
        let storage = emission.budget.storage().checked_add(bytes).unwrap();
        assert!(emission.budget.peak_storage() < storage);
        (emission.budget.work(), storage)
    };
    // Exactly the prefix's work is available: proceeding to canonical admission
    // instead of reserving the rows must fail this typed storage assertion.
    let mut work = Work::new(required_work);
    let mut budget = ArgumentBudgetV1::new(&mut work, required_storage - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(materialize(&mut budget),
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error)
            )
        )) if error.actual() == required_storage && error.limit() == required_storage - 1));
    assert_eq!(budget.work(), required_work);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn borrowed_owner_resource_boundaries_restore_the_incoming_floor() {
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let required_work = budget.work();
    let required_storage = budget.peak_storage();
    let correspondence_bytes =
        borrowed_aggregate_correspondence_bytes_v1(&owner.correspondence, &mut budget).unwrap();
    assert!(correspondence_bytes > 0);
    assert!(required_work > 0 && required_storage > FLOOR);
    drop(owner);
    for (work_limit, storage_limit, success) in [
        (required_work, required_storage, true),
        (required_work - 1, required_storage, false),
        (required_work, required_storage - 1, false),
        (
            required_work,
            required_storage - correspondence_bytes,
            false,
        ),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = materialize(&mut budget);
        assert_eq!(
            result.is_ok(),
            success,
            "work={work_limit} storage={storage_limit}: {result:?}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}
