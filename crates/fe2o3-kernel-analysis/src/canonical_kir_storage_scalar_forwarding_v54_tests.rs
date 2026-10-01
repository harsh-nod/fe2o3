use super::*;
use fe2o3_kernel_ir::{StorageLayoutIdV1 as LayoutId, StorageLayoutLimitsV1, StorageLayoutV1};
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 1,
    edges: 0,
    containment_depth: 1,
    object_bytes: 16,
};

fn storage_input(scalar: ScalarType) -> Module {
    let mut module = fixture();
    module.functions[0].signature.parameters[0] = Type::Scalar(scalar);
    module.functions[0].signature.results[0] = Type::Scalar(scalar);
    let width = u32::from(scalar.bit_width().unwrap() / 8);
    module.storage_layouts.push(StorageLayoutV1 {
        size: u64::from(width),
        alignment: width,
        kind: StorageLayoutKindV1::Scalar(scalar),
    });
    let body = blocks(&mut module);
    body[0].operations[0] = Operation::effect_free(
        ValueDef::new(
            ValueId(100),
            Type::pointer(
                Type::StorageObject(LayoutId(0)),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        Kind::Alloca {
            element: Type::StorageObject(LayoutId(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: width,
        },
    );
    body[0].operations.truncate(1);
    for index in [1, 2] {
        body[index].operations.push(Operation::new(
            vec![],
            Kind::Storage(Storage::WriteValue {
                address: ValueId(100),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, width),
            }),
        ));
    }
    body[3].operations[0] = Operation::effect_free(
        ValueDef::new(ValueId(101), Type::Scalar(scalar)),
        Kind::Storage(Storage::ReadValue {
            address: ValueId(100),
            access: MemoryAccess::new(AddressSpace::Private, 1),
        }),
    );
    module
}
fn storage_output(input: &Module) -> (Module, Vec<Row>) {
    let mut output = input.clone();
    blocks(&mut output)[3].operations[0].kind = Kind::Binary {
        op: BinaryOp::BitOr,
        lhs: ValueId(0),
        rhs: ValueId(0),
    };
    let rows = input.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(b, block)| {
            block.operations.iter().enumerate().map(move |(o, _)| Row {
                input: site(b, o),
                output: site(b, o),
                store: (b == 3 && o == 0).then_some(site(1, 0)),
            })
        })
        .collect();
    (output, rows)
}
fn storage_owner(module: &Module) -> Owner18 {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner18::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget)
        .unwrap()
        .0
}
fn check_storage(
    input: &Owner18,
    output: &Owner18,
    rows: &[Row],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize, Option<usize>, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(113).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = check_canonical_kir_cross_block_forwarding_v18(
        input,
        output,
        rows,
        Limits::default(),
        &mut budget,
    )
    .map(|(pair, _)| {
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), output));
        assert!(!pair.grants_authority());
    });
    assert_eq!(budget.storage(), 113);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let used = budget.work();
    let peak = budget.peak_storage();
    let failed_storage = budget.failed_storage();
    drop(budget);
    (result, used, peak, work.failed_work(), failed_storage)
}
fn reject_storage(module: &Module) {
    let (out, rows) = storage_output(module);
    assert!(
        check_storage(
            &storage_owner(module),
            &storage_owner(&out),
            &rows,
            WORK,
            STORAGE
        )
        .0
        .is_err()
    );
}

#[test]
fn storage_scalar_pair_independently_accepts_each_fixed_width_and_alignment_difference() {
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
        let input = storage_input(scalar);
        let (output, mut rows) = storage_output(&input);
        let (before, after) = (storage_owner(&input), storage_owner(&output));
        check_storage(&before, &after, &rows, WORK, STORAGE)
            .0
            .unwrap();
        // Either distinct actual store is a valid representative; both paths
        // must be independently checked, not only the listed coordinate.
        rows.last_mut().unwrap().store = Some(site(2, 0));
        check_storage(&before, &after, &rows, WORK, STORAGE)
            .0
            .unwrap();
    }
}

#[test]
fn storage_scalar_pair_rejects_missing_or_unequal_nonrepresentative_path_and_unsupported_width() {
    let mut missing = storage_input(ScalarType::U32);
    blocks(&mut missing)[2].operations.clear();
    reject_storage(&missing);
    let mut unequal = storage_input(ScalarType::U32);
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
    reject_storage(&unequal);
    let mut alignment = storage_input(ScalarType::U32);
    let Kind::Storage(Storage::WriteValue { ref mut access, .. }) =
        blocks(&mut alignment)[2].operations[0].kind
    else {
        unreachable!()
    };
    access.alignment = 1;
    reject_storage(&alignment);
    for scalar in [ScalarType::I128, ScalarType::U128] {
        reject_storage(&storage_input(scalar))
    }
}

#[test]
fn storage_scalar_pair_rejects_volatile_clobbers_and_forged_actual_output() {
    let mut volatile = storage_input(ScalarType::U32);
    let Kind::Storage(Storage::ReadValue { ref mut access, .. }) =
        blocks(&mut volatile)[3].operations[0].kind
    else {
        unreachable!()
    };
    access.volatile = true;
    reject_storage(&volatile);
    let mut clobber = storage_input(ScalarType::U32);
    blocks(&mut clobber)[2].operations.push(allocation(200));
    reject_storage(&clobber);
    let input = storage_input(ScalarType::U32);
    let (mut output, rows) = storage_output(&input);
    blocks(&mut output)[3].operations[0].kind = Kind::Binary {
        op: BinaryOp::BitXor,
        lhs: ValueId(0),
        rhs: ValueId(0),
    };
    assert!(
        check_storage(
            &storage_owner(&input),
            &storage_owner(&output),
            &rows,
            WORK,
            STORAGE
        )
        .0
        .is_err()
    );
    let (output, mut rows) = storage_output(&input);
    rows.last_mut().unwrap().store = Some(site(0, 0));
    assert!(
        check_storage(
            &storage_owner(&input),
            &storage_owner(&output),
            &rows,
            WORK,
            STORAGE
        )
        .0
        .is_err()
    );
}

#[test]
fn storage_scalar_pair_exact_work_and_peak_keep_first_denial_and_floor() {
    let input = storage_input(ScalarType::I64);
    let (output, rows) = storage_output(&input);
    let (before, after) = (storage_owner(&input), storage_owner(&output));
    let full = check_storage(&before, &after, &rows, WORK, STORAGE);
    full.0.unwrap();
    let exact = check_storage(&before, &after, &rows, full.1, full.2);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (full.1, full.2, None, None)
    );
    let denied = check_storage(&before, &after, &rows, full.1 - 1, full.2);
    assert!(denied.0.is_err());
    assert_eq!(denied.3, Some(full.1));
    let denied = check_storage(&before, &after, &rows, full.1, full.2 - 1);
    assert!(denied.0.is_err());
    assert_eq!(denied.4, Some(full.2));
}
