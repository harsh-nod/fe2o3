use super::*;

fn cast(id: u32, value: u32, to: Type) -> Operation {
    op(
        id,
        to.clone(),
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(value),
            to,
        },
    )
}

fn cast_fixture() -> Module {
    let mut module = fixture(false);
    let function = &mut module.functions[0];
    function.signature.parameters[1] = Type::Scalar(ScalarType::U64);
    function.signature.parameters[2] = Type::Scalar(ScalarType::U64);
    function
        .signature
        .parameters
        .push(function.signature.parameters[0].clone());
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(30));
    body.blocks[0].operations = vec![
        op(
            4,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            13,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        cast(40, 1, Type::INDEX),
        cast(41, 4, Type::Scalar(ScalarType::U64)),
        cast(42, 41, Type::INDEX),
        cast(43, 1, Type::INDEX),
        cast(44, 43, Type::Scalar(ScalarType::U64)),
        cast(45, 44, Type::INDEX),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(40),
                rhs: ValueId(42),
            },
        ),
    ];
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let OperationKind::GetElementPointer { offset, .. } = &mut body.blocks[1].operations[1].kind
    else {
        unreachable!();
    };
    *offset = ValueId(45);
    module
}

fn replace_cast_source(module: &mut Module, result: u32, source: u32) {
    let operation = module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|op| op.results.first().is_some_and(|r| r.id == ValueId(result)))
        .unwrap();
    let OperationKind::Cast { value, .. } = &mut operation.kind else {
        unreachable!();
    };
    *value = ValueId(source);
}

fn assert_unproved(module: &Module, at: Coordinate) {
    let (graph, credit) = owner(module);
    run(&graph, credit, |view, budget| {
        assert!(matches!(
            view.read_at(at, budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::NotProved(
                CanonicalGuardedGlobalReadReasonV1::MissingBoundOrProvenance
            )
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn normalized_read_origins_keep_direct_and_multistep_transport_coordinates_distinct() {
    for (module, index, compare, origin) in [
        (fixture(false), 5, (5, 4), 5),
        (cast_fixture(), 45, (40, 42), 1),
    ] {
        let (graph, credit) = owner(&module);
        run(&graph, credit, |view, budget| {
            let storage = budget.storage();
            let mut previous_cost = None;
            for _ in 0..4 {
                let before = budget.work();
                let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
                    view.read_at(coordinate(0, 1, 2), budget)?
                else {
                    panic!("actual guarded read");
                };
                assert!(std::ptr::eq(read.owner(), &graph));
                assert_eq!(read.operation(), coordinate(0, 1, 2));
                assert_eq!(read.domain().index(), ValueId(index));
                assert_eq!(
                    read.comparison_operands(),
                    (ValueId(compare.0), ValueId(compare.1))
                );
                assert_eq!(
                    read.normalized_index_origin(),
                    CanonicalGuardedReadIndexOriginV1::ProvenOrigin(ValueId(origin))
                );
                assert_eq!(read.normalized_length_origin(), ValueId(4));
                assert!(read.requires_runtime_allocation_binding());
                let cost = budget.work() - before;
                assert!(previous_cost.is_none_or(|previous| previous == cost));
                previous_cost = Some(cost);
                assert_eq!(budget.storage(), storage);
            }
            let predicate = view
                .true_at(coordinate(0, 1, 2), ValueId(7), budget)?
                .unwrap();
            assert_eq!(predicate.edge(), (BlockId(10), 0, BlockId(20)));
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn normalized_read_origins_reject_same_typed_foreign_index_and_replaced_length_root() {
    let (graph, credit) = owner(&cast_fixture());
    run(&graph, credit, |view, budget| {
        assert!(matches!(
            view.read_at(coordinate(0, 1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(_)
        ));
        Ok(())
    })
    .unwrap();
    let mut wrong_index = cast_fixture();
    replace_cast_source(&mut wrong_index, 43, 2);
    assert_unproved(&wrong_index, coordinate(0, 1, 2));

    let mut wrong_length = cast_fixture();
    replace_cast_source(&mut wrong_length, 42, 2);
    assert_unproved(&wrong_length, coordinate(0, 1, 2));

    let mut wrong_slice = cast_fixture();
    wrong_slice.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::SliceLength { slice: ValueId(30) };
    assert_unproved(&wrong_slice, coordinate(0, 1, 2));
}

#[test]
fn normalized_read_length_retains_the_exact_same_root_slice_length_occurrence() {
    let mut module = cast_fixture();
    replace_cast_source(&mut module, 41, 13);
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("actual guarded read");
        };
        assert_eq!(read.domain().slice(), ValueId(0));
        assert_eq!(read.comparison_operands(), (ValueId(40), ValueId(42)));
        assert_eq!(read.normalized_length_origin(), ValueId(13));
        // An independently bounded same-root read is valid, but cannot join
        // a source occurrence which specifically requires the former result.
        assert_ne!(read.normalized_length_origin(), ValueId(4));
        Ok(())
    })
    .unwrap();
}

fn carried_index_fixture() -> Module {
    let mut module = cast_fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let mut moved = Vec::new();
    body.blocks[0].operations.retain(|operation| {
        if operation
            .results
            .first()
            .is_some_and(|result| [43, 44, 45].contains(&result.id.0))
        {
            moved.push(operation.clone());
            false
        } else {
            true
        }
    });
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) = &mut body.blocks[0].terminator
    else {
        unreachable!();
    };
    then_arguments.push(ValueId(1));
    body.blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(50), Type::Scalar(ScalarType::U64)));
    body.blocks[1].operations.splice(0..0, moved);
    replace_cast_source(&mut module, 43, 50);
    module
}

#[test]
fn normalized_read_origins_follow_genuine_unique_block_parameter_transport() {
    let mut module = carried_index_fixture();
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 5), budget)?
        else {
            panic!("carried guarded read");
        };
        assert_eq!(read.domain().index(), ValueId(45));
        assert_eq!(
            read.normalized_index_origin(),
            CanonicalGuardedReadIndexOriginV1::ProvenOrigin(ValueId(1))
        );
        assert_eq!(read.normalized_length_origin(), ValueId(4));
        Ok(())
    })
    .unwrap();
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!();
    };
    then_arguments[0] = ValueId(2);
    assert_unproved(&module, coordinate(0, 1, 5));
}

fn carried_length_fixture() -> Module {
    let mut module = cast_fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.retain(|operation| {
        !operation
            .results
            .first()
            .is_some_and(|result| [42, 7].contains(&result.id.0))
    });
    let old_terminator = body.blocks[0].terminator.take();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![ValueId(41)],
    });
    let mut guard = BasicBlock::new(BlockId(15));
    guard
        .parameters
        .push(ValueDef::new(ValueId(50), Type::Scalar(ScalarType::U64)));
    guard.operations = vec![
        cast(42, 50, Type::INDEX),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(40),
                rhs: ValueId(42),
            },
        ),
    ];
    guard.terminator = old_terminator;
    body.blocks.push(guard);
    module
}

