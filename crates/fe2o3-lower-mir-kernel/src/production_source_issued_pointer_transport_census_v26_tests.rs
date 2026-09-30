fn queries(fault: u8) -> [SourceIssuedPointerTransportV26; 4] {
    [
        SourceIssuedPointerTransportV26 {
            pointer: ValueId(5),
            issuer: ValueId(0),
        },
        SourceIssuedPointerTransportV26 {
            pointer: ValueId(if fault == 13 { 4 } else { 7 }),
            issuer: ValueId(if fault == 12 { 1 } else { 0 }),
        },
        SourceIssuedPointerTransportV26 {
            pointer: ValueId(5),
            issuer: ValueId(0),
        },
        SourceIssuedPointerTransportV26 {
            pointer: ValueId(0),
            issuer: ValueId(0),
        },
    ]
}

fn run(
    function: &Function,
    rows: &[SourceIssuedPointerTransportV26],
    work: usize,
    storage: usize,
) -> (
    Result<[Option<ValueId>; 4], ProductionSemanticKirErrorV1>,
    usize,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage);
    budget.reserve_storage(17).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let actual = SourceIssuedActualV29::from_function(function, budget)?;
        let floor = budget.storage();
        let census = source_issued_pointer_transport_census_v26(function, &actual, rows, budget)?;
        let credit = std::mem::size_of::<Vec<ValueId>>()
            + std::mem::align_of::<Vec<ValueId>>()
            + std::mem::size_of::<Result<Vec<ValueId>, ProductionSemanticKirErrorV1>>()
            + census.capacity() * std::mem::size_of::<ValueId>();
        assert_eq!(
            budget.storage(),
            floor + credit,
            "returned backing remains paid after CFG/bitmap disposal"
        );
        assert!(census.len() <= 4);
        let mut output = [None; 4];
        for (slot, value) in output.iter_mut().zip(&census) {
            *slot = Some(*value);
        }
        drop(census);
        budget.release_storage(credit)?;
        assert_eq!(budget.storage(), floor);
        Ok(output)
    });
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn issued_transport_census_deduplicates_complete_paths_and_excludes_unrelated_casts() {
    for function in [transport_graph(0), transport_graph(0)] {
        let (result, _, held, _) = run(&function, &queries(0), usize::MAX, usize::MAX);
        assert_eq!(
            result.unwrap(),
            [Some(ValueId(3)), Some(ValueId(6)), None, None]
        );
        assert_eq!(held, 17);
    }
    let (empty, _, held, _) = run(&transport_graph(0), &[], usize::MAX, usize::MAX);
    assert_eq!(empty.unwrap(), [None; 4]);
    assert_eq!(held, 17);
}

#[test]
fn issued_transport_census_refuses_partial_foreign_and_same_count_paths() {
    for fault in 1..=15 {
        let (result, _, held, _) = run(
            &transport_graph(fault),
            &queries(fault),
            usize::MAX,
            usize::MAX,
        );
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "fault={fault}: {result:?}"
        );
        assert_eq!(held, 17);
    }
    let function = transport_graph(0);
    let foreign = function.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let actual = SourceIssuedActualV29::from_function(&function, budget)?;
        source_issued_pointer_transport_census_v26(&foreign, &actual, &queries(0), budget)
            .map(|_| ())
    });
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
    assert_eq!(budget.storage(), 17);
}

#[test]
fn issued_transport_census_preserves_both_cast_orders_and_exact_restricted_issuer_stop() {
    let mut function = transport_graph(0);
    let global_read = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let generic_read = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let blocks = &mut function.body.as_mut().unwrap().blocks;
    blocks[0].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(3), global_read.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(0),
            to: global_read.clone(),
        },
    );
    blocks[1].parameters[0].ty = global_read;
    blocks[1].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(6), generic_read.clone()),
        OperationKind::Cast {
            kind: CastKind::PointerToGeneric,
            value: ValueId(5),
            to: generic_read,
        },
    );
    let full = [SourceIssuedPointerTransportV26 {
        pointer: ValueId(7),
        issuer: ValueId(0),
    }];
    assert_eq!(
        run(&function, &full, usize::MAX, usize::MAX).0.unwrap(),
        [Some(ValueId(3)), Some(ValueId(6)), None, None]
    );
    let restricted = [SourceIssuedPointerTransportV26 {
        pointer: ValueId(7),
        issuer: ValueId(3),
    }];
    assert_eq!(
        run(&function, &restricted, usize::MAX, usize::MAX)
            .0
            .unwrap(),
        [Some(ValueId(6)), None, None, None]
    );
}

#[test]
fn issued_transport_census_has_exact_and_one_short_work_and_storage_with_paid_output() {
    let function = transport_graph(0);
    let rows = queries(0);
    let (result, work, held, peak) = run(&function, &rows, usize::MAX, usize::MAX);
    let expected = result.unwrap();
    let (exact, exact_work, exact_held, exact_peak) = run(&function, &rows, work, peak);
    assert_eq!(exact.unwrap(), expected);
    assert_eq!((exact_work, exact_held, exact_peak), (work, held, peak));
    let (short, _, held, _) = run(&function, &rows, work - 1, peak);
    assert!(
        matches!(short, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error))) if error.limit() == work - 1 && error.actual() > error.limit())
    );
    assert_eq!(held, 17);
    let (short, _, held, _) = run(&function, &rows, work, peak - 1);
    assert!(
        matches!(short, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error))) if error.limit() == peak - 1 && error.actual() > error.limit())
    );
    assert_eq!(held, 17);
}
