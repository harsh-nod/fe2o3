use fe2o3_kernel_ir::*;

const FLOOR: usize = 23;
const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 8,
    edges: 16,
    containment_depth: 16,
    object_bytes: 1024,
};

fn fixture(count: u32) -> Module {
    let concrete = Type::slice(Type::F32, AddressSpace::Private, AccessMode::ReadWrite);
    let generic = Type::slice(Type::F32, AddressSpace::Generic, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = (1..=count)
        .map(|id| {
            Operation::effect_free(
                ValueDef::new(ValueId(id), generic.clone()),
                OperationKind::Cast {
                    kind: CastKind::SliceToGeneric,
                    value: ValueId(0),
                    to: generic.clone(),
                },
            )
        })
        .collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("slice-resources");
    module.functions.push(Function::internal_helper(
        "expose",
        Signature::new(vec![concrete], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

struct Run {
    result: Result<
        (
            VerifiedCanonicalKernelIrModuleV18,
            CanonicalKernelIrReplayStorageV18,
        ),
        CanonicalKernelIrReplayAdmissionErrorV18,
    >,
    work: usize,
    peak: usize,
    denied_work: bool,
    denied_storage: bool,
}

fn run(module: &Module, work_limit: usize, storage_limit: usize) -> Run {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module,
        LIMITS,
        &mut budget,
    );
    assert_eq!(budget.storage(), FLOOR);
    let used = budget.work();
    let peak = budget.peak_storage();
    let denied_storage = budget.failed_storage().is_some();
    Run {
        result,
        work: used,
        peak,
        denied_work: work.failed_work().is_some(),
        denied_storage,
    }
}

#[test]
fn exposure_admission_has_exact_work_and_peak_boundaries_without_storage_leak() {
    let module = fixture(9);
    let observed = run(&module, 100_000_000, 100_000_000);
    let (owner, receipt) = observed.result.unwrap();
    assert!(!observed.denied_work && !observed.denied_storage);
    assert!(
        receipt.retained_storage()
            >= std::mem::size_of::<VerifiedCanonicalKernelIrModuleV18>()
                + owner.canonical_bytes().len()
                + 9 * (std::mem::size_of::<Operation>() + std::mem::size_of::<ValueDef>())
    );
    let exact = run(&module, observed.work, observed.peak);
    let (exact_owner, exact_receipt) = exact.result.unwrap();
    assert_eq!(exact_owner.canonical_bytes(), owner.canonical_bytes());
    assert_eq!(exact_receipt.retained_storage(), receipt.retained_storage());
    assert_eq!(exact.work, observed.work);
    assert_eq!(exact.peak, observed.peak);
    let short_work = run(&module, observed.work - 1, observed.peak);
    assert!(short_work.result.is_err());
    assert!(short_work.denied_work);
    let short_storage = run(&module, observed.work, observed.peak - 1);
    assert!(short_storage.result.is_err());
    assert!(short_storage.denied_storage);
    assert_eq!(owner.module(), &module);
}

#[test]
fn bounded_copy_and_replay_preserve_the_live_input_and_cast_kind() {
    let module = fixture(3);
    let (owner, receipt) = run(&module, 100_000_000, 100_000_000).result.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
    budget
        .reserve_storage(FLOOR + receipt.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (mut copy, copy_storage) = owner
        .copy_module_for_transformation_v18(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(copy_storage.retained_storage())
        .unwrap();
    assert!(
        owner
            .matches_module_with_budget_v18(&copy, &mut budget)
            .unwrap()
    );
    let OperationKind::Cast { kind, .. } =
        &mut copy.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *kind = CastKind::Bitcast;
    assert!(
        !owner
            .matches_module_with_budget_v18(&copy, &mut budget)
            .unwrap()
    );
    assert_eq!(owner.module(), &module);
    drop(copy);
    budget
        .release_storage(copy_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn guarded_effect_query_meters_exposure_lineage_and_restores_exact_floor() {
    let mut module = fixture(9);
    let function = &mut module.functions[0];
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    for pointer in 1..=9 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(20 + pointer),
                Type::pointer(Type::F32, AddressSpace::Generic, AccessMode::ReadWrite),
            ),
            OperationKind::SliceData {
                slice: ValueId(pointer),
            },
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(40 + pointer), Type::F32),
            OperationKind::Load {
                pointer: ValueId(20 + pointer),
                access: MemoryAccess::new(AddressSpace::Generic, 4),
            },
        ));
    }
    let (owner, receipt) = run(&module, 100_000_000, 100_000_000).result.unwrap();
    let floor = FLOOR + receipt.retained_storage();
    let query = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = with_canonical_guarded_global_reads_v18(
            &owner,
            Default::default(),
            &mut budget,
            |view, budget| {
                // Proven Private roots are not Global effects; unresolved Generic
                // roots have separate conservative positive/negative tests.
                assert_eq!(
                    view.function_effects(CanonicalKirFunctionCoordinateV1(0), budget)?,
                    (0, 0, 0)
                );
                Ok(())
            },
        );
        assert_eq!(budget.storage(), floor);
        let used = budget.work();
        let peak = budget.peak_storage();
        let denied_storage = budget.failed_storage().is_some();
        (
            result.is_ok(),
            used,
            peak,
            work.failed_work().is_some(),
            denied_storage,
        )
    };
    let baseline = query(100_000_000, 100_000_000);
    assert!(baseline.0);
    assert!(baseline.1 >= 9 * 4);
    assert!(baseline.2 > floor);
    let exact = query(baseline.1, baseline.2);
    assert!(exact.0);
    assert_eq!((exact.1, exact.2), (baseline.1, baseline.2));
    let short_work = query(baseline.1 - 1, baseline.2);
    assert!(!short_work.0 && short_work.3);
    let short_storage = query(baseline.1, baseline.2 - 1);
    assert!(!short_storage.0 && short_storage.4);
}

#[test]
fn slice_wire_comparison_has_an_independently_counted_exact_work_schedule() {
    // These are schema widths/chunks, not observed work totals. No table,
    // result signature, block parameters, capabilities or kernel rows exist.
    const PRIOR: usize = 7;
    for count in [1_u32, 9] {
        let source = fixture(count);
        let (owner, receipt) = run(&source, 100_000_000, 100_000_000).result.unwrap();
        let module_bytes = 20 + 4 + "slice-resources".len() + 4 * 4;
        let function_bytes =
            4 + "expose".len() + 1 + 4 + 5 + 4 + 1 + 4 + 4 + 4 + 4 + 4 + 4 + 1 + 1 + 4 + 4;
        // Cast row: result count, value ID, five-byte slice type, op+cast
        // tags, operand ID, and destination slice type.
        let cast_bytes = 4 + 4 + 5 + 1 + 1 + 4 + 5;
        let bytes = module_bytes + function_bytes + count as usize * cast_bytes;
        let module_chunks = 5 + 2 + 4;
        let function_chunks = 2 + 1 + 1 + 5 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1;
        let cast_chunks = 1 + 1 + 5 + 1 + 1 + 1 + 5;
        let chunks = module_chunks + function_chunks + count as usize * cast_chunks;
        // One scope entry; one counting pass; compared payload bytes and
        // chunks (except placeholder); patch length bytes/chunk; completion.
        let exact = 1 + chunks + bytes + (chunks - 1) + 4 + 1 + 1;
        assert_eq!(owner.canonical_bytes().len(), bytes);
        for short in [false, true] {
            for prior_denial in [false, true] {
                let mut work =
                    CanonicalKernelIrWorkBudgetV1::new(PRIOR + exact - usize::from(short));
                work.charge_work(PRIOR).unwrap();
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
                let floor = FLOOR + receipt.retained_storage();
                budget.reserve_storage(floor).unwrap();
                if prior_denial {
                    assert!(budget.charge_work(exact + 31).is_err());
                }
                let result = owner.matches_module_with_budget_v18(&source, &mut budget);
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.work(), PRIOR + exact - usize::from(short));
                if short {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), true);
                }
                assert_eq!(
                    work.failed_work(),
                    if prior_denial {
                        Some(PRIOR + exact + 31)
                    } else if short {
                        Some(PRIOR + exact)
                    } else {
                        None
                    }
                );
            }
        }
    }
}

