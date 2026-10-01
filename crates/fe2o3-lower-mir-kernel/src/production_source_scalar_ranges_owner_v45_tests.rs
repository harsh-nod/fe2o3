use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[test]
fn original_scalar_range_query_has_exact_geometry_independent_headers_and_resource_cuts() {
    use std::mem::size_of;
    type Frame<'a> = (
        &'a SemanticFunctionDeclV1,
        &'a [SemanticTypeDeclV1],
        &'a SemanticPlaceV1,
        &'a SemanticTypeDeclV1,
        &'a [SemanticTypeIdV1],
        &'a [u64],
        std::slice::Iter<'a, SemanticProjectionV1>,
        &'a SemanticProjectionV1,
        SemanticTypeIdV1,
        SemanticTypeIdV1,
        SourceScalarByteRangeV45,
        u64,
        u64,
        u64,
    );
    type Value = (SemanticTypeIdV1, SourceScalarByteRangeV45);
    let headers = size_of::<Frame<'_>>()
        + 2 * size_of::<Result<Frame<'_>, ProductionSemanticKirErrorV1>>()
        + size_of::<Value>()
        + 2 * size_of::<Result<Value, ProductionSemanticKirErrorV1>>();
    assert_eq!(
        size_of::<Frame<'_>>(),
        size_of::<SourceScalarRangeFrameV45<'_>>()
    );
    let owner = retained_checked_owner(SemanticCheckedBinaryOpV1::Add);
    let source = owner.source_semantic();
    let original = &source.functions()[0];
    for (field, ty, start, end) in [(0, U32, 0, 4), (1, original.locals()[3].ty(), 4, 5)] {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
            ty,
        )
        .unwrap();
        let run = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let result = source_scalar_range_v45(original, source.types(), &place, &mut budget);
            assert_eq!(budget.storage(), MODULE_FLOOR);
            (result, budget.work(), budget.peak_storage())
        };
        let exact = run(21, MODULE_FLOOR + headers);
        assert_eq!(
            exact.0.unwrap(),
            (
                original.locals()[4].ty(),
                SourceScalarByteRangeV45 { start, end }
            )
        );
        assert_eq!((exact.1, exact.2), (6 + 9 + 6, MODULE_FLOOR + headers));
        assert!(matches!(run(20, MODULE_FLOOR + headers).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
            if error.limit() == 20 && error.actual() == 21));
        assert!(matches!(run(21, MODULE_FLOOR + headers - 1).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
            if error.limit() == MODULE_FLOOR + headers - 1 && error.actual() == MODULE_FLOOR + headers));
    }
    for (local, kind, ty) in [
        (4, SemanticProjectionKindV1::Field(2), U32),
        (4, SemanticProjectionKindV1::Field(1), U32),
        (4, SemanticProjectionKindV1::Dereference, U32),
        (u32::MAX, SemanticProjectionKindV1::Field(0), U32),
    ] {
        let malformed = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(kind, ty).unwrap()],
            ty,
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        assert!(matches!(
            source_scalar_range_v45(original, source.types(), &malformed, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn scalar_move_kill_order_headers_cover_the_full_original_key() {
    use std::mem::size_of;
    type Frame<'a> = (
        Option<(BlockId, usize, [usize; 5], usize)>,
        (BlockId, usize, [usize; 5], usize),
        &'a SourceAddressKillV29,
    );
    assert_eq!(
        size_of::<Frame<'_>>(),
        size_of::<SourceAddressKillOrderFrameV45<'_>>()
    );
    let expected =
        size_of::<Frame<'_>>() + 2 * size_of::<Result<Frame<'_>, ProductionSemanticKirErrorV1>>();
    let actual =
        source_reference_emission_headers_v29::<SourceAddressKillOrderFrameV45<'_>>().unwrap();
    assert_eq!(actual, expected);
    for limit in [expected, expected - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_FLOOR + limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            budget
                .reserve_storage(actual)
                .map_err(ProductionSemanticKirErrorV1::from)
        });
        if limit == expected {
            result.unwrap();
            assert_eq!(budget.peak_storage(), MODULE_FLOOR + expected);
        } else {
            assert!(matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error)))
                if error.limit() == MODULE_FLOOR + limit && error.actual() == MODULE_FLOOR + expected));
        }
        assert_eq!((budget.work(), budget.storage()), (0, MODULE_FLOOR));
    }
}

