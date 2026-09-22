use super::*;
use ProductionCanonicalRankedSourceErrorV1 as CrError;

fn cr_owner_from_ssa(ssa: ProductionSemanticSsaOwnerV1) -> ProductionPreRankedKirOwnerV1 {
    let semantic = ssa.source_semantic();
    let inputs = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap();
    let mut work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}
fn cr_noop(names: &[&str]) -> ProductionPreRankedKirOwnerV1 {
    cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(
            noop_semantic_owner(names),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap(),
    )
}
fn cr_run_owner<T>(
    owner: &ProductionPreRankedKirOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    run: impl for<'a> FnOnce(&ProductionCanonicalRankedMetadataV1<'a>, &mut Budget<'_>) -> CrResultV1<T>,
) -> (CrResultV1<T>, usize, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let result = owner.with_canonical_ranked_metadata_v1(&mut budget, run);
    assert_eq!(budget.storage(), floor);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}
#[test]
fn canonical_ranked_owner_noop_keeps_source_order_and_one_actual_graph() {
    let owner = cr_noop(&["zeta", "alpha"]);
    let original = owner.executable().canonical().canonical_bytes().to_vec();
    let (result, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        let inventory = source.inventory(budget)?;
        assert!(std::ptr::eq(inventory.owner(), owner.executable()));
        let launches = source.launches(budget)?;
        assert_eq!(launches.len(), 2);
        for (index, launch) in launches.iter().enumerate() {
            assert_eq!(launch.kernel().ordinal as usize, index);
            assert!(std::ptr::eq(
                launch.source(),
                &owner.source_launch.roots()[index]
            ));
        }
        assert_eq!(source.arguments(budget)?.len(), 0);
        assert_eq!(source.effects(budget)?.len(), 0);
        assert_eq!(source.catalog(budget)?.definitions().len(), 0);
        assert!(source.external_refinement_is_pending());
        assert!(!source.ranked_verification_is_complete());
        Ok(())
    });
    result.unwrap();
    assert_eq!(owner.executable().canonical().canonical_bytes(), original);
}
#[test]
fn canonical_ranked_owner_retains_real_unitlocal_original_memory_and_fences() {
    use fe2o3_kernel_ir::{AddressSpace, KirLocalMemoryEffectRefV1 as Effect, OperationKind};

    let erased = erased();
    let owner = erased.original_source();
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    let original = owner.executable().canonical().canonical_bytes().to_vec();
    let mut work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            let source = view.metadata(budget)?;
            assert!(std::ptr::eq(
                source.inventory(budget)?.owner(),
                owner.executable()
            ));
            let [allocation, store, load] = source.effects(budget)? else {
                panic!("the retained helper must allocate, store, then load its private cell");
            };
            assert!(matches!(
                (&allocation.operation.kind, allocation.effect),
                (
                    OperationKind::Alloca { .. },
                    Effect::Allocate(AddressSpace::Private)
                )
            ));
            assert!(matches!(
                (&store.operation.kind, store.effect),
                (
                    OperationKind::Store { .. },
                    Effect::Write(AddressSpace::Private)
                )
            ));
            assert!(matches!(
                (&load.operation.kind, load.effect),
                (
                    OperationKind::Load { .. },
                    Effect::Read(AddressSpace::Private)
                )
            ));
            assert!(source.spans(budget)?.iter().any(|row| matches!(row.site(),
            ProductionCanonicalRankedSourceSiteV1::Synthetic(span)
                if span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)));
            assert!(source.spans(budget)?.iter().any(|row| matches!(row.site(),
            ProductionCanonicalRankedSourceSiteV1::Statement { source, .. }
                if matches!(source.kind(), SemanticStatementKindV1::Store(_)))));
            let mut helpers = 0;
            for association in 0..source.function_count(budget)? {
                if source.function(association, budget)?.source().role
                    != SemanticKirFunctionRoleV1::InternalHelper
                {
                    continue;
                }
                let frame = source.helper_frame(association, budget)?.unwrap();
                assert_eq!(frame.allocations().len(), 1);
                assert_eq!(frame.accesses().len(), 2);
                assert!(!frame.control().is_empty());
                assert!(std::ptr::eq(
                    frame.source(),
                    &owner.correspondence.lowered_functions[association]
                ));
                helpers += 1;
            }
            assert_eq!(helpers, 1);
            assert!(!view.ranked_verification_is_complete());
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(owner.executable().canonical().canonical_bytes(), original);
    assert!(matches!(
        owner.require_legacy_helper_policy_v1("regression"),
        Err(ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable { .. })
    ));
}
#[test]
fn canonical_ranked_owner_accepts_genuine_nonempty_source_catalog_without_fake_bindings() {
    // Constructed admitted semantic source: unreachable calls retain a genuine
    // definition while SSA prunes their allocation sites. Not ordinary Rust or
    // native/tutorial qualification, and zero bindings are asserted explicitly.
    let source = nonempty_source();
    let owner = cr_owner_from_ssa(source.semantic_ssa);
    let (result, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        let catalog = source.catalog(budget)?;
        assert_eq!(catalog.definitions().len(), 1);
        assert_eq!(catalog.bindings().len(), 0);
        let row = catalog.definitions()[0];
        assert_eq!(row.semantic_pipeline_type, 4);
        assert_eq!(row.semantic_payload_type, 1);
        assert_eq!(row.packed_bits, 64);
        assert_eq!(row.source_size_bytes, 8);
        assert_eq!(row.source_alignment_bytes, 8);
        assert!(!source.callables(budget)?.is_empty());
        assert!(!catalog.grants_authority());
        Ok(())
    });
    result.unwrap();
}
#[test]
fn canonical_ranked_owner_rejects_missing_and_changed_genuine_catalog_definitions() {
    let source = nonempty_source();
    let owner = cr_owner_from_ssa(source.semantic_ssa);
    let (result, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        for changed in [false, true] {
            cr_protected_v1(budget, |budget| {
                budget.reserve_storage(size_of::<
                    Vec<fe2o3_kernel_ir::KernelIrPipelineContractDefinitionV1>,
                >())?;
                let mut definitions =
                    cr_vec_v1(source.contracts.catalog.definitions().len(), budget)?;
                for row in source.contracts.catalog.definitions() {
                    cr_push_v1(&mut definitions, *row, budget)?;
                }
                if changed {
                    definitions[0].elements += 1;
                } else {
                    definitions.clear();
                }
                let (catalog, storage) = fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                    *owner.semantic_ssa.source_semantic().semantic_sha256().as_bytes(), &definitions, &[], budget,
                ).map_err(ProductionSourceOutputCatalogErrorV1::Codec)?;
                budget.reserve_storage(storage.retained_storage())?;
                assert!(
                    cr_check_catalog_source_v1(
                        &owner,
                        source.inventory,
                        source.source,
                        &catalog,
                        budget
                    )
                    .is_err()
                );
                Ok(())
            })?;
        }
        Ok(())
    });
    result.unwrap();
}
#[test]
fn canonical_ranked_owner_exact_query_work_and_storage_boundary_fail_closed() {
    let owner = cr_noop(&["query_floor"]);
    let query = |view: &ProductionCanonicalRankedMetadataV1<'_>, budget: &mut Budget<'_>| {
        assert_eq!(view.function_count(budget)?, 1);
        Ok(())
    };
    let (success, w, p, _) = cr_run_owner(&owner, 1 << 40, S, query);
    success.unwrap();
    let (exact, exact_w, exact_p, failed) = cr_run_owner(&owner, w, p, query);
    exact.unwrap();
    assert_eq!((exact_w, exact_p, failed), (w, p, None));
    let mut entered = false;
    let (short, prefix, _, _) = cr_run_owner(&owner, w - 1, p, |view, budget| {
        entered = true;
        let before = budget.work();
        assert_eq!(before, w - 1);
        let error = view.function_count(budget).unwrap_err();
        assert!(matches!(error, CrError::Resource(Resource::Work(error))
            if error.actual() == w && error.limit() == w - 1));
        // An ignored query error must still reject the outer result.
        Ok(())
    });
    assert!(entered);
    assert_eq!(prefix, w - 1);
    assert!(matches!(short, Err(CrError::Resource(Resource::Work(_)))));
    let (short, _, _, failed) = cr_run_owner(&owner, w, p - 1, query);
    assert!(short.is_err());
    assert!(failed.is_some_and(|actual| actual > p - 1));
    // P is measured. This is not an independent exact oracle for every
    // construction-phase prefix; that broader qualification remains separate.
}
#[test]
fn canonical_ranked_owner_invalid_queries_are_sticky_and_keep_first_error() {
    let owner = cr_noop(&["invalid_query"]);
    let (result, _, _, _) = cr_run_owner(&owner, 1 << 40, S, |source, budget| {
        assert!(matches!(
            source.function(usize::MAX, budget),
            Err(CrError::Invalid("function ordinal"))
        ));
        let before = budget.work();
        assert!(matches!(
            source.argument_path(usize::MAX, true, budget),
            Err(CrError::Invalid("function ordinal"))
        ));
        assert_eq!(budget.work(), before);
        Ok(())
    });
    assert!(matches!(result, Err(CrError::Invalid("function ordinal"))));
}
#[test]
fn canonical_ranked_owner_callback_error_panic_and_late_undercut_restore_original_floor() {
    let owner = cr_noop(&["callback_exit"]);
    for mode in [0, 1, 2, 3] {
        let (result, _, _, _) =
            cr_run_owner(&owner, 1 << 40, S, |source, budget| -> CrResultV1<()> {
                match mode {
                    0 => Err(CrError::Invalid("caller refusal")),
                    1 => panic!("caller panic"),
                    2 => {
                        budget.release_storage(1)?;
                        assert!(matches!(
                            source.function_count(budget),
                            Err(CrError::Resource(Resource::Accounting))
                        ));
                        budget.reserve_storage(1)?;
                        Ok(())
                    }
                    3 => {
                        // Even without a query, postflight must reject released backing.
                        budget.release_storage(1)?;
                        Ok(())
                    }
                    _ => unreachable!(),
                }
            });
        match mode {
            0 => assert!(matches!(result, Err(CrError::Invalid("caller refusal")))),
            1 => assert!(matches!(result, Err(CrError::Panicked))),
            2 | 3 => assert!(matches!(
                result,
                Err(CrError::Resource(Resource::Accounting))
            )),
            _ => unreachable!(),
        }
    }
}
#[test]
fn canonical_ranked_owner_foreign_and_moved_slots_do_not_debit_substitutes() {
    let owner = cr_noop(&["ledger_slots"]);
    for moved in [false, true] {
        let mut work = Work::new(1 << 40);
        let mut foreign_work = Work::new(1 << 40);
        let mut budget = Budget::new(&mut work, S);
        let mut foreign = Budget::new(&mut foreign_work, S);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        foreign.reserve_storage(SIBLING).unwrap();
        let result = owner.with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
            if moved {
                std::mem::swap(budget, &mut foreign);
                let before = foreign.work();
                assert!(source.function_count(&mut foreign).is_err());
                assert_eq!(foreign.work(), before);
                std::mem::swap(budget, &mut foreign);
            } else {
                let before = foreign.work();
                assert!(source.function_count(&mut foreign).is_err());
                assert_eq!(foreign.work(), before);
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(CrError::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(foreign.storage(), SIBLING);
        assert_eq!(foreign.work(), 0);
    }
}
#[test]
fn canonical_ranked_owner_rejected_value_drops_with_displaced_original_budget_paid() {
    use std::cell::RefCell;
    struct Observed<'a, 'w> {
        original: Option<Budget<'w>>,
        saved: &'a RefCell<Option<Budget<'w>>>,
        observed: &'a Cell<usize>,
    }
    impl Drop for Observed<'_, '_> {
        fn drop(&mut self) {
            let original = self.original.take().unwrap();
            self.observed.set(original.storage());
            *self.saved.borrow_mut() = Some(original);
        }
    }
    let owner = cr_noop(&["rejected_drop"]);
    let mut work = Work::new(1 << 40);
    let mut foreign_work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(SIBLING).unwrap();
    let saved = RefCell::new(None);
    let observed = Cell::new(0);
    let callback_floor = Cell::new(0);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap()
        + size_of::<Observed<'_, '_>>()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let result = owner.with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
        assert_eq!(source.function_count(budget)?, 1);
        callback_floor.set(budget.storage());
        let original = std::mem::replace(budget, foreign);
        Ok(Observed {
            original: Some(original),
            saved: &saved,
            observed: &observed,
        })
    });
    assert!(matches!(
        result,
        Err(CrError::Resource(Resource::Accounting)) | Err(CrError::Analysis(_))
    ));
    assert_eq!(observed.get(), callback_floor.get());
    assert!(observed.get() > floor);
    assert_eq!(budget.storage(), SIBLING);
    assert_eq!(budget.work(), 0);
    let original = saved.borrow_mut().take().unwrap();
    let replacement = std::mem::replace(&mut budget, original);
    drop(replacement);
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_ranked_owner_rejected_destructor_panic_keeps_original_payment() {
    use std::cell::RefCell;
    struct Panicking<'a, 'w> {
        original: Option<Budget<'w>>,
        saved: &'a RefCell<Option<Budget<'w>>>,
        observed: &'a Cell<usize>,
    }
    impl Drop for Panicking<'_, '_> {
        fn drop(&mut self) {
            let original = self.original.take().unwrap();
            self.observed.set(original.storage());
            *self.saved.borrow_mut() = Some(original);
            panic!("rejected owned result destructor");
        }
    }
    let owner = cr_noop(&["panicking_result"]);
    let mut work = Work::new(1 << 40);
    let mut foreign_work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(SIBLING).unwrap();
    let saved = RefCell::new(None);
    let observed = Cell::new(0);
    let callback_floor = Cell::new(0);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap()
        + size_of::<Panicking<'_, '_>>()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let result = owner.with_canonical_ranked_metadata_v1(&mut budget, |_, budget| {
        callback_floor.set(budget.storage());
        let original = std::mem::replace(budget, foreign);
        Ok(Panicking {
            original: Some(original),
            saved: &saved,
            observed: &observed,
        })
    });
    assert!(matches!(
        result,
        Err(CrError::Resource(Resource::Accounting)) | Err(CrError::Analysis(_))
    ));
    assert_eq!(observed.get(), callback_floor.get());
    assert!(observed.get() > floor);
    assert_eq!((budget.storage(), budget.work()), (SIBLING, 0));
    let original = saved.borrow_mut().take().unwrap();
    let replacement = std::mem::replace(&mut budget, original);
    drop(replacement);
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_ranked_owner_preserves_emitted_elided_and_shared_assertion_edges() {
    use super::super::super::assert_origins_v1_tests::{Fixture, materialize as assertion_owner};
    for (kind, shared) in [
        (Fixture::Literal(true), false),
        (Fixture::Literal(false), false),
        (Fixture::Literal(false), true),
        (Fixture::ElidedBounds, false),
        (Fixture::Unreachable, false),
    ] {
        let mut work = Work::new(1 << 40);
        let mut budget = Budget::new(&mut work, S);
        let owner = assertion_owner(kind, shared, &mut budget);
        let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
        budget.reserve_storage(floor).unwrap();
        owner.with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            let source = view.metadata(budget)?;
            let assertions = source.assertions(budget)?;
            assert_eq!(assertions.len(), owner.assert_origins().source_site_count());
            if matches!(kind, Fixture::Unreachable) { assert!(assertions.is_empty()); return Ok(()); }
            assert!(!assertions.is_empty());
            for row in assertions {
                let span = &source.source.spans[row.span()];
                let ProductionCanonicalRankedSourceSiteV1::Terminator { source: term, .. } = span.site() else { panic!("assertion needs a source terminator"); };
                let SemanticTerminatorKindV1::Assert { expected, target, .. } = term.kind() else { panic!("genuine source assertion"); };
                assert_eq!(row.binding().expected(), *expected);
                assert_eq!(row.binding().semantic_success(), target.target());
                match (kind, row.binding().outcome()) {
                    (Fixture::ElidedBounds, SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { success_edge }) => {
                        assert_eq!(success_edge.source, span.block());
                        assert_eq!(success_edge.successor, 0);
                    }
                    (Fixture::Literal(polarity), SemanticKirAssertConditionOutcomeV1::Emitted {
                        success_edge, failure_edge, condition_use, ..
                    }) => {
                        assert_eq!(success_edge.source, span.block());
                        assert_eq!(failure_edge.source, span.block());
                        assert_eq!(success_edge.successor, u32::from(!polarity));
                        assert_eq!(failure_edge.successor, u32::from(polarity));
                        assert!(matches!(condition_use, fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::TerminatorOperand { block, operand: 0 } if block == span.block()));
                    }
                    _ => panic!("source emission/elision role changed"),
                }
            }
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

// Reachable constructed semantic source, not a rustc callback or pipeline-safety proof.
fn cr_reachable_pipeline_owner() -> ProductionPreRankedKirOwnerV1 {
    let seed = nonempty_source();
    let semantic = seed.semantic_ssa.source_semantic();
    let root = &semantic.functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let pipeline = SemanticTypeIdV1::from_index(4);
    let pipeline_ref = SemanticTypeIdV1::from_index(5);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let scalar_constant = |bits| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 8).unwrap()),
        ))
    };
    let call = |callee, arguments, destination, ty, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance,
            statements,
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    };
    let read_abi = semantic.callables()[2].binding().unwrap().abi();
    let write_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([163; 32]),
        semantic.target_layout_identity(),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![
            read_abi.arguments()[0].value().clone(),
            read_abi.arguments()[1].value().clone(),
            read_abi.arguments()[2].value().clone(),
            read_abi.return_value().clone(),
        ],
        root.abi().return_value().clone(),
    )
    .unwrap();
    let mut callables = semantic.callables().to_vec();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([163; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([163; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([163; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([163; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([163; 32]),
            provenance,
            write_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
            pipeline,
            element: scalar,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([163; 32]),
    });
    // Reuse genuine scope/create/borrow input syntax, but re-admit the changed
    // complete request before SSA. No correspondence or catalog row is injected.
    let root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        vec![
            block(
                60,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            root.blocks()[1].clone(),
            root.blocks()[2].clone(),
            block(
                63,
                root.blocks()[3].statements().to_vec(),
                call(
                    4,
                    vec![
                        SemanticOperandV1::Move(place(4, pipeline_ref)),
                        scalar_constant(0),
                        scalar_constant(0),
                        scalar_constant(7),
                    ],
                    0,
                    unit,
                    4,
                ),
            ),
            block(
                64,
                root.blocks()[3].statements().to_vec(),
                call(
                    2,
                    vec![
                        SemanticOperandV1::Move(place(4, pipeline_ref)),
                        scalar_constant(0),
                        scalar_constant(0),
                    ],
                    5,
                    scalar,
                    5,
                ),
            ),
            block(65, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        vec![root],
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(admitted.semantic_sha256(), semantic.semantic_sha256());
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    assert_eq!(ssa.summary().input_blocks(), 6);
    assert_eq!(ssa.summary().reachable_blocks(), 6);
    assert_eq!(ssa.summary().pruned_blocks(), 0);
    cr_owner_from_ssa(ssa)
}

#[test]
fn canonical_ranked_owner_checks_reachable_pipeline_create_write_read_binding() {
    use fe2o3_kernel_ir::{
        KernelIrPipelineContractDefinitionV1 as Definition,
        KernelIrPipelineStorageBindingV1 as Binding, WorkgroupMemoryExtent,
    };
    let owner = cr_reachable_pipeline_owner();
    let mut work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut entered = false;
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            entered = true;
            let source = view.metadata(budget)?;
            let inventory = source.inventory(budget)?;
            let catalog = source.catalog(budget)?;
            assert!(std::ptr::eq(inventory.owner(), owner.executable()));
            assert_eq!(
                catalog.semantic_source(),
                owner
                    .semantic_ssa
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes()
            );
            assert_eq!(
                catalog.definitions(),
                &[Definition {
                    key: 0,
                    semantic_pipeline_type: 4,
                    semantic_payload_type: 1,
                    buffers: 2,
                    elements: 64,
                    prefetch_distance: 1,
                    packed_bits: 64,
                    source_size_bytes: 8,
                    source_alignment_bytes: 8,
                }]
            );
            // Expected binding is derived from the actual emitted allocation,
            // independently of catalog construction and its source checker.
            let mut allocations = inventory
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::WorkgroupMemory(_)));
            let allocation = allocations.next().unwrap();
            assert!(allocations.next().is_none());
            let OperationKind::WorkgroupMemory(memory) = &allocation.operation.kind else {
                unreachable!()
            };
            assert_eq!(memory.element, Type::Scalar(ScalarType::U64));
            assert_eq!(memory.extent, WorkgroupMemoryExtent::Static(2 * 64));
            assert_eq!(memory.alignment, 8);
            let [storage] = allocation.operation.results.as_slice() else {
                panic!("one physical pipeline storage result")
            };
            assert_eq!(
                storage.ty,
                Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite
                )
            );
            assert_eq!(
                catalog.bindings(),
                &[Binding {
                    function: allocation.coordinate.block.function.0,
                    storage: storage.id.0,
                    key: 0,
                    block: allocation.coordinate.block.block,
                    operation: allocation.coordinate.operation,
                }]
            );
            let mut calls = [0_usize; 3];
            for row in source.spans(budget)? {
                let ProductionCanonicalRankedSourceSiteV1::Terminator { span, source } = row.site()
                else {
                    continue;
                };
                let SemanticTerminatorKindV1::Call(call) = source.kind() else {
                    continue;
                };
                let (slot, block, callee) = match span.semantic_block.index() {
                    2 => (0, 2, 1),
                    3 => (1, 3, 4),
                    4 => (2, 4, 2),
                    _ => continue,
                };
                assert_eq!(span.semantic_function.index(), 0);
                assert_eq!(span.semantic_block.index(), block);
                assert_eq!(call.callee().index(), callee);
                let operations = &inventory.operations()[row.operations()];
                assert!(operations.iter().any(|operation| match slot {
                    0 => std::ptr::eq(operation.operation, allocation.operation),
                    1 => matches!(operation.operation.kind, OperationKind::Store { .. }),
                    2 => matches!(operation.operation.kind, OperationKind::Load { .. }),
                    _ => unreachable!(),
                }));
                calls[slot] += 1;
            }
            assert_eq!(calls, [1, 1, 1]);
            let live = budget.storage();
            let (checked, receipt) = bind(inventory, catalog, budget)
                .map_err(ProductionSourceOutputCatalogErrorV1::Binding)?;
            budget.reserve_storage(receipt.retained_storage())?;
            assert!(std::ptr::eq(checked.inventory(), inventory));
            assert!(std::ptr::eq(checked.catalog(), catalog));
            assert_eq!(checked.marker_count(), 0);
            assert!(!checked.grants_authority());
            drop(checked);
            budget.release_storage(receipt.retained_storage())?;
            assert_eq!(budget.storage(), live);
            assert!(!catalog.grants_authority());
            assert!(!view.ranked_verification_is_complete());
            Ok(())
        })
        .unwrap();
    assert!(entered);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
}

#[derive(Clone, Copy)]
enum CrPipelineBindingDamage {
    Missing,
    SubstitutedStorage,
    WrongOperation,
}

fn cr_reject_reachable_pipeline_binding(damage: CrPipelineBindingDamage) {
    use fe2o3_kernel_ir::KernelIrPipelineStorageBindingV1 as Binding;
    let owner = cr_reachable_pipeline_owner();
    let mut work = Work::new(1 << 40);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut rejected = false;
    owner
        .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
            let source = view.metadata(budget)?;
            let catalog = source.catalog(budget)?;
            assert_eq!(catalog.definitions().len(), 1);
            assert_eq!(catalog.bindings().len(), 1);
            let live = budget.storage();
            cr_protected_v1(budget, |budget| {
                budget.reserve_storage(size_of::<Vec<Binding>>())?;
                let mut bindings = cr_vec_v1(catalog.bindings().len(), budget)?;
                for row in catalog.bindings() {
                    cr_push_v1(&mut bindings, *row, budget)?;
                }
                let expected = match damage {
                    CrPipelineBindingDamage::Missing => {
                        bindings.clear();
                        "missing source pipeline allocation"
                    }
                    CrPipelineBindingDamage::SubstitutedStorage => {
                        // A real same-function non-allocation result, not an
                        // invented foreign graph or an invalid framing key.
                        let donor = source
                            .inventory
                            .operations()
                            .iter()
                            .find_map(|row| {
                                if row.coordinate.block.function.0 != bindings[0].function
                                    || matches!(
                                        row.operation.kind,
                                        OperationKind::WorkgroupMemory(_)
                                    )
                                {
                                    return None;
                                }
                                row.operation.results.first().map(|result| result.id.0)
                            })
                            .unwrap();
                        assert_ne!(donor, bindings[0].storage);
                        bindings[0].storage = donor;
                        "missing source pipeline allocation"
                    }
                    CrPipelineBindingDamage::WrongOperation => {
                        bindings[0].operation = bindings[0].operation.checked_add(1).unwrap();
                        "source pipeline allocation identity"
                    }
                };
                let (hostile, receipt) =
                    fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                        *catalog.semantic_source(),
                        catalog.definitions(),
                        &bindings,
                        budget,
                    )
                    .map_err(ProductionSourceOutputCatalogErrorV1::Codec)?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert_eq!(hostile.semantic_source(), catalog.semantic_source());
                assert_eq!(hostile.definitions(), catalog.definitions());
                let binding_floor = budget.storage();
                let graph_result = bind(source.inventory, &hostile, budget);
                match damage {
                    CrPipelineBindingDamage::Missing => {
                        // With no event markers the graph-only checker cannot
                        // know an omitted allocation belongs to source Create.
                        let (checked, receipt) = graph_result.unwrap();
                        budget.reserve_storage(receipt.retained_storage())?;
                        assert_eq!(checked.marker_count(), 0);
                        assert!(!checked.grants_authority());
                        drop(checked);
                        budget.release_storage(receipt.retained_storage())?;
                    }
                    _ => {
                        assert!(matches!(
                        graph_result,
                        Err(fe2o3_kernel_analysis::KernelIrContractCatalogBindingErrorV1::Invalid(
                            "allocation occurrence"
                        ))
                    ))
                    }
                }
                assert_eq!(budget.storage(), binding_floor);
                let result = cr_check_catalog_source_v1(
                    &owner,
                    source.inventory,
                    source.source,
                    &hostile,
                    budget,
                );
                assert!(matches!(result, Err(CrError::Invalid(actual)) if actual == expected));
                rejected = true;
                drop(hostile);
                drop(bindings);
                Ok(())
            })?;
            assert_eq!(budget.storage(), live);
            // A rejected inert row set does not replace or corrupt the actual
            // retained source catalog; recheck that exact original afterward.
            cr_protected_v1(budget, |budget| {
                cr_check_catalog_source_v1(&owner, source.inventory, source.source, catalog, budget)
            })?;
            assert_eq!(budget.storage(), live);
            Ok(())
        })
        .unwrap();
    assert!(rejected);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
}

#[test]
fn canonical_ranked_owner_rejects_missing_reachable_pipeline_binding() {
    cr_reject_reachable_pipeline_binding(CrPipelineBindingDamage::Missing);
}

#[test]
fn canonical_ranked_owner_rejects_same_source_substituted_pipeline_storage() {
    cr_reject_reachable_pipeline_binding(CrPipelineBindingDamage::SubstitutedStorage);
}

#[test]
fn canonical_ranked_owner_rejects_same_source_wrong_pipeline_operation() {
    cr_reject_reachable_pipeline_binding(CrPipelineBindingDamage::WrongOperation);
}

mod canonical_ranked_policy_tests {
    include!("production_canonical_ranked_checks_v1_tests.rs");
}

mod canonical_ranked_native_loop_tests {
    include!("production_canonical_ranked_native_loops_v1_tests.rs");
}