#[test]
fn normalized_read_length_follows_only_unique_incoming_representation_origin() {
    let mut module = carried_length_fixture();
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("carried length");
        };
        assert_eq!(read.normalized_length_origin(), ValueId(4));
        assert_eq!(read.comparison_operands(), (ValueId(40), ValueId(42)));
        assert_eq!(
            read.domain().path(),
            FormalGuardedPathV1::TrueEdge {
                source: BlockId(15),
                ordinal: 0,
                target: BlockId(20),
            }
        );
        Ok(())
    })
    .unwrap();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(15),
            then_arguments: vec![ValueId(41)],
            else_target: BlockId(15),
            else_arguments: vec![ValueId(2)],
        });
    assert_unproved(&module, coordinate(0, 1, 2));
}

#[test]
fn normalized_read_origins_distinguish_exact_block_parameter_from_proven_incoming_origin() {
    let mut module = cast_fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.pop();
    body.blocks[0].operations.push(cast(46, 2, Type::INDEX));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(15),
        then_arguments: vec![ValueId(40)],
        else_target: BlockId(15),
        else_arguments: vec![ValueId(46)],
    });
    let mut guard = BasicBlock::new(BlockId(15));
    guard
        .parameters
        .push(ValueDef::new(ValueId(50), Type::INDEX));
    guard.operations.push(op(
        7,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(50),
            rhs: ValueId(42),
        },
    ));
    guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(guard);
    let OperationKind::GetElementPointer { offset, .. } = &mut body.blocks[1].operations[1].kind
    else {
        unreachable!();
    };
    *offset = ValueId(50);
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("exact guarded parameter");
        };
        assert_eq!(
            read.normalized_index_origin(),
            CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(ValueId(50))
        );
        assert_eq!(read.normalized_length_origin(), ValueId(4));
        assert_eq!(read.domain().index(), ValueId(50));
        assert_eq!(read.comparison_operands(), (ValueId(50), ValueId(42)));
        Ok(())
    })
    .unwrap();
}

