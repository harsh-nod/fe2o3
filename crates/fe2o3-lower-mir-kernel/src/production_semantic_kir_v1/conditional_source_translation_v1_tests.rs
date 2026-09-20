fn conditional_pending(
    recipe: ProductionRankedKernelV1,
) -> fe2o3_pliron::ProductionConditionalRankedAnalysisV1 {
    use fe2o3_pliron::{ProductionConditionalOwnershipSiteV1, ProductionPlironSessionV1};
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("conditional_source", recipe).unwrap(),
        )
        .unwrap();
    let (stage, root) = session.construct_registered(registered).unwrap();
    session
        .prepare_conditional_ranked_analysis_v1(
            stage,
            root,
            &[ProductionConditionalOwnershipSiteV1 {
                block: 0,
                operation: 4,
                view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            }],
        )
        .unwrap()
}

fn conditional_candidate<'a>(
    pending: &'a fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    rows: &'a [ProductionRankedAccessSourceV1],
) -> crate::NativeRankedSourceCandidateV1<'a> {
    crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
        0,
        1,
        pending.kernel().unwrap(),
        rows,
        &[],
        "diagnostic only",
    )
}

fn conditional_rows() -> [ProductionRankedAccessSourceV1; 1] {
    [ProductionRankedAccessSourceV1::new(0, Some(2), 0, 0, 5)]
}

#[test]
fn conditional_translation_requires_retained_source_storage() {
    let source = ranked_wrapping_source(false, 32, 1);
    let pending = conditional_pending(ranked_wrapping_recipe(
        &source,
        false,
        32,
        1,
        RankedMutation::None,
    ));
    let rows = conditional_rows();
    let floor = source.retained_analysis_storage_v1();
    assert!(floor > 0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor - 1).unwrap();
    assert!(matches!(
        source.check_conditional_source_translation_v1(
            &pending,
            conditional_candidate(&pending, &rows),
            &mut budget
        ),
        Err(
            ProductionConditionalSourceTranslationErrorV1::Correspondence(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        )
    ));
    assert_eq!(budget.storage(), floor - 1);
}

fn conditional_check<'a>(
    source: &'a ProductionPreRankedKirOwnerV1,
    pending: &'a fe2o3_pliron::ProductionConditionalRankedAnalysisV1,
    candidate: crate::NativeRankedSourceCandidateV1<'_>,
) -> Result<
    ProductionConditionalSourceTranslationV1<'a>,
    ProductionConditionalSourceTranslationErrorV1,
> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + 19;
    budget.reserve_storage(floor).unwrap();
    let result = source.check_conditional_source_translation_v1(pending, candidate, &mut budget);
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn conditional_translation_retains_actual_owners_without_clean_conversion() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let first = conditional_pending(recipe.clone());
    let second = conditional_pending(recipe);
    let rows = conditional_rows();
    for pending in [&first, &second] {
        let result =
            conditional_check(&source, pending, conditional_candidate(pending, &rows)).unwrap();
        assert!(std::ptr::eq(result.source(), &source));
        assert!(std::ptr::eq(result.pending(), pending));
        assert_eq!(result.memory_effects(), 1);
        assert_eq!(result.value_expressions(), 1);
        assert!(!pending.pending_pipeline_checks().is_empty());
    }
    assert!(matches!(
        conditional_check(&source, &first, conditional_candidate(&second, &rows)),
        Err(ProductionConditionalSourceTranslationErrorV1::ForeignRecipe),
    ));
    let detached = first.kernel().unwrap().clone();
    let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
        0,
        1,
        &detached,
        &rows,
        &[],
        "same bytes",
    );
    assert!(matches!(
        conditional_check(&source, &first, candidate),
        Err(ProductionConditionalSourceTranslationErrorV1::ForeignRecipe)
    ));
}

#[test]
fn conditional_translation_rejects_real_source_and_ranked_value_mutations() {
    let source = ranked_wrapping_source(false, 32, 1);
    let rows = conditional_rows();
    for mutation in [
        RankedMutation::SwapOperands,
        RankedMutation::ReplaceOperand,
        RankedMutation::Operator,
        RankedMutation::Signedness,
        RankedMutation::Width,
    ] {
        let pending = conditional_pending(ranked_wrapping_recipe(&source, false, 32, 1, mutation));
        assert!(
            matches!(
                conditional_check(&source, &pending, conditional_candidate(&pending, &rows)),
                Err(
                    ProductionConditionalSourceTranslationErrorV1::Correspondence(
                        ProductionSemanticKirErrorV1::MirPlironTranslation(_)
                    )
                )
            ),
            "{mutation:?}"
        );
    }
    let pending = conditional_pending(ranked_wrapping_recipe(
        &source,
        false,
        32,
        1,
        RankedMutation::None,
    ));
    let changed_source = ranked_wrapping_source(false, 32, 0);
    assert!(matches!(
        conditional_check(
            &changed_source,
            &pending,
            conditional_candidate(&pending, &rows)
        ),
        Err(
            ProductionConditionalSourceTranslationErrorV1::Correspondence(
                ProductionSemanticKirErrorV1::MirPlironTranslation(_)
            )
        )
    ));
}