#[test]
fn original_scalar_windows_follow_nested_array_stride_reverse_indices_and_exact_types() {
    let owner = retained_checked_owner(SemanticCheckedBinaryOpV1::Add);
    let source = owner.source_semantic();
    let old = &source.functions()[0];
    let pair = old.locals()[4].ty();
    let boolean = old.locals()[3].ty();
    let mut types = source.types().to_vec();
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([249; 32]),
        SemanticLayoutIdentityV1::from_sha256([249; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            32,
            4,
            SemanticFieldsShapeV1::array(8, 4),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: pair,
            length: 4,
        },
    ));
    let mut locals = old.locals().to_vec();
    let index = locals.len() as u32;
    locals.push(local(250, array, SemanticLocalRoleV1::Temporary));
    let original = rebuild_root(old, locals, old.blocks().to_vec());
    let make = |offset, minimum_length, from_end, child, field, result| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(index),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                    child,
                )
                .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), result).unwrap(),
            ],
            result,
        )
        .unwrap()
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    for (from_end, base) in [(false, 8), (true, 24)] {
        for (field, result, relative, bytes) in [(0, U32, 0, 4), (1, boolean, 4, 1)] {
            let before = budget.work();
            assert_eq!(
                source_scalar_range_v45(
                    &original,
                    &types,
                    &make(1, 4, from_end, pair, field, result),
                    &mut budget
                )
                .unwrap(),
                (
                    array,
                    SourceScalarByteRangeV45 {
                        start: base + relative,
                        end: base + relative + bytes
                    }
                )
            );
            assert_eq!(budget.work() - before, 6 + 2 * 9 + 6);
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
    for place in [
        make(4, 5, false, pair, 0, U32),
        make(5, 5, true, pair, 0, U32),
        make(1, 4, false, U32, 0, U32),
        make(1, 4, false, pair, 1, U32),
    ] {
        assert!(matches!(
            source_scalar_range_v45(&original, &types, &place, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
    // Inexpressible original offsets are rejected before the layout query.
    for (offset, from_end) in [(4, false), (0, true), (5, true)] {
        assert!(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length: 4,
                    from_end,
                },
                pair
            )
            .is_err()
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum RangeCaseV45 {
    ConditionMove,
    FailureCopy,
    FailureMove,
    FailureMoveThenRead,
    MoveThenDead,
    SelfAssignment,
    ReadMoved,
}

fn scalar_range_owner_v45(case: RangeCaseV45) -> ProductionSemanticSsaOwnerV1 {
    let base = retained_checked_owner(SemanticCheckedBinaryOpV1::Add);
    let source = base.source_semantic();
    let original = &source.functions()[0];
    let pair = original.locals()[4].ty();
    let boolean = original.locals()[3].ty();
    let field = |index, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let marker = |live, local| {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            if live {
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local))
            } else {
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local))
            },
        )
    };
    let mut blocks = original.blocks().to_vec();
    let body = &original.blocks()[2];
    let mut statements = body.statements().to_vec();
    let mut terminator = body.terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert {
        condition, message, ..
    } = &mut terminator
    else {
        panic!("original checked overflow guard");
    };
    match case {
        RangeCaseV45::ConditionMove => *condition = SemanticOperandV1::Move(field(1, boolean)),
        RangeCaseV45::FailureCopy => {
            *message =
                SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Copy(field(0, U32)))
        }
        RangeCaseV45::FailureMove => {
            *message =
                SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(field(0, U32)))
        }
        RangeCaseV45::FailureMoveThenRead => {
            *message = SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Move(field(0, U32)),
                right: SemanticOperandV1::Copy(field(0, U32)),
            }
        }
        RangeCaseV45::MoveThenDead => {
            statements.insert(0, marker(true, 4));
            // Remove the address carrier before ending its referent lifetime.
            statements.push(marker(false, 5));
            statements.push(assign(
                place(3, boolean),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(1, boolean))),
            ));
            statements.push(marker(false, 4));
            terminator = SemanticTerminatorKindV1::Return;
        }
        RangeCaseV45::SelfAssignment => statements.push(assign(
            field(0, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, U32))),
        )),
        RangeCaseV45::ReadMoved => statements.push(assign(
            place(2, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(0, U32))),
        )),
    }
    assert!(statements.iter().any(|statement| matches!(statement.kind(),
        SemanticStatementKindV1::Assign(assignment)
        if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
            if place.local().index() == 4 && place.ty() == pair))));
    blocks[2] = SemanticBasicBlockV1::new(
        body.identity(),
        body.source(),
        statements,
        SemanticTerminatorV1::new(body.terminator().source(), terminator),
    )
    .unwrap();
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(original, original.locals().to_vec(), blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(
        !owner.plans()[0]
            .plan()
            .promoted_variables()
            .iter()
            .any(|local| local.get() == 4)
    );
    owner
}