#[test]
fn normalized_read_origins_do_not_admit_raw_u64_comparison_operands() {
    let mut module = cast_fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .last_mut()
        .unwrap()
        .kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(1),
        rhs: ValueId(41),
    };
    assert_unproved(&module, coordinate(0, 1, 2));
}

#[test]
fn normalized_read_origins_remain_bound_to_the_exact_borrowed_owner() {
    let module = cast_fixture();
    let (graph, credit) = owner(&module);
    let (foreign, foreign_credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("guarded read");
        };
        run(&foreign, foreign_credit, |other, other_budget| {
            let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(other_read) =
                other.read_at(coordinate(0, 1, 2), other_budget)?
            else {
                panic!("foreign read");
            };
            assert_eq!(
                read.normalized_index_origin(),
                other_read.normalized_index_origin()
            );
            assert_eq!(
                read.normalized_length_origin(),
                other_read.normalized_length_origin()
            );
            assert_eq!(read.operation(), other_read.operation());
            assert!(std::ptr::eq(read.owner(), &graph));
            assert!(std::ptr::eq(other_read.owner(), &foreign));
            assert!(!std::ptr::eq(read.owner(), other_read.owner()));
            Ok(())
        })?;
        Ok(())
    })
    .unwrap();
}

fn measured_origins(
    graph: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        graph,
        Default::default(),
        &mut budget,
        |view, budget| {
            let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
                view.read_at(coordinate(0, 1, 2), budget)?
            else {
                panic!("guarded read");
            };
            assert_eq!(read.normalized_length_origin(), ValueId(4));
            assert_eq!(
                read.normalized_index_origin(),
                CanonicalGuardedReadIndexOriginV1::ProvenOrigin(ValueId(1))
            );
            Ok(())
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn normalized_read_origins_whole_transaction_exact_and_one_short_work_and_storage() {
    let (graph, credit) = owner(&cast_fixture());
    let (result, work, peak) = measured_origins(&graph, credit, 100_000_000, 100_000_000);
    result.unwrap();
    measured_origins(&graph, credit, work, peak).0.unwrap();
    assert!(matches!(
        measured_origins(&graph, credit, work - 1, peak).0,
        Err(Failure::Resource(ResourceError::Work(_)))
    ));
    assert!(matches!(
        measured_origins(&graph, credit, work, peak - 1).0,
        Err(Failure::Resource(ResourceError::Storage { .. }))
    ));
}

#[test]
fn normalized_read_origins_independent_collector_frame_exact_and_one_short() {
    let expected = size_of::<ReadRow>()
        + size_of::<(
            Option<RuntimeSliceReadConditionsV1>,
            CanonicalGuardedGlobalReadReasonV1,
        )>();
    assert_eq!(read_row_frame_bytes().unwrap(), expected);
    assert_eq!(
        size_of::<CanonicalGuardedGlobalReadFactV18<'_, '_>>(),
        3 * size_of::<usize>()
    );
    for allowed in [expected - 1, expected] {
        let module = fixture(false);
        let function = &module.functions[0];
        let mut result = FunctionFacts {
            function,
            controls: vec![],
            predicates: vec![],
            reads: vec![],
            global_read_occurrences: 0,
            other_global_effects: 0,
            unresolved_calls: 0,
        };
        let mut body = function.body.as_ref().unwrap().clone();
        body.blocks.clear();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = Budget::new(&mut work, 17 + allowed);
        budget.reserve_storage(17).unwrap();
        let actual = {
            let mut meter = LiveGuardMeter::new(&mut budget, 100, expected, 1);
            collect_effects_without_reads(&mut result, FunctionCoordinate(0), &body, &mut meter)
        };
        if allowed == expected {
            actual.unwrap();
            assert_eq!(budget.storage(), 17 + expected);
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(
                actual,
                Err(Failure::Resource(ResourceError::Storage { .. }))
            ));
            assert_eq!(budget.failed_storage(), Some(17 + expected));
            assert_eq!(budget.peak_storage(), 17);
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
        assert!(result.reads.is_empty());
    }
}

#[test]
fn normalized_read_origins_query_work_denial_stays_first_after_retry_and_unwind() {
    let (graph, credit) = owner(&cast_fixture());
    let mut constructor_work = 0;
    let mut query_work = 0;
    run(&graph, credit, |view, budget| {
        constructor_work = budget.work();
        assert!(matches!(
            view.read_at(coordinate(0, 1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(_)
        ));
        query_work = budget.work() - constructor_work;
        Ok(())
    })
    .unwrap();
    assert!(query_work > 1);
    for unwind in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(constructor_work + query_work - 1);
        let mut budget = Budget::new(&mut work, 100_000_000);
        budget.reserve_storage(credit + 17).unwrap();
        let mut first = None;
        let result: Result<()> = with_canonical_guarded_global_reads_v18(
            &graph,
            Default::default(),
            &mut budget,
            |view, budget| {
                assert_eq!(budget.work(), constructor_work);
                let Err(error) = view.read_at(coordinate(0, 1, 2), budget) else {
                    panic!("one-short query was accepted");
                };
                assert!(matches!(error, Failure::Resource(ResourceError::Work(_))));
                let work = budget.work();
                assert!(matches!(view.owner(budget), Err(ref repeated) if *repeated == error));
                assert_eq!(budget.work(), work);
                first = Some(error);
                if unwind {
                    panic!("after authentic work denial");
                }
                Ok(())
            },
        );
        assert_eq!(result, Err(first.unwrap()));
        assert_eq!(budget.storage(), credit + 17);
    }
}

#[test]
fn normalized_read_origins_foreign_ledger_cannot_refresh_a_paid_fact() {
    let (graph, credit) = owner(&cast_fixture());
    let result = run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("genuine positive before foreign query");
        };
        assert_eq!(read.normalized_length_origin(), ValueId(4));
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut other = Budget::new(&mut other_work, 100_000_000);
        other.reserve_storage(budget.storage() + 1).unwrap();
        assert!(matches!(
            view.read_at(coordinate(0, 1, 2), &mut other),
            Err(Failure::Resource(ResourceError::Accounting))
        ));
        let before = budget.work();
        assert!(matches!(
            view.read_at(coordinate(0, 1, 2), budget),
            Err(Failure::Resource(ResourceError::Accounting))
        ));
        assert_eq!(budget.work(), before);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
}

#[test]
fn normalized_read_origin_getter_bundle_is_paid_by_the_outer_scope() {
    let getters = size_of::<&CanonicalGuardedGlobalReadFactV18<'_, '_>>()
        + size_of::<CanonicalGuardedReadIndexOriginV1>()
        + size_of::<ValueId>()
        + size_of::<(ValueId, ValueId)>();
    assert_eq!(
        read_origin_query_frame_bytes::<VerifiedCanonicalKernelIrModuleV18>().unwrap(),
        getters
    );
    let headers = size_of::<Facts<'_, VerifiedCanonicalKernelIrModuleV18>>()
        + size_of::<Accounting>()
        + size_of::<CheckedCanonicalGuardedGlobalReadsV18<'_, '_>>()
        + 2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>()
        + getters;
    let (graph, credit) = owner(&cast_fixture());
    for allowed in [headers - 1, headers] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = Budget::new(&mut work, credit + 17 + allowed);
        budget.reserve_storage(credit + 17).unwrap();
        let mut callbacks = 0;
        let result: Result<()> = with_canonical_guarded_global_reads_v18(
            &graph,
            Default::default(),
            &mut budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
        assert_eq!(callbacks, 0);
        assert_eq!(budget.storage(), credit + 17);
        if allowed == headers {
            assert_eq!(budget.work(), 1);
            assert_eq!(budget.peak_storage(), credit + 17 + headers);
            assert_eq!(
                budget.failed_storage(),
                Some(credit + 17 + headers + size_of::<FunctionFacts<'_>>())
            );
        } else {
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.peak_storage(), credit + 17);
            assert_eq!(budget.failed_storage(), Some(credit + 17 + headers));
        }
    }
    run(&graph, credit, |view, budget| {
        assert!(budget.storage() >= credit + 17 + headers);
        let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
            view.read_at(coordinate(0, 1, 2), budget)?
        else {
            panic!("guarded read");
        };
        assert_eq!(read.normalized_length_origin(), ValueId(4));
        Ok(())
    })
    .unwrap();
}
