use super::*;
use crate::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor,
    CanonicalOutputGuardedFormalMemoryErrorV1 as AdapterError,
    analyze_canonical_output_guarded_formal_memory_v1 as analyze,
};

fn with_final(
    unit_local: bool,
    profile: Profile,
    check: impl FnOnce(&VerifiedCanonicalKernelIrModuleV12, Anchor<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    macro_rules! route {
        ($fixture:ident, $variant:ident, $source:ident, $roots:expr) => {{
            let (preheaders, inherited) = $fixture::prefix(profile, true);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (licm, added) = preheaders.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(added.retained_storage()).unwrap();
            let (refined, added) = licm
                .continue_induction_refinement_v1(RefineLimits::default(), &mut budget)
                .unwrap();
            budget.reserve_storage(added.retained_storage()).unwrap();
            let (value, added) = refined
                .continue_cross_block_forwarding_v1(ForwardLimits::default(), &mut budget)
                .unwrap();
            budget.reserve_storage(added.retained_storage()).unwrap();
            actual(
                value.prefix().prefix().output(),
                value.prefix().output(),
                value.output(),
                value.refinement_origins(),
                value.origins(),
                $roots,
                $roots,
            );
            value.verify_equivalence(&mut budget).unwrap();
            let source = value
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .$source();
            let floor = budget.storage();
            check(value.output(), Anchor::$variant(source), &mut budget);
            assert_eq!(budget.storage(), floor);
            drop(value);
        }};
    }
    if unit_local {
        route!(erased, Erased, erased_source, 2);
    } else {
        route!(direct, Direct, source_semantic_kir, 1);
    }
}

#[test]
fn canonical_output_formal_matches_actual_direct_and_erased_refined_forwarded_f() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_final(erased, profile, |output, source, budget| {
                let floor = budget.storage();
                let (report, added) = analyze(output, source, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(std::ptr::eq(report.output(), output));
                assert_eq!(report.kernels().len(), if erased { 2 } else { 1 });
                assert_eq!(report.retained_storage(), added.retained_storage());
                budget.reserve_storage(added.retained_storage()).unwrap();
                let fresh =
                    derive_checked_output_guarded_obligations_v1(output, report.max_operations())
                        .unwrap();
                assert_eq!(report.kernels(), fresh.as_ref());
                assert!(!report.grants_artifact_or_launch_authority());
                drop(fresh);
                drop(report);
                budget.release_storage(added.retained_storage()).unwrap();
            });
        }
    }
}

fn rematerialize(shared: bool, max_operations: usize) -> ProductionPreRankedKirOwnerV1 {
    let ProductionPreRankedKirOwnerV1 {
        semantic_ssa,
        source_launch,
        ..
    } = source(shared, true);
    let limits = ProductionSemanticKirLimitsV1::new_with_max_operations(
        1_024,
        16_384,
        65_536,
        max_operations,
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let inherited = semantic_ssa
        .occurrence_storage()
        .map_or(0, |storage| storage.retained_storage());
    budget.reserve_storage(inherited).unwrap();
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        semantic_ssa,
        source_launch,
        limits,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), inherited);
    owner
}