#[test]
fn guarded_cast_queries_preserve_selected_failure_panic_and_paid_owner_floor() {
    let module = fixture(3);
    let (owner, receipt) = run(&module, 100_000_000, 100_000_000).result.unwrap();
    let invalid = CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(0),
            block: 99,
        },
        operation: 0,
    };
    for mode in 0..5 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000_000);
        let floor = FLOOR + receipt.retained_storage();
        budget.reserve_storage(floor).unwrap();
        let mut entered = false;
        let result: Result<(), CanonicalGuardedGlobalReadErrorV1> =
            with_canonical_guarded_global_reads_v18(
                &owner,
                Default::default(),
                &mut budget,
                |view, budget| {
                    entered = true;
                    assert_eq!(view.function_count(budget)?, 1);
                    if matches!(mode, 2 | 4) {
                        assert!(matches!(
                            view.read_at(invalid, budget),
                            Err(CanonicalGuardedGlobalReadErrorV1::Coordinate(actual))
                                if actual == invalid
                        ));
                        let used = budget.work();
                        assert!(matches!(
                            view.function_count(budget),
                            Err(CanonicalGuardedGlobalReadErrorV1::Coordinate(actual))
                                if actual == invalid
                        ));
                        assert_eq!(budget.work(), used);
                    }
                    if matches!(mode, 3 | 4) {
                        std::panic::panic_any(1237_usize);
                    }
                    if mode == 1 {
                        return Err(CanonicalGuardedGlobalReadErrorV1::Coordinate(invalid));
                    }
                    Ok(())
                },
            );
        assert!(entered);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
        match mode {
            0 => result.unwrap(),
            3 => assert_eq!(result, Err(CanonicalGuardedGlobalReadErrorV1::Panicked)),
            _ => assert_eq!(
                result,
                Err(CanonicalGuardedGlobalReadErrorV1::Coordinate(invalid))
            ),
        }
        assert_eq!(owner.module(), &module);
    }
}
