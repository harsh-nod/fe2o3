use super::*;
use ProductionCanonicalRankedPolicyErrorV1 as PolicyError;
use ProductionCanonicalRankedSourceRequirementV1 as Need;

fn policy_source_scalar() -> ProductionPreRankedKirOwnerV1 {
    let seed = noop_semantic_owner(&["scalar_source"]);
    let source = seed.semantic();
    let root = &source.functions()[0];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let scalar = SemanticTypeIdV1::from_index(1);
    let fresh = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        provenance,
        root.abi().clone(),
        vec![
            root.locals()[0].clone(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([207; 32]),
                scalar,
                SemanticLocalRoleV1::Temporary,
                provenance,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([208; 32]),
                provenance,
                vec![SemanticStatementV1::new(
                    provenance,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], scalar)
                            .unwrap(),
                        SemanticRvalueV1::new(
                            scalar,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    scalar,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(11, 8).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                    )),
                )],
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        source.target(),
        vec![source.types()[0].clone(), scalar_type()],
        vec![],
        vec![],
        vec![],
        vec![fresh],
        source.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let owner = cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap(),
    );
    assert!(
        owner
            .executable()
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .any(|block| !block.operations.is_empty())
    );
    owner
}

fn run_source_policies<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalRankedSourcePoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> CrPolicyResultV1<T>,
) -> CrPolicyResultV1<T> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
        Ok(view.with_policy_checks_v1(budget, callback))
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result.unwrap()
}

#[test]
fn source_policy_genuine_noop_scalar_and_multiroot_keep_exact_subject_and_all_nine_reports() {
    for owner in [
        cr_noop(&["noop"]),
        policy_source_scalar(),
        cr_noop(&["zeta", "alpha"]),
    ] {
        let before = owner.executable().canonical().canonical_bytes().to_vec();
        run_source_policies(&owner, |view, budget| {
            let source = view.metadata(budget)?;
            assert!(std::ptr::eq(
                source.inventory(budget)?.owner(),
                owner.executable()
            ));
            let policies = view.policies(budget)?;
            assert!(std::ptr::eq(policies.owner(budget)?, owner.executable()));
            assert_eq!(
                policies.function_count(budget)?,
                owner.executable().module().functions.len()
            );
            for index in 0..policies.function_count(budget)? {
                let report = policies.report(index, budget)?;
                assert_eq!(report.pass_order().len(), 9);
                assert!(report.is_clean());
                assert_eq!(policies.history(index, budget)?.function(), index);
            }
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!view.ranked_verification_is_complete());
            assert!(!view.grants_artifact_or_launch_authority());
            assert!(!policies.ranked_verification_is_complete());
            assert!(!policies.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
        assert_eq!(owner.executable().canonical().canonical_bytes(), before);
    }
}

#[test]
fn source_policy_unitlocal_rejects_original_memory_without_using_erasure() {
    let erased = erased();
    let owner = erased.original_source();
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    let error = run_source_policies(owner, |_, _| -> CrPolicyResultV1<()> {
        panic!("unsupported local memory must not enter fixed reports callback")
    })
    .unwrap_err();
    assert!(matches!(
        error,
        PolicyError::Unsupported {
            requirement: Need::LocalMemory,
            ordinal: 0
        }
    ));
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn source_policy_real_catalog_definitions_and_emitted_bindings_cannot_become_absence() {
    let definitions = nonempty_source();
    let definitions = cr_owner_from_ssa(definitions.semantic_ssa);
    let binding = cr_reachable_pipeline_owner();
    for (owner, bindings) in [(&definitions, 0), (&binding, 1)] {
        let (observed, _, _, _) = cr_run_owner(owner, 1 << 40, S, |source, budget| {
            assert_eq!(source.catalog(budget)?.definitions().len(), 1);
            assert_eq!(source.catalog(budget)?.bindings().len(), bindings);
            Ok(())
        });
        observed.unwrap();
        let before = owner.executable().canonical().canonical_bytes().to_vec();
        let error = run_source_policies(owner, |_, _| -> CrPolicyResultV1<()> {
            panic!("real pipeline requirements cannot become empty reports")
        })
        .unwrap_err();
        // Reachable pipeline operations may be refused by their real effect
        // before the catalog reader; either way the source remains unchanged.
        assert!(matches!(
            error,
            PolicyError::Unsupported {
                requirement: Need::Catalog | Need::Effect,
                ..
            }
        ));
        assert_eq!(owner.executable().canonical().canonical_bytes(), before);
    }
}

#[test]
fn source_policy_emitted_and_zero_operation_assertions_require_their_real_reader() {
    use super::super::super::super::assert_origins_v1_tests::{
        Fixture, materialize as assertion_owner,
    };
    for (kind, shared) in [
        (Fixture::Literal(true), false),
        (Fixture::Literal(false), true),
        (Fixture::ElidedBounds, false),
    ] {
        let mut work = Work::new(1 << 40);
        let mut budget = Budget::new(&mut work, S);
        let owner = assertion_owner(kind, shared, &mut budget);
        let (observed, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
            assert!(!source.assertions(budget)?.is_empty());
            if matches!(kind, Fixture::ElidedBounds) {
                assert!(source.assertions(budget)?.iter().any(|row| matches!(
                    row.binding().outcome(),
                    SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
                )));
            }
            Ok(())
        });
        observed.unwrap();
        let error = run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
            panic!("assertion reader missing")
        })
        .unwrap_err();
        assert!(matches!(
            error,
            PolicyError::Unsupported {
                requirement: Need::Assertion,
                ..
            }
        ));
    }
}