#[test]
fn conditional_translation_rejects_missing_duplicate_and_misdirected_rows() {
    let source = ranked_wrapping_source(false, 32, 1);
    let pending = conditional_pending(ranked_wrapping_recipe(
        &source,
        false,
        32,
        1,
        RankedMutation::None,
    ));
    let row = conditional_rows()[0];
    let missing = conditional_check(&source, &pending, conditional_candidate(&pending, &[]));
    assert!(matches!(
        missing,
        Err(
            ProductionConditionalSourceTranslationErrorV1::Correspondence(
                ProductionSemanticKirErrorV1::MirPlironTranslation(_)
            )
        )
    ));
    for rows in [
        vec![row, row],
        vec![ProductionRankedAccessSourceV1::new(0, Some(2), 1, 0, 5)],
        vec![ProductionRankedAccessSourceV1::new(0, Some(2), 0, 0, 4)],
    ] {
        assert!(matches!(
            conditional_check(&source, &pending, conditional_candidate(&pending, &rows)),
            Err(ProductionConditionalSourceTranslationErrorV1::SourceRows)
        ));
    }
}

#[test]
fn conditional_translation_rejects_wrong_root_rank_and_layout() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let pending = conditional_pending(recipe.clone());
    let rows = conditional_rows();
    for (root, rank) in [(u32::MAX, 1), (0, 2)] {
        let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
            root,
            rank,
            pending.kernel().unwrap(),
            &rows,
            &[],
            "",
        );
        assert!(matches!(
            conditional_check(&source, &pending, candidate),
            Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
        ));
    }
    for axis in 0..3 {
        let mut operations = recipe.blocks()[0].operations().to_vec();
        let ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            ..
        } = &mut operations[0]
        else {
            unreachable!()
        };
        match axis {
            0 => *grid_identity += 1,
            1 => global_extents[0] = 2,
            _ => {
                global_extents[0] = 2;
                workgroup_extents[0] = 2;
            }
        }
        let changed = ProductionRankedKernelV1::new(
            NAME,
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let pending = conditional_pending(changed);
        assert!(matches!(
            conditional_check(&source, &pending, conditional_candidate(&pending, &rows)),
            Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
        ));
    }
}

#[test]
fn conditional_translation_rejects_changed_allocation_and_removed_effect() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let rows = conditional_rows();
    for remove in [false, true] {
        let mut operations = recipe.blocks()[0].operations().to_vec();
        if remove {
            operations.pop();
        } else {
            match &mut operations[1] {
                ProductionRankedOperationV1::View {
                    allocation_origin, ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    allocation_origin, ..
                } => *allocation_origin = 2,
                _ => unreachable!(),
            }
        }
        let changed = ProductionRankedKernelV1::new(
            NAME,
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let pending = conditional_pending(changed);
        let result = conditional_check(&source, &pending, conditional_candidate(&pending, &rows));
        if remove {
            assert!(matches!(
                result,
                Err(ProductionConditionalSourceTranslationErrorV1::SourceRows)
            ));
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionConditionalSourceTranslationErrorV1::Correspondence(
                        ProductionSemanticKirErrorV1::MirPlironTranslation(
                            ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch { .. }
                        )
                    )
                )
            ));
        }
    }
}

#[test]
fn conditional_translation_rejects_unmapped_extra_ranked_memory() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let rows = conditional_rows();
    for extra in [
        recipe.blocks()[0].operations()[5].clone(),
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
            indices: vec![ProductionRankedValueV1::Local(
                ProductionRankedValueIdV1::new(1),
            )],
        },
        ProductionRankedOperationV1::AllocationEffect {
            kind: AccessKindAttr::Read,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
    ] {
        let mut operations = recipe.blocks()[0].operations().to_vec();
        operations.push(extra);
        let changed = ProductionRankedKernelV1::new(
            NAME,
            0,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let pending = conditional_pending(changed);
        assert!(matches!(
            conditional_check(&source, &pending, conditional_candidate(&pending, &rows)),
            Err(
                ProductionConditionalSourceTranslationErrorV1::Correspondence(
                    ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::ExtraRankedEffect {
                            ranked_block: 0,
                            ranked_operation: 6
                        }
                    )
                )
            )
        ));
    }
}

