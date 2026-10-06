use super::*;
use crate::CanonicalKirInventoryV18;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, MemoryAccess, StorageFieldV1, StorageLayoutIdV1 as Id,
    StorageLayoutKindV1 as Kind, StorageLayoutLimitsV1, StorageLayoutV1,
    StorageOperationV1 as Storage, StorageVariantEncodingV1, StorageVariantV1,
    VerifiedCanonicalKernelIrModuleV18,
};

fn fixture() -> Module {
    let pointer = Type::pointer(
        Type::StorageObject(Id(1)),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    let mut module = module(
        vec![pointer.clone(), pointer],
        vec![Type::Scalar(ScalarType::U128)],
        vec![0, 1],
        vec![returning(
            0,
            vec![value(
                2,
                Type::Scalar(ScalarType::U128),
                OperationKind::Storage(Storage::ReadDiscriminant {
                    address: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                }),
            )],
            &[2],
        )],
    );
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Variants {
                encoding: StorageVariantEncodingV1::Direct {
                    tag: StorageFieldV1 {
                        offset: 0,
                        layout: Id(0),
                    },
                },
                variants: [17, 250]
                    .into_iter()
                    .zip([255, u128::MAX])
                    .map(|(bits, discriminant)| StorageVariantV1 {
                        discriminant,
                        direct_tag_bits: Some(bits),
                        uninhabited: false,
                        layout: Id(0),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
        },
    ];
    module
}

fn compare(left: &Module, right: &Module) -> bool {
    compare_with(left, right, |a, _| Rows::identity(a))
}

fn compare_with(
    left: &Module,
    right: &Module,
    make: impl FnOnce(&CanonicalKirInventoryV18<'_>, &CanonicalKirInventoryV18<'_>) -> Rows,
) -> bool {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let limits = StorageLayoutLimitsV1 {
        rows: 8,
        edges: 16,
        containment_depth: 8,
        object_bytes: 64,
    };
    let (a, a_bytes) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            left,
            limits,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(a_bytes.retained_storage()).unwrap();
    let (b, b_bytes) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            right,
            limits,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(b_bytes.retained_storage()).unwrap();
    let (ai, ai_bytes) = CanonicalKirInventoryV18::derive_v18(&a, &mut budget).unwrap();
    budget.reserve_storage(ai_bytes.retained_storage()).unwrap();
    let (bi, bi_bytes) = CanonicalKirInventoryV18::derive_v18(&b, &mut budget).unwrap();
    budget.reserve_storage(bi_bytes.retained_storage()).unwrap();
    let rows = make(&ai, &bi);
    budget.reserve_storage(rows.storage()).unwrap();
    let result = check_canonical_kir_transition_v18(&ai, &bi, rows.candidate(), &mut budget);
    result.is_ok()
}

#[test]
fn unused_discriminant_read_cannot_be_deleted_as_a_dead_pure_value() {
    let mut input = fixture();
    input.functions[0].signature.results.clear();
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Return { values: vec![] });
    let mut output = input.clone();
    output.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    assert!(!compare_with(&input, &output, |a, b| Plan {
        chains: vec![vec![(0, None)]],
        relations: vec![(0, 0, R), (1, 1, R)],
        ..Plan::default()
    }
    .rows(a, b)));
}

#[test]
fn identical_pointer_discriminant_reads_cannot_be_csed_with_or_without_a_retag() {
    for retag in [false, true] {
        let mut input = fixture();
        for parameter in &mut input.functions[0].signature.parameters {
            let Type::Pointer(pointer) = parameter else {
                unreachable!()
            };
            pointer.access = AccessMode::ReadWrite;
        }
        let block = &mut input.functions[0].body.as_mut().unwrap().blocks[0];
        if retag {
            block.operations.push(Operation::new(
                vec![],
                OperationKind::Storage(Storage::SetDiscriminant {
                    address: ValueId(0),
                    variant: 1,
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                }),
            ));
        }
        let mut second = block.operations[0].clone();
        second.results[0].id = ValueId(3);
        block.operations.push(second);
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        let mut output = input.clone();
        let block = &mut output.functions[0].body.as_mut().unwrap().blocks[0];
        block.operations.pop();
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(2)],
        });
        assert!(!compare_with(&input, &output, |a, b| {
            let mut operations = vec![Origin::Retained(op(0, 0))];
            let mut uses = vec![operand(0, 0, 0)];
            if retag {
                operations.push(Origin::Retained(op(0, 1)));
                uses.push(operand(0, 1, 0));
            }
            uses.push(term(0, 0));
            Plan {
                chains: vec![vec![(0, None)]],
                operations,
                uses,
                relations: vec![(0, 0, R), (1, 1, R), (2, 2, R), (3, 2, S)],
                ..Plan::default()
            }
            .rows(a, b)
        }));
    }
}

#[test]
fn discriminant_read_transport_checks_actual_operand_access_and_complete_logical_mapping() {
    let original = fixture();
    assert!(compare(&original, &original));
    let operation = &original.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind;
    assert!(
        !payload::pure(operation),
        "identical-pointer tag reads are not CSE values"
    );
    let mut changed = original.clone();
    let OperationKind::Storage(Storage::ReadDiscriminant { address, .. }) =
        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *address = ValueId(1);
    assert!(!compare(&original, &changed));
    let mut changed = original.clone();
    let OperationKind::Storage(Storage::ReadDiscriminant { access, .. }) =
        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    access.alignment = 1;
    assert!(!compare(&original, &changed));
    let mut changed = original.clone();
    let Kind::Variants { variants, .. } = &mut changed.storage_layouts[1].kind else {
        unreachable!()
    };
    variants[0].discriminant = 254;
    assert!(!compare(&original, &changed));
}
