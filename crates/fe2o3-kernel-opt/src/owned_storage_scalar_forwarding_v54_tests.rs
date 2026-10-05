use super::*;
use fe2o3_kernel_ir::{
    StorageFieldV1, StorageLayoutIdV1 as LayoutId, StorageLayoutLimitsV1, StorageLayoutV1,
    StorageProjectionV1, VerifiedCanonicalKernelIrModuleV18 as Owner18,
};

const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 2,
    edges: 1,
    containment_depth: 2,
    object_bytes: 16,
};
fn storage_fixture(scalar: ScalarType) -> Module {
    let mut module = fixture(scalar);
    let bytes = u32::from(scalar.bit_width().unwrap() / 8);
    module.storage_layouts.push(StorageLayoutV1 {
        size: u64::from(bytes),
        alignment: bytes,
        kind: StorageLayoutKindV1::Scalar(scalar),
    });
    let initializing = blocks(&mut module)[0].operations.remove(1);
    blocks(&mut module)[1].operations.push(initializing.clone());
    blocks(&mut module)[2].operations.push(initializing);
    for block in blocks(&mut module) {
        for operation in &mut block.operations {
            operation.kind = match operation.kind {
                Kind::Alloca {
                    count,
                    address_space,
                    alignment,
                    ..
                } => {
                    let element = Type::StorageObject(LayoutId(0));
                    operation.results[0].ty = Type::pointer(
                        element.clone(),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    );
                    Kind::Alloca {
                        element,
                        count,
                        address_space,
                        alignment,
                    }
                }
                Kind::Store {
                    pointer,
                    value,
                    access,
                } => Kind::Storage(Storage::WriteValue {
                    address: pointer,
                    value,
                    access,
                }),
                Kind::Load { pointer, .. } => Kind::Storage(Storage::ReadValue {
                    address: pointer,
                    access: MemoryAccess::new(AddressSpace::Private, 1),
                }),
                _ => panic!("closed scalar fixture"),
            }
        }
    }
    module
}
fn admit_storage(module: &Module) -> (Owner18, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, size) =
        Owner18::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget)
            .unwrap();
    (owner, size.retained_storage())
}
fn run_storage(
    input: &Owner18,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<OwnedCrossBlockForwardingV18>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result =
        prepare_owned_cross_block_forwarding_v18(input, Limits::default(), LAYOUTS, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let used = budget.work();
    let peak = budget.peak_storage();
    let failed = budget.failed_storage();
    drop(budget);
    (result, used, peak, work.failed_work(), failed)
}
fn expect_count(module: Module, count: usize) {
    let (input, floor) = admit_storage(&module);
    let result = run_storage(&input, floor, WORK, STORAGE).0.unwrap();
    assert_eq!(
        result
            .origins()
            .iter()
            .filter(|row| row.store.is_some())
            .count(),
        count
    );
    assert_eq!(
        result.output().module().storage_layouts,
        module.storage_layouts
    );
    assert_eq!(input.module(), &module);
    if count == 0 {
        assert_eq!(result.output().canonical_bytes(), input.canonical_bytes())
    }
}

#[test]
fn storage_scalar_forwarding_all_admitted_widths_replays_and_then_is_noop() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        let module = storage_fixture(scalar);
        let (input, floor) = admit_storage(&module);
        let output = run_storage(&input, floor, WORK, STORAGE).0.unwrap();
        let selected: Vec<_> = output
            .origins()
            .iter()
            .filter(|row| row.store.is_some())
            .collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].input, selected[0].output);
        assert_eq!(
            (selected[0].input.block.block, selected[0].input.operation),
            (3, 0)
        );
        assert_eq!(
            output.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[3]
                .operations[0]
                .kind,
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(0),
                rhs: ValueId(0)
            }
        );
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget
            .reserve_storage(floor + output.retained_storage())
            .unwrap();
        let (pair, size) = output.replay_against(&input, &mut budget).unwrap();
        budget.reserve_storage(size.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), &input));
        assert!(!pair.grants_authority());
        drop(pair);
        budget.release_storage(size.retained_storage()).unwrap();
        let second = prepare_owned_cross_block_forwarding_v18(
            output.output(),
            Limits::default(),
            LAYOUTS,
            &mut budget,
        )
        .unwrap();
        assert!(second.origins().iter().all(|row| row.store.is_none()));
        assert_eq!(
            second.output().canonical_bytes(),
            output.output().canonical_bytes()
        );
    }
    let mut ordinary = fixture(ScalarType::U32);
    let Kind::Load { ref mut access, .. } = blocks(&mut ordinary)[3].operations[0].kind else {
        unreachable!()
    };
    access.alignment = 1;
    assert_count(ordinary, 1);
}