#[test]
fn clean_translation_also_rejects_unmapped_extra_ranked_read() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let mut operations = recipe.blocks()[0].operations().to_vec();
    operations.push(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Read,
        view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        indices: vec![ProductionRankedValueV1::Local(
            ProductionRankedValueIdV1::new(1),
        )],
    });
    let changed = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("extra_read", changed).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    assert!(matches!(
        ranked_wrapping_attach(source, lowering, RankedMutation::None),
        Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::ExtraRankedEffect {
                ranked_block: 0,
                ranked_operation: 6
            }
        ))
    ));
}

fn conditional_extra_effect_is_rejected_at(
    source: &ProductionPreRankedKirOwnerV1,
    operations: Vec<ProductionRankedOperationV1>,
    expected_operation: u32,
) {
    let recipe = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("the extra effect must first pass the actual ranked recipe constructor");
    let pending = conditional_pending(recipe);
    let rows = conditional_rows();
    assert!(matches!(
        conditional_check(source, &pending, conditional_candidate(&pending, &rows)),
        Err(ProductionConditionalSourceTranslationErrorV1::Correspondence(
            ProductionSemanticKirErrorV1::MirPlironTranslation(
                ProductionMirPlironTranslationErrorV1::ExtraRankedEffect {
                    ranked_block: 0,
                    ranked_operation,
                }
            )
        )) if ranked_operation == expected_operation
    ));
}

#[test]
fn conditional_translation_rejects_constructor_admitted_extra_atomic_access() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let mut operations = recipe.blocks()[0].operations().to_vec();
    operations.push(ProductionRankedOperationV1::AtomicAccess {
        kind: AccessKindAttr::AtomicRead,
        ordering: dialect_kernel::AtomicOrderingAttr::Acquire,
        scope: dialect_kernel::AtomicScopeAttr::Device,
        view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        indices: vec![ProductionRankedValueV1::Local(
            ProductionRankedValueIdV1::new(1),
        )],
    });
    conditional_extra_effect_is_rejected_at(&source, operations, 6);
}

#[test]
fn conditional_translation_rejects_constructor_admitted_extra_atomic_value_access() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let mut operations = recipe.blocks()[0].operations().to_vec();
    operations.push(ProductionRankedOperationV1::AtomicValueAccess {
        kind: AccessKindAttr::AtomicWrite,
        ordering: dialect_kernel::AtomicOrderingAttr::Release,
        scope: dialect_kernel::AtomicScopeAttr::Device,
        view: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        indices: vec![ProductionRankedValueV1::Local(
            ProductionRankedValueIdV1::new(1),
        )],
        value: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(2)),
    });
    conditional_extra_effect_is_rejected_at(&source, operations, 6);
}

#[test]
fn conditional_translation_rejects_constructor_admitted_extra_predicated_access() {
    let source = ranked_wrapping_source(false, 32, 1);
    let recipe = ranked_wrapping_recipe(&source, false, 32, 1, RankedMutation::None);
    let mut operations = recipe.blocks()[0].operations().to_vec();
    let id = ProductionRankedValueIdV1::new;
    let local = |value| ProductionRankedValueV1::Local(id(value));
    // A real dynamic view and the exact paired success/index are constructor
    // requirements. A detached Boolean or the old static view is not sufficient.
    operations.extend([
        ProductionRankedOperationV1::IndexConstant {
            result: id(3),
            value: 1,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: id(4),
            element_width: 32,
            writable: true,
            shape: vec![0],
            dynamic_extents: vec![local(3)],
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: 2,
            noalias_class: 2,
        },
        ProductionRankedOperationV1::PredicatedCheckedRowStripedIndex2D {
            result: id(5),
            success: id(6),
            invocation: local(1),
            component: local(1),
            rows: local(3),
            columns: local(3),
            row_stride: local(3),
            physical_extent: local(3),
            lanes_per_row: 1,
            elements_per_lane: 1,
        },
        ProductionRankedOperationV1::PredicatedAccess {
            kind: AccessKindAttr::Read,
            view: local(4),
            index: local(5),
            success: local(6),
        },
    ]);
    conditional_extra_effect_is_rejected_at(&source, operations, 9);
}