fn unit_roots(
    original: &ProductionPreRankedKirOwnerV1,
) -> Vec<ProductionRankedSemanticProjectionRootV1> {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    original
        .source_launch()
        .roots()
        .iter()
        .map(|root| {
            let function = &original.semantic_ssa().source_semantic().functions()
                [root.selected_root().index() as usize];
            let name =
                std::str::from_utf8(function.kernel_entry().unwrap().export_symbol().as_bytes())
                    .unwrap();
            let layout = root.layout();
            let kernel = ProductionRankedKernelV1::new(
                name,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel("output_formal_unit", kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            ProductionRankedSemanticProjectionRootV1::new(
                root.selected_root(),
                root.source_rank(),
                lowering,
                "actual private source; empty external footprint".to_owned(),
                vec![],
                vec![],
            )
        })
        .collect()
}

fn with_limit(
    unit_local: bool,
    limit: usize,
    check: impl FnOnce(Anchor<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let original = rematerialize(unit_local, limit);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    if unit_local {
        let roots = unit_roots(&original);
        let input = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
            &original,
            &roots,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(FLOOR + input).unwrap();
        let (owner, added) =
            ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(original, roots, &mut budget)
                .unwrap();
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            (owner.deleted_call_count(), owner.deleted_function_count()),
            (3, 2)
        );
        let floor = budget.storage();
        check(Anchor::Erased(&owner), &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(owner);
    } else {
        let receipt = array_output_ranked_receipt_v1(original);
        budget
            .reserve_storage(
                FLOOR
                    + receipt
                        .materialized
                        .unit_local_source_storage_floor_v1()
                        .unwrap(),
            )
            .unwrap();
        let owner =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
                receipt,
                &mut budget,
            )
            .unwrap();
        let required = owner.pre_ranked_retained_analysis_storage_v1().unwrap();
        if budget.storage() < FLOOR + required {
            budget
                .reserve_storage(FLOOR + required - budget.storage())
                .unwrap();
        }
        let floor = budget.storage();
        check(Anchor::Direct(&owner), &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(owner);
    }
}

// Canonical guarded-load component, not ordinary-rustc or source lineage evidence.
fn guarded_module(extra: usize, mode: usize) -> Module {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Function, Kernel,
        LaunchDomain, LaunchExtent, MemoryAccess, Operation, Signature, Terminator, ValueDef,
        ValueId,
    };
    let scalar = Type::Scalar(ScalarType::U16);
    let slice = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    let mut emit = |id, ty, kind| {
        block
            .operations
            .push(Operation::effect_free(ValueDef::new(ValueId(id), ty), kind))
    };
    emit(
        3,
        pointer.clone(),
        OperationKind::SliceData { slice: ValueId(0) },
    );
    emit(
        4,
        Type::INDEX,
        OperationKind::SliceLength {
            slice: ValueId(if mode == 2 { 1 } else { 0 }),
        },
    );
    emit(5, Type::INDEX, OperationKind::Constant(Constant::Index(0)));
    emit(6, Type::BOOL, OperationKind::Constant(Constant::Bool(true)));
    emit(7, scalar.clone(), OperationKind::Constant(Constant::U16(0)));
    emit(8, Type::INDEX, OperationKind::Constant(Constant::Index(1)));
    emit(
        9,
        Type::INDEX,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(2),
            rhs: ValueId(8),
        },
    );
    emit(
        10,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(9),
            rhs: ValueId(4),
        },
    );
    let guard = ValueId(if mode == 1 { 6 } else { 10 });
    emit(
        11,
        Type::INDEX,
        OperationKind::Select {
            condition: guard,
            true_value: ValueId(9),
            false_value: ValueId(5),
        },
    );
    emit(
        12,
        pointer,
        OperationKind::GetElementPointer {
            base: ValueId(3),
            offset: ValueId(11),
        },
    );
    emit(
        13,
        scalar.clone(),
        OperationKind::GuardedLoad {
            pointer: ValueId(12),
            predicate: guard,
            fallback: ValueId(7),
            access: MemoryAccess::new(AddressSpace::Global, 2),
        },
    );
    if mode == 3 || mode == 4 {
        emit(
            14,
            scalar,
            OperationKind::Load {
                pointer: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 2),
            },
        );
    }
    for ordinal in 0..extra {
        emit(
            100 + u32::try_from(ordinal).unwrap(),
            Type::INDEX,
            OperationKind::Constant(Constant::Index(0)),
        );
    }
    if mode == 4 {
        block
            .operations
            .retain(|operation| operation.results[0].id != ValueId(13));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("output-formal-guard");
    module.functions.push(Function::kernel_entry(
        "guard",
        Signature::new(vec![slice.clone(), slice, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "guard",
        "guard",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

#[test]
fn canonical_output_formal_uses_each_actual_nondefault_source_limit() {
    for (erased, limit) in [(false, 1_024), (true, 2_048)] {
        with_limit(erased, limit, |source, budget| {
            let floor = budget.storage();
            for exceeds in [false, true] {
                let module = guarded_module(if exceeds { limit } else { 0 }, 0);
                let (output, storage) = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let result = analyze(&output, source, budget);
                if exceeds {
                    let Err(AdapterError::Formal(
                        crate::ProductionFormalMemoryErrorV1::GuardedAccessDischarge {
                            reasons,
                            detail,
                        },
                    )) = result
                    else {
                        panic!(
                            "retained source operation limit must refuse the larger guarded graph"
                        )
                    };
                    assert!(!reasons.is_empty());
                    assert_eq!(
                        detail,
                        crate::ProductionMemoryDischargeFailureV1::Stage(
                            "guarded proof exceeded the operation resource limit"
                        )
                    );
                } else {
                    let (report, added) = result.unwrap();
                    assert_eq!(report.max_operations(), limit);
                    budget.reserve_storage(added.retained_storage()).unwrap();
                    assert_eq!(
                        report.kernels(),
                        derive_checked_output_guarded_obligations_v1(&output, limit)
                            .unwrap()
                            .as_ref()
                    );
                    assert_eq!(report.kernels()[0].accesses().len(), 1);
                    drop(report);
                    budget.release_storage(added.retained_storage()).unwrap();
                }
                drop(output);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            }
        });
    }
}

#[test]
fn canonical_output_formal_preserves_guarded_and_mixed_reason_refusals() {
    with_limit(false, 1_024, |source, budget| {
        let floor = budget.storage();
        for mode in 1..=4 {
            let module = guarded_module(0, mode);
            let (output, storage) =
                VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                    &module, budget,
                )
                .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            match (mode, analyze(&output, source, budget)) {
                (
                    1 | 2,
                    Err(AdapterError::Formal(
                        crate::ProductionFormalMemoryErrorV1::GuardedAccessDischarge { .. },
                    )),
                ) => {}
                (
                    3 | 4,
                    Err(AdapterError::Formal(crate::ProductionFormalMemoryErrorV1::Incomplete {
                        reasons,
                    })),
                ) => {
                    assert!(reasons.iter().any(|r| matches!(r, fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. })));
                    assert_eq!(reasons.iter().any(|r| matches!(r, fe2o3_kernel_ir::FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. })), mode == 3);
                }
                _ => panic!("unchanged exact formal-policy refusal"),
            }
            drop(output);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn canonical_output_formal_public_entry_precedes_source_checks_and_keeps_typed_replay_error() {
    for erased in [false, true] {
        with_final(erased, Profile::Gfx942, |output, source, inherited| {
            let floor = inherited.storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let Err(AdapterError::Resource(AssertOriginResourceV1::Work(error))) =
                analyze(output, source, &mut budget)
            else {
                panic!("original ledger entry charge precedes source replay")
            };
            assert_eq!((error.actual(), error.limit()), (4, 3));
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (0, floor, floor)
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let Err(AdapterError::Source(actual)) = analyze(output, source, &mut budget) else {
                panic!("source replay keeps its typed child error")
            };
            let actual_work = budget.work();
            let actual_peak = budget.peak_storage();
            assert_eq!(budget.storage(), floor);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
            let mut reference = AssertOriginBudgetV1::new(&mut work, STORAGE);
            reference.reserve_storage(floor).unwrap();
            reference.charge_work(4).unwrap();
            let expected = match source {
                Anchor::Direct(source) => source.verify_equivalence_with_budget_v1(&mut reference),
                Anchor::Erased(source) => source.verify_equivalence(&mut reference),
            }
            .unwrap_err();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            assert_eq!(
                (actual_work, actual_peak),
                (reference.work(), reference.peak_storage())
            );
            let required = match source {
                Anchor::Direct(source) => source.pre_ranked_retained_analysis_storage_v1().unwrap(),
                Anchor::Erased(source) => source.retained_storage_floor_v1(),
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut short_floor = AssertOriginBudgetV1::new(&mut work, STORAGE);
            short_floor.reserve_storage(required - 1).unwrap();
            assert!(matches!(
                analyze(output, source, &mut short_floor),
                Err(AdapterError::Resource(AssertOriginResourceV1::Accounting))
            ));
            assert_eq!(
                (
                    short_floor.work(),
                    short_floor.storage(),
                    short_floor.peak_storage()
                ),
                (4, required - 1, required - 1)
            );
        });
    }
}

#[test]
fn canonical_output_formal_disconnected_legacy_source_refuses_after_exact_public_entry() {
    let (semantic_ssa, launch) = fixture(Fixture::Literal(true), false);
    drop(launch);
    let source = ProductionSemanticKirOwnerV1::try_lower(
        semantic_ssa.into_source_owner().unwrap(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    source.verify_equivalence().unwrap();
    assert!(source.pre_ranked_executable().is_none());
    assert!(source.pre_ranked_retained_analysis_storage_v1().is_none());
    assert!(!source.retains_mandatory_generic_checks());
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut setup = AssertOriginBudgetV1::new(&mut setup_work, STORAGE);
    let (output, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            source.module(),
            &mut setup,
        )
        .unwrap();
    setup.reserve_storage(storage.retained_storage()).unwrap();
    let sibling = [0x75_u8; 23];
    let floor = storage
        .retained_storage()
        .checked_add(sibling.len())
        .unwrap();
    for limit in [3, 4] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        {
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = analyze(&output, Anchor::Direct(&source), &mut budget);
            if limit == 3 {
                let Err(AdapterError::Resource(AssertOriginResourceV1::Work(error))) = result
                else {
                    panic!("public entry must precede disconnected-source refusal");
                };
                assert_eq!((error.actual(), error.limit()), (4, 3));
            } else {
                assert!(matches!(result, Err(AdapterError::MissingConnectedSource)));
            }
            assert_eq!(budget.work(), if limit == 3 { 0 } else { 4 });
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            assert_eq!(budget.failed_storage(), None);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x75; 23]);
        }
        assert_eq!(work.failed_work(), if limit == 3 { Some(4) } else { None });
    }
    drop(output);
    setup.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(setup.storage(), 0);
    drop(source);
}

#[test]
fn canonical_output_formal_complete_public_work_peak_and_last_charge_are_exact() {
    for erased in [false, true] {
        with_final(erased, Profile::Gfx942, |output, source, inherited| {
            let sibling = vec![0x67u8; 29];
            let floor = inherited.storage() + std::mem::size_of_val(&sibling) + sibling.capacity();
            let run = |work_limit, storage_limit| {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                work.charge_work(17).unwrap();
                let record = {
                    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
                    budget.reserve_storage(floor).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let result = analyze(output, source, &mut budget).map(|(report, storage)| {
                        assert_eq!(report.retained_storage(), storage.retained_storage());
                        assert!(std::ptr::eq(report.output(), output));
                        drop(report);
                    });
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(sibling, [0x67; 29]);
                    (
                        result,
                        budget.work(),
                        budget.peak_storage(),
                        budget.failed_storage(),
                    )
                };
                (record, work.failed_work())
            };
            let ((result, used, peak, failed_storage), failed_work) = run(WORK, STORAGE);
            result.unwrap();
            assert_eq!((failed_storage, failed_work), (None, None));
            let ((result, exact_work, exact_peak, failed_storage), failed_work) = run(used, peak);
            result.unwrap();
            assert_eq!(
                (exact_work, exact_peak, failed_storage, failed_work),
                (used, peak, None, None)
            );
            let ((result, short_work, short_peak, failed_storage), failed_work) =
                run(used - 1, peak);
            let Err(AdapterError::Resource(AssertOriginResourceV1::Work(error))) = result else {
                panic!("full public adapter final charge")
            };
            assert_eq!((error.actual(), error.limit()), (used, used - 1));
            assert_eq!(
                (short_work, short_peak, failed_storage, failed_work),
                (used - 1, peak, None, Some(used))
            );
        });
    }
}

#[test]
fn canonical_output_formal_preserves_actual_cross_invocation_conflicts() {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, Function, Kernel, LaunchDomain,
        LaunchExtent, MemoryAccess, Operation, Signature, Terminator, ValueDef, ValueId,
    };
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(9)),
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("output-formal-conflict");
    module.functions.push(Function::kernel_entry(
        "conflict",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "conflict",
        "conflict",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    with_limit(false, 1_024, |source, budget| {
        let (output, receipt) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &module, budget,
            )
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let expected = derive_checked_output_guarded_obligations_v1(&output, 1_024).unwrap_err();
        let crate::ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: expected,
        } = expected
        else {
            panic!("actual fixed-address cross-invocation store conflict")
        };
        assert!(!expected.is_empty());
        let Err(AdapterError::Formal(
            crate::ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts },
        )) = analyze(&output, source, budget)
        else {
            panic!("adapter must preserve the exact conflict refusal")
        };
        assert_eq!(conflicts, expected);
        drop(output);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}