fn run_scalar_range_owner_v45(
    case: RangeCaseV45,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    run_scalar_range_owner_mode_v45(case, work, storage, false)
}

fn run_scalar_range_owner_mode_v45(
    case: RangeCaseV45,
    work: usize,
    storage: usize,
    optimized: bool,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    let mut ledger = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut ledger, storage);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let projection = scalar_range_owner_v45(case);
    let owner = scalar_range_owner_v45(case);
    let mut reached = false;
    let result = (|| -> SourceOwnedResultV18<()> {
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        with_module_fixture_view(&projection, ModuleFixture::Ordinary, &mut budget,
            |input, budget| -> SourceOwnedResultV18<()> {
                let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                let roots = fixture.roots();
                let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner, launch, input.input, ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(), budget,
                )?;
                if optimized {
                    return with_production_optimized_consumer_v18(prepared, budget,
                        |original, optimized, budget| {
                            let expected = original.source(budget)?.root_count(budget)?;
                            assert_eq!(expected, 1);
                            let floor = budget.storage();
                            assert_eq!(original.check_optimized_source_currentness_v18(
                                optimized, budget,
                            )?, expected);
                            assert_eq!(budget.storage(), floor);
                            reached = true;
                            Ok(())
                        });
                }
                prepared.with_source_consumer_v18(budget, |source, budget| {
                    source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                            source_scalar_normalization_scratch_v18(source.cleanup, budget, 0, |budget| {
                                scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget, |memory, budget| -> SourceOwnedResultV18<()> {
                                    let (rows, kills) = memory.scalar_range_test_rows_v45(budget)?;
                                    assert!(!rows.is_empty(), "{case:?}");
                                    let failure = matches!(case, RangeCaseV45::FailureCopy | RangeCaseV45::FailureMove);
                                    assert!(rows.iter().any(|row| row.failure_only == failure
                                        && row.range.start < row.range.end));
                                    if matches!(case, RangeCaseV45::MoveThenDead) {
                                        assert!(rows.iter().any(|row| !row.failure_only &&
                                            kills.iter().any(|kill|
                                                (kill.block, kill.gap, kill.slot) == (row.block, row.gap, row.slot)
                                                && kill.source_order > row.source_order)));
                                    }
                                    reached = true;
                                    Ok(())
                                })
                            })
                        })
                    }))
                })
            })?.0
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{case:?}: {result:?}");
    (result, budget.work(), budget.peak_storage(), reached)
}