#[test]
fn source_policy_same_shape_foreign_checked_graph_never_reuses_a_source_roster() {
    let first = cr_noop(&["first"]);
    let second = cr_noop(&["second"]);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = first.unit_local_source_storage_floor_v1().unwrap()
        + second.unit_local_source_storage_floor_v1().unwrap()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    first
        .with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
            second.with_checked_canonical_ranked_source_v1(budget, |foreign, budget| {
                let mut mixed = ProductionCanonicalRankedSourceViewV1 {
                    source,
                    checked: foreign.checked,
                };
                let result = mixed.with_policy_checks_v1(budget, |_, _| -> CrPolicyResultV1<()> {
                    panic!("foreign same-shaped graph cannot borrow source authority")
                });
                assert!(matches!(
                    result,
                    Err(PolicyError::Unsupported {
                        requirement: Need::GraphIdentity,
                        ordinal: 0
                    })
                ));
                Ok(())
            })
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_policy_callback_query_poison_error_and_panic_preserve_source_owner() {
    let owner = cr_noop(&["source_errors"]);
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    let error = run_source_policies(&owner, |view, budget| {
        let _ = view.policies(budget)?.report(1, budget);
        Ok(())
    })
    .unwrap_err();
    assert!(
        matches!(error, PolicyError::Policy(ref error) if matches!(error.failure(),
        fe2o3_pliron::CanonicalRankedPolicyFailureV1::InvalidQuery { function: 1 }))
    );
    let error = run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
        Err(PolicyError::Unsupported {
            requirement: Need::Callable,
            ordinal: 7,
        })
    })
    .unwrap_err();
    assert!(matches!(
        error,
        PolicyError::Unsupported {
            requirement: Need::Callable,
            ordinal: 7
        }
    ));
    let error = run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
        panic!("real callback unwind")
    })
    .unwrap_err();
    assert!(
        matches!(error, PolicyError::Policy(ref error) if matches!(error.failure(),
        fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked))
    );
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn source_policy_callback_scratch_must_restore_its_exact_paid_floor() {
    let owner = cr_noop(&["callback_scratch"]);
    for excess in [false, true] {
        let error = run_source_policies(&owner, |_, budget| {
            if excess {
                budget.reserve_storage(1)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(
            matches!(error, PolicyError::Policy(ref error) if matches!(error.failure(),
            fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                ArgumentResourceV1::Accounting
            ))),
            "{error:?}"
        );
    }
    run_source_policies(&owner, |view, budget| {
        let floor = budget.storage();
        budget.reserve_storage(17)?;
        assert_eq!(view.policies(budget)?.function_count(budget)?, 1);
        assert_eq!(view.metadata(budget)?.function_count(budget)?, 1);
        budget.release_storage(17)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_policy_ignored_source_query_cannot_escape_through_clean_reports() {
    let owner = cr_noop(&["source_query_poison"]);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let result = owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
        Ok(view.with_policy_checks_v1(budget, |view, budget| {
            let _ = view.metadata(budget)?.function(usize::MAX, budget);
            Ok(())
        }))
    });
    assert!(matches!(result, Err(CrError::Invalid("function ordinal"))));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_policy_scalar_profile_keeps_external_refinement_pending() {
    let owner = cr_noop(&["typed_profile"]);
    let (result, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        assert!(source.arguments(budget)?.is_empty());
        // Direct profile success is only syntax coverage; it still has every
        // safety/refinement obligation pending and grants no completed result.
        cr_policy_source_profile_v1(source, budget).unwrap();
        assert!(source.external_refinement_is_pending());
        Ok(())
    });
    result.unwrap();
}

#[test]
fn source_policy_real_helper_call_transport_requires_its_source_reader() {
    let owner = cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(
            helper_closure_semantic_owner(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    );
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    assert!(
        owner
            .correspondence
            .call_returns
            .iter()
            .any(|row| { matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. }) })
    );
    assert!(matches!(
        run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
            panic!("helper transport cannot be replaced with an empty policy report")
        }),
        Err(PolicyError::Unsupported {
            requirement: Need::CallTransport | Need::Effect,
            ..
        })
    ));
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn source_policy_unused_shared_reference_argument_keeps_its_real_ownership() {
    use super::super::super::super::assert_origins_v1_tests::{
        Fixture, materialize as assertion_owner,
    };
    let mut work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let seed = assertion_owner(Fixture::ElidedBounds, false, &mut budget);
    let semantic = seed.semantic_ssa.source_semantic();
    let root = &semantic.functions()[0];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    // A new genuine no-op source request reuses the established reference ABI.
    // Its reference argument is intentionally unused, but its ownership and
    // complete source type table remain present in the new source owner.
    let function = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        provenance,
        root.abi().clone(),
        root.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                root.blocks()[0].identity(),
                provenance,
                vec![],
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let owner = cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap(),
    );
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    let (observed, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        assert!(source.assertions(budget)?.is_empty());
        assert!(source.arguments(budget)?.iter().any(|argument| {
            argument.source_ownership() == SemanticSourceArgumentOwnershipV1::SharedBorrow
        }));
        Ok(())
    });
    observed.unwrap();
    assert!(matches!(
        run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
            panic!("an unused shared reference cannot mean absent ownership")
        }),
        Err(PolicyError::Unsupported {
            requirement: Need::Type | Need::ArgumentOwnership,
            ..
        })
    ));
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn source_policy_unused_raw_pointer_type_is_not_scalar_absence() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
        SemanticPointerTypeV1,
    };
    let seed = noop_semantic_owner(&["unused_pointer_source"]);
    let semantic = seed.semantic();
    let root = &semantic.functions()[0];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let pointer = SemanticTypeIdV1::from_index(semantic.types().len().try_into().unwrap());
    let mut types = semantic.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(1, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                1,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = root.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([213; 32]),
        pointer,
        SemanticLocalRoleV1::Temporary,
        provenance,
    ));
    let function = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        provenance,
        root.abi().clone(),
        locals,
        root.entry(),
        root.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let owner = cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap(),
    );
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    let (observed, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        assert!(source.arguments(budget)?.is_empty());
        assert!(
            source
                .types(budget)?
                .iter()
                .any(|ty| matches!(ty.shape(), SemanticTypeShapeV1::Pointer(_)))
        );
        Ok(())
    });
    observed.unwrap();
    assert!(matches!(
        run_source_policies(&owner, |_, _| -> CrPolicyResultV1<()> {
            panic!("raw pointer carrier requires a source reader")
        }),
        Err(PolicyError::Unsupported {
            requirement: Need::Type,
            ..
        })
    ));
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn source_policy_foreign_and_moved_report_queries_do_not_debit_substitutes() {
    let owner = cr_noop(&["policy_ledger_slots"]);
    for moved in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut foreign_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(SIBLING).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(source.with_policy_checks_v1(budget, |view, budget| {
                    let policies = view.policies(budget)?;
                    if moved {
                        std::mem::swap(budget, &mut foreign);
                        let before = foreign.work();
                        assert!(matches!(
                            policies.function_count(&mut foreign),
                            Err(fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(foreign.work(), before);
                        std::mem::swap(budget, &mut foreign);
                    } else {
                        assert!(matches!(
                            policies.function_count(&mut foreign),
                            Err(fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                    }
                    Ok(())
                }))
            });
        assert!(matches!(
            result.unwrap(),
            Err(PolicyError::Policy(ref error))
                if matches!(error.failure(),
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                        ArgumentResourceV1::Accounting
                    ))
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!((foreign.storage(), foreign.work()), (SIBLING, 0));
    }
}

#[test]
fn source_policy_rejected_drop_owns_the_displaced_paid_budget() {
    use std::cell::{Cell, RefCell};

    struct PaidDrop<'a, 'w> {
        original: Option<Budget<'w>>,
        saved: &'a RefCell<Option<Budget<'w>>>,
        observed: &'a Cell<usize>,
        drops: &'a Cell<usize>,
        panic_on_drop: bool,
    }
    impl Drop for PaidDrop<'_, '_> {
        fn drop(&mut self) {
            let original = self.original.take().unwrap();
            self.observed.set(original.storage());
            self.drops.set(self.drops.get() + 1);
            *self.saved.borrow_mut() = Some(original);
            if self.panic_on_drop {
                panic!("rejected policy callback result destructor");
            }
        }
    }

    let owner = cr_noop(&["policy_paid_drop"]);
    for panic_on_drop in [false, true] {
        let mut work = Work::new(1 << 48);
        let mut foreign_work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        foreign.reserve_storage(SIBLING).unwrap();
        let saved = RefCell::new(None);
        let observed = Cell::new(0);
        let drops = Cell::new(0);
        let callback_floor = Cell::new(0);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap()
            + std::mem::size_of::<PaidDrop<'_, '_>>()
            + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result =
            owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(source.with_policy_checks_v1(budget, |view, budget| {
                    assert_eq!(view.policies(budget)?.function_count(budget)?, 1);
                    callback_floor.set(budget.storage());
                    let original = std::mem::replace(budget, foreign);
                    Ok(PaidDrop {
                        original: Some(original),
                        saved: &saved,
                        observed: &observed,
                        drops: &drops,
                        panic_on_drop,
                    })
                }))
            });
        assert!(matches!(
            result,
            Err(CrError::Resource(ArgumentResourceV1::Accounting))
                | Err(CrError::Analysis(
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Resource(
                        ArgumentResourceV1::Accounting
                    )
                ))
        ));
        assert_eq!(drops.get(), 1);
        assert_eq!(observed.get(), callback_floor.get());
        assert!(observed.get() > floor);
        assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
        let original = saved.borrow_mut().take().unwrap();
        assert!(original.work_ledger_identity_v1() == ledger);
        assert_eq!(original.storage(), callback_floor.get());
        let _replacement = std::mem::replace(&mut budget, original);
        budget
            .release_storage(callback_floor.get() - floor)
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn source_policy_real_panic_payload_drops_once_before_return() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let owner = cr_noop(&["policy_panic_payload"]);
    let drops = Arc::new(AtomicUsize::new(0));
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap()
        + std::mem::size_of::<Payload>()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
        Ok(
            source.with_policy_checks_v1(budget, |_, _| -> CrPolicyResultV1<()> {
                std::panic::panic_any(Payload(Arc::clone(&drops)));
            }),
        )
    });
    assert!(matches!(
        result.unwrap(),
        Err(PolicyError::Policy(ref error))
            if matches!(error.failure(),
                fe2o3_pliron::CanonicalRankedPolicyFailureV1::Panicked)
    ));
    // This observes real payload destruction, not the Budget during that Drop.
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn source_policy_final_paid_query_has_literal_one_zero_work_boundary() {
    const WORK_LIMIT: usize = 1 << 48;
    let owner = cr_noop(&["policy_final_query"]);
    for remaining in [1, 0] {
        let mut work = Work::new(WORK_LIMIT);
        let mut budget = Budget::new(&mut work, S);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let mut entered = false;
        let result =
            owner.with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                Ok(source.with_policy_checks_v1(budget, |_, budget| {
                    entered = true;
                    // The only remaining metered query is the literal one-unit
                    // policy postflight. No measured successful run supplies this cut.
                    let target = WORK_LIMIT - remaining;
                    budget.charge_work(target.checked_sub(budget.work()).unwrap())?;
                    assert_eq!(budget.work(), target);
                    Ok(())
                }))
            });
        assert!(entered);
        let result = result.unwrap();
        if remaining == 1 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(PolicyError::Policy(ref error))
                    if matches!(error.failure(),
                        fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(
                            ArgumentResourceV1::Work(error)
                        ) if error.actual() == WORK_LIMIT + 1
                            && error.limit() == WORK_LIMIT)
            ));
        }
        assert_eq!(budget.work(), WORK_LIMIT);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn source_policy_header_increment_matches_the_literal_owned_types() {
    use std::cell::Cell;

    let owner = cr_noop(&["policy_header_increment"]);
    let direct_floor = Cell::new(0);
    let headers = std::mem::size_of::<ProductionCanonicalRankedSourcePoliciesV1<'_, '_, '_>>()
        + std::mem::size_of::<std::thread::Result<CrPolicyResultV1<()>>>()
        + std::mem::size_of::<CrPolicyResultV1<()>>();
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
            let checked_floor = budget.storage();
            fe2o3_pliron::with_canonical_ranked_policy_checks_v1(
                source.checked,
                budget,
                |_, budget| -> Result<
                    CrPolicyResultV1<()>,
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1,
                > {
                    direct_floor.set(budget.storage());
                    Ok(Ok(()))
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(budget.storage(), checked_floor);
            source
                .with_policy_checks_v1(budget, |_, budget| {
                    assert_eq!(
                        budget.storage(),
                        direct_floor.get().checked_add(headers).unwrap()
                    );
                    Ok(())
                })
                .unwrap();
            assert_eq!(budget.storage(), checked_floor);
            Ok(())
        })
        .unwrap();
    assert!(direct_floor.get() > floor);
    assert_eq!(budget.storage(), floor);
    // A cross-route header census is not an interior reservation-denial oracle.
}
