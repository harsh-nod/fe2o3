fn graph(name: &str, repeats: usize, space: AddressSpace) -> Function {
    let from = Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadWrite);
    let generic = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    for id in [2, 3] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), generic.clone()),
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: ValueId(0),
                to: generic.clone(),
            },
        ));
    }
    for _ in 0..repeats {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::definition(
        name,
        fe2o3_kernel_ir::Signature::new(vec![from, Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )
}

#[test]
fn issued_global_origin_batch_binds_distinct_roots_and_reuses_repeated_pointer_results() {
    for repeats in [1, 4, 32] {
        let first = graph("batch_first", repeats, AddressSpace::Global);
        let second = graph("batch_second", repeats, AddressSpace::Private);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        with_canonical_call_scratch_v1(&mut budget, |budget| {
            let before = SourceIssuedGlobalOriginsV26::prepare(&first, budget)?;
            assert_eq!(before.rows, [(ValueId(2), Some(ValueId(0)))]);
            assert_eq!(budget.storage(), before.floor + before.owned);
            assert!(
                budget.peak_storage() > budget.storage(),
                "index/CFG scratch settled"
            );
            let after = SourceIssuedGlobalOriginsV26::prepare(&second, budget)?;
            assert_eq!(after.rows, [(ValueId(2), None)]);
            assert_eq!(budget.storage(), after.floor + after.owned);
            let held = budget.storage();
            let peak = budget.peak_storage();
            let work = budget.work();
            for _ in 0..64 {
                assert_eq!(before.query(&first, ValueId(2), budget)?, Some(ValueId(0)));
                assert_eq!(after.query(&second, ValueId(2), budget)?, None);
            }
            assert_eq!(budget.work() - work, 128 * (2 + 2 * 16));
            assert_eq!((budget.storage(), budget.peak_storage()), (held, peak));
            drop((after, before));
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn issued_global_origin_batch_refuses_foreign_function_ledger_and_uncensused_pointer() {
    let function = graph("bound", 3, AddressSpace::Global);
    let foreign = function.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let origins = SourceIssuedGlobalOriginsV26::prepare(&function, budget)?;
        let held = budget.storage();
        let peak = budget.peak_storage();
        assert!(matches!(
            origins.query(&foreign, ValueId(2), budget),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        // Value 3 has the same valid Global cast but no actual memory use. A
        // missing request must refuse, never launch an unbounded lazy rebuild.
        assert!(matches!(
            origins.query(&function, ValueId(3), budget),
            Err(ProductionSemanticKirErrorV1::Unsupported { .. })
        ));
        assert_eq!((budget.storage(), budget.peak_storage()), (held, peak));
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign_budget = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        foreign_budget.reserve_storage(held).unwrap();
        assert!(matches!(
            origins.query(&function, ValueId(2), &mut foreign_budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((foreign_budget.work(), foreign_budget.storage()), (0, held));
        assert_eq!(
            origins.query(&function, ValueId(2), budget)?,
            Some(ValueId(0))
        );
        drop(origins);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 17);
}

fn run(
    repeats: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let function = graph("limits", repeats, AddressSpace::Global);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(17).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let origins = SourceIssuedGlobalOriginsV26::prepare(&function, budget)?;
        assert_eq!(budget.storage(), origins.floor + origins.owned);
        assert_eq!(
            origins.query(&function, ValueId(2), budget)?,
            Some(ValueId(0))
        );
        drop(origins);
        Ok(())
    });
    assert_eq!(budget.storage(), 17, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn issued_global_origin_batch_has_exact_and_one_short_work_and_storage_limits() {
    for repeats in [1, 32] {
        let (result, work, peak) = run(repeats, usize::MAX, usize::MAX);
        result.unwrap();
        let (exact, exact_work, exact_peak) = run(repeats, work, peak);
        exact.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        assert!(matches!(
            run(repeats, work - 1, peak).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
        assert!(matches!(
            run(repeats, work, peak - 1).0,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ));
    }
}

#[test]
fn issued_global_origin_batch_header_has_independent_live_map_equation() {
    #[allow(dead_code)]
    struct MapFrame<'a> {
        function: &'a Function,
        rows: Vec<(ValueId, Option<ValueId>)>,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        floor: usize,
        owned: usize,
    }
    let expected = std::mem::size_of::<(
        MapFrame<'_>,
        Result<MapFrame<'_>, ProductionSemanticKirErrorV1>,
        [usize; 8],
        Option<ValueId>,
    )>();
    assert_eq!(SourceIssuedGlobalOriginsV26::headers(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 17 + expected - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(SourceIssuedGlobalOriginsV26::headers());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 17 + expected && error.limit() == 16 + expected)
            );
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!((budget.work(), budget.storage()), (0, 17));
    }
}