#[test]
fn retained_scalar_range_moves_reach_exact_original_byte_history() {
    for case in [
        RangeCaseV45::ConditionMove,
        RangeCaseV45::FailureCopy,
        RangeCaseV45::FailureMove,
        RangeCaseV45::MoveThenDead,
        RangeCaseV45::SelfAssignment,
    ] {
        let observed =
            run_scalar_range_owner_v45(case, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        observed.0.unwrap();
        assert!(observed.3, "{case:?}");
    }
}

#[test]
fn retained_scalar_range_moves_reject_failure_and_success_reads_after_consumption() {
    for case in [RangeCaseV45::FailureMoveThenRead, RangeCaseV45::ReadMoved] {
        let observed =
            run_scalar_range_owner_v45(case, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(!observed.3, "{case:?}");
        let error = observed.0.expect_err("consumed original scalar range");
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
        let mut exact = false;
        while let Some(part) = current {
            assert!(
                part.downcast_ref::<ArgumentResourceV1>().is_none(),
                "{error:?}"
            );
            exact |= matches!(
                part.downcast_ref::<ProductionSemanticKirErrorV1>(),
                Some(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped source-slot allocation census is incomplete or mismatched",
                    ..
                })
            );
            current = part.source();
        }
        assert!(exact, "{case:?}: {error:?}");
    }
}

#[test]
fn retained_scalar_range_moves_rejoin_checked_optimized_memory_and_order() {
    for case in [
        RangeCaseV45::ConditionMove,
        RangeCaseV45::FailureMove,
        RangeCaseV45::MoveThenDead,
        RangeCaseV45::SelfAssignment,
    ] {
        let observed = run_scalar_range_owner_mode_v45(
            case,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            true,
        );
        observed.0.unwrap();
        assert!(observed.3, "{case:?}");
    }
}

#[test]
fn retained_scalar_range_move_complete_transaction_has_exact_resource_boundaries() {
    let case = RangeCaseV45::ConditionMove;
    let measured = run_scalar_range_owner_v45(case, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    measured.0.unwrap();
    assert!(measured.3);
    let exact = run_scalar_range_owner_v45(case, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, measured.2, true));
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.2, true),
        (measured.1, measured.2 - 1, false),
    ] {
        let short = run_scalar_range_owner_v45(case, work, storage);
        let mut error: &(dyn std::error::Error + 'static) = short.0.as_ref().unwrap_err();
        loop {
            if let Some(resource) = error.downcast_ref::<ArgumentResourceV1>() {
                match resource {
                    ArgumentResourceV1::Work(limit) if is_work => {
                        assert_eq!(limit.limit(), work);
                        assert_eq!(limit.actual(), measured.1);
                    }
                    ArgumentResourceV1::Storage(limit) if !is_work => {
                        assert_eq!(limit.limit(), storage);
                        assert_eq!(limit.actual(), measured.2);
                    }
                    other => panic!("wrong resource: {other:?}"),
                }
                break;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing resource: {:?}", short.0));
        }
    }
}

thread_local! {
    static SCALAR_MOVE_FAULT_V45: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SCALAR_MOVE_MUTATIONS_V45: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn alter_scalar_move_anchor_v45(
    _: &ExecutionLifecycleSourceV29<'_>,
    _: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let fault = SCALAR_MOVE_FAULT_V45.get();
    for lowered in emitted.iter_mut().flatten() {
        let Some(anchors) = lowered.scoped_memory_anchors.as_mut() else {
            continue;
        };
        budget.charge_work(anchors.rows.len())?;
        let Some(index) = anchors
            .rows
            .iter()
            .position(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::ScalarMove { .. }))
        else {
            continue;
        };
        let row = anchors.rows[index];
        let ScopedMemoryAnchorKindV29::ScalarMove { event, local } = row.kind else {
            unreachable!()
        };
        assert_eq!(local, 4);
        assert!(matches!(
            row.source,
            Some(ScopedMemoryFrameV29 {
                site: ExecutionSiteV29::Terminator { .. },
                role: Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::AssertCondition
                )),
            })
        ));
        match fault {
            0 => {}
            1 => {
                budget.charge_work(anchors.rows.len())?;
                anchors.rows.remove(index);
            }
            2 => {
                emission_push_v1(&mut anchors.rows, row, budget)?;
                budget.charge_work(anchors.rows.len() - index - 1)?;
                anchors.rows[index + 1..].rotate_right(1);
            }
            3 => {
                anchors.rows[index].kind = ScopedMemoryAnchorKindV29::ScalarMove {
                    event: event.checked_add(1).unwrap(),
                    local,
                }
            }
            4 => {
                anchors.rows[index].kind = ScopedMemoryAnchorKindV29::ScalarMove {
                    event,
                    local: local + 1,
                }
            }
            5 => anchors.rows[index].kind = ScopedMemoryAnchorKindV29::FailureRead { event, local },
            _ => unreachable!(),
        }
        SCALAR_MOVE_MUTATIONS_V45.set(SCALAR_MOVE_MUTATIONS_V45.get() + 1);
    }
    Ok(())
}

#[test]
fn retained_scalar_moves_require_one_exact_original_common_path_occurrence() {
    struct Restore(Option<ScopedSlotObserverV29>, u8, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
            SCALAR_MOVE_FAULT_V45.set(self.1);
            SCALAR_MOVE_MUTATIONS_V45.set(self.2);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(alter_scalar_move_anchor_v45)),
        SCALAR_MOVE_FAULT_V45.get(),
        SCALAR_MOVE_MUTATIONS_V45.get(),
    );
    for fault in 0..=5 {
        SCALAR_MOVE_FAULT_V45.set(fault);
        SCALAR_MOVE_MUTATIONS_V45.set(0);
        let observed = run_scalar_range_owner_v45(
            RangeCaseV45::ConditionMove,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        assert!(
            SCALAR_MOVE_MUTATIONS_V45.get() > 0,
            "fault={fault}: {:?}",
            observed.0
        );
        assert_eq!(
            observed.0.is_ok(),
            fault == 0,
            "fault={fault}: {:?}",
            observed.0
        );
        assert_eq!(observed.3, fault == 0, "fault={fault}: {:?}", observed.0);
        if fault == 0 {
            continue;
        }
        let error = observed.0.unwrap_err();
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&error);
        let mut exact = false;
        while let Some(part) = current {
            assert!(
                part.downcast_ref::<ArgumentResourceV1>().is_none(),
                "{error:?}"
            );
            exact |= matches!(
                part.downcast_ref::<ProductionSemanticKirErrorV1>(),
                Some(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped memory anchors differ from their source instance",
                    ..
                })
            );
            current = part.source();
        }
        assert!(exact, "fault={fault}: {error:?}");
    }
}