#[test]
fn storage_scalar_forwarding_rejects_missing_unequal_volatile_and_nonmatching_store_classes() {
    let mut missing = storage_fixture(ScalarType::U32);
    blocks(&mut missing)[2].operations.clear();
    expect_count(missing, 0);
    let mut unequal = storage_fixture(ScalarType::U32);
    unequal.functions[0].signature.parameters.push(ty());
    unequal.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(2));
    let Kind::Storage(Storage::WriteValue { ref mut value, .. }) =
        blocks(&mut unequal)[2].operations[0].kind
    else {
        unreachable!()
    };
    *value = ValueId(2);
    expect_count(unequal, 0);
    for index in [1, 3] {
        let mut volatile = storage_fixture(ScalarType::U32);
        match &mut blocks(&mut volatile)[index].operations[0].kind {
            Kind::Storage(
                Storage::WriteValue { access, .. } | Storage::ReadValue { access, .. },
            ) => access.volatile = true,
            _ => unreachable!(),
        }
        expect_count(volatile, 0);
    }
    let mut different = storage_fixture(ScalarType::U32);
    let Kind::Storage(Storage::WriteValue { ref mut access, .. }) =
        blocks(&mut different)[2].operations[0].kind
    else {
        unreachable!()
    };
    access.alignment = 1;
    expect_count(different, 0);
    for scalar in [
        ScalarType::I128,
        ScalarType::U128,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        expect_count(storage_fixture(scalar), 0)
    }
}

#[test]
fn storage_scalar_forwarding_keeps_aggregate_aliases_and_reallocation_cuts_closed() {
    let mut aggregate = storage_fixture(ScalarType::U32);
    aggregate.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: StorageLayoutKindV1::Record(
            vec![StorageFieldV1 {
                offset: 0,
                layout: LayoutId(0),
            }]
            .into_boxed_slice(),
        ),
    });
    let op = &mut blocks(&mut aggregate)[0].operations[0];
    op.results[0].ty = Type::pointer(
        Type::StorageObject(LayoutId(1)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let Kind::Alloca {
        ref mut element, ..
    } = op.kind
    else {
        unreachable!()
    };
    *element = Type::StorageObject(LayoutId(1));
    blocks(&mut aggregate)[0].operations.push(value(
        200,
        Type::pointer(
            Type::StorageObject(LayoutId(0)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        Kind::Storage(Storage::Project {
            base: ValueId(100),
            step: StorageProjectionV1::Field(0),
        }),
    ));
    for block in blocks(&mut aggregate) {
        for op in &mut block.operations {
            if let Kind::Storage(
                Storage::ReadValue { address, .. } | Storage::WriteValue { address, .. },
            ) = &mut op.kind
            {
                *address = ValueId(200)
            }
        }
    }
    expect_count(aggregate, 0);
    let mut allocation_cut = storage_fixture(ScalarType::U32);
    blocks(&mut allocation_cut)[3]
        .operations
        .insert(0, allocation(201, ty()));
    expect_count(allocation_cut, 0);
}

#[test]
fn storage_scalar_forwarding_has_exact_work_peak_and_deterministic_output() {
    let (input, floor) = admit_storage(&storage_fixture(ScalarType::U64));
    let measured = run_storage(&input, floor, WORK, STORAGE);
    let output = measured.0.unwrap();
    let exact = run_storage(&input, floor, measured.1, measured.2);
    assert_eq!(
        exact.0.as_ref().unwrap().output().canonical_bytes(),
        output.output().canonical_bytes()
    );
    assert_eq!(exact.0.as_ref().unwrap().origins(), output.origins());
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (measured.1, measured.2, None, None)
    );
    let denied = run_storage(&input, floor, measured.1 - 1, measured.2);
    assert!(denied.0.is_err());
    assert_eq!(denied.3, Some(measured.1));
    let denied = run_storage(&input, floor, measured.1, measured.2 - 1);
    assert!(denied.0.is_err());
    assert_eq!(denied.4, Some(measured.2));
}

#[test]
fn storage_scalar_forwarding_cycles_require_every_incoming_store_path() {
    let mut grounded = storage_fixture(ScalarType::I32);
    blocks(&mut grounded)[1].terminator = Some(branch(1, 30, 40));
    blocks(&mut grounded)[2].terminator = Some(branch(1, 20, 40));
    expect_count(grounded, 1);
    let mut missing = storage_fixture(ScalarType::I32);
    blocks(&mut missing)[2].operations.clear();
    blocks(&mut missing)[2].terminator = Some(branch(1, 30, 40));
    expect_count(missing, 0);
}

#[test]
fn storage_scalar_layout_query_has_independent_twelve_work_boundary() {
    let (owner, floor) = admit_storage(&storage_fixture(ScalarType::U32));
    for limit in [12, 11] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result = resources::scoped(&mut budget, |meter| {
            cell_scalar(&owner, &Type::StorageObject(LayoutId(0)), meter)
        });
        if limit == 12 {
            assert_eq!(result.unwrap(), Some(ScalarType::U32));
            assert_eq!(budget.work(), 12);
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == 12 && error.limit() == 11)
            );
        }
        assert_eq!(budget.storage(), floor);
    }
}
