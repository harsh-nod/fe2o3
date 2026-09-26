use super::*;

fn construct(variant: u32) -> StorageOperationV1 {
    StorageOperationV1::Project {
        base: ValueId(0),
        step: StorageProjectionV1::VariantForWrite { index: variant },
    }
}

fn set(variant: u32) -> StorageOperationV1 {
    StorageOperationV1::SetDiscriminant {
        address: ValueId(0),
        variant,
        access: memory(),
    }
}

#[test]
fn construction_requires_writable_holder_and_forces_write_only_result() {
    for rights in [AccessMode::WriteOnly, AccessMode::ReadWrite] {
        accepts(&module(
            vec![address(8, AddressSpace::Private, rights)],
            vec![address(5, AddressSpace::Private, AccessMode::WriteOnly)],
            construct(0),
        ));
        for result in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            rejects(
                &module(
                    vec![address(8, AddressSpace::Private, rights)],
                    vec![address(5, AddressSpace::Private, result)],
                    construct(0),
                ),
                DiagnosticCode::TypeMismatch,
            );
        }
    }
    for (space, rights) in [
        (AddressSpace::Private, AccessMode::ReadOnly),
        (AddressSpace::Constant, AccessMode::ReadOnly),
    ] {
        rejects(
            &module(
                vec![address(8, space, rights)],
                vec![address(5, space, AccessMode::WriteOnly)],
                construct(0),
            ),
            DiagnosticCode::InvalidMemoryAccess,
        );
    }
    rejects(
        &module(
            vec![private(8)],
            vec![address(5, AddressSpace::Global, AccessMode::WriteOnly)],
            construct(0),
        ),
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn construction_refuses_missing_uninhabited_and_non_enum_children() {
    rejects(
        &module(vec![private(8)], vec![], construct(2)),
        DiagnosticCode::InvalidMemoryAccess,
    );
    rejects(
        &module(vec![private(5)], vec![], construct(0)),
        DiagnosticCode::InvalidMemoryAccess,
    );
    let mut uninhabited = module(
        vec![private(8)],
        vec![address(5, AddressSpace::Private, AccessMode::WriteOnly)],
        construct(0),
    );
    let StorageLayoutKindV1::Variants { variants, .. } = &mut uninhabited.storage_layouts[8].kind
    else {
        unreachable!()
    };
    variants[0].uninhabited = true;
    rejects(&uninhabited, DiagnosticCode::InvalidMemoryAccess);
    rejects(
        &module(vec![private(8)], vec![], construct(0)),
        DiagnosticCode::ResultArity,
    );
    rejects(
        &module(vec![private(8)], vec![private(6)], construct(0)),
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn set_discriminant_requires_write_rights_and_exact_nonvolatile_access() {
    for rights in [AccessMode::WriteOnly, AccessMode::ReadWrite] {
        accepts(&module(
            vec![address(8, AddressSpace::Private, rights)],
            vec![],
            set(0),
        ));
    }
    for (space, rights) in [
        (AddressSpace::Private, AccessMode::ReadOnly),
        (AddressSpace::Constant, AccessMode::ReadOnly),
    ] {
        let operation = StorageOperationV1::SetDiscriminant {
            address: ValueId(0),
            variant: 0,
            access: MemoryAccess::new(space, 1),
        };
        rejects(
            &module(vec![address(8, space, rights)], vec![], operation),
            DiagnosticCode::InvalidMemoryAccess,
        );
    }
    for access in [
        MemoryAccess::new(AddressSpace::Global, 1),
        MemoryAccess {
            volatile: true,
            ..memory()
        },
        MemoryAccess::new(AddressSpace::Private, 16),
    ] {
        rejects(
            &module(
                vec![private(8)],
                vec![],
                StorageOperationV1::SetDiscriminant {
                    address: ValueId(0),
                    variant: 0,
                    access,
                },
            ),
            DiagnosticCode::InvalidMemoryAccess,
        );
    }
    rejects(
        &module(
            vec![private(8)],
            vec![],
            StorageOperationV1::SetDiscriminant {
                address: ValueId(0),
                variant: 0,
                access: MemoryAccess::new(AddressSpace::Private, 3),
            },
        ),
        DiagnosticCode::InvalidAlignment,
    );
    rejects(
        &module(vec![private(8)], vec![Type::INDEX], set(0)),
        DiagnosticCode::ResultArity,
    );
}

#[test]
fn set_discriminant_refuses_missing_uninhabited_and_non_enum_variants() {
    rejects(
        &module(vec![private(8)], vec![], set(2)),
        DiagnosticCode::InvalidMemoryAccess,
    );
    rejects(
        &module(vec![private(5)], vec![], set(0)),
        DiagnosticCode::InvalidMemoryAccess,
    );
    let mut uninhabited = module(vec![private(8)], vec![], set(1));
    let StorageLayoutKindV1::Variants { variants, .. } = &mut uninhabited.storage_layouts[8].kind
    else {
        unreachable!()
    };
    variants[1].uninhabited = true;
    rejects(&uninhabited, DiagnosticCode::InvalidMemoryAccess);
}

fn enum_rows(niche: bool, pointer: bool) -> Vec<StorageLayoutV1> {
    let tag = if pointer {
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(1),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        }
    } else {
        scalar(
            if niche {
                ScalarType::U8
            } else {
                ScalarType::I8
            },
            1,
        )
    };
    let payload = StorageLayoutV1 {
        size: 16,
        alignment: 8,
        kind: StorageLayoutKindV1::Record(
            if niche {
                vec![field(0, 0), field(8, 1)]
            } else {
                vec![field(8, 1)]
            }
            .into_boxed_slice(),
        ),
    };
    let mut variants = vec![
        StorageVariantV1 {
            discriminant: if niche { 0 } else { u128::MAX },
            direct_tag_bits: (!niche).then_some(255),
            uninhabited: false,
            layout: StorageLayoutIdV1(2),
        },
        StorageVariantV1 {
            discriminant: 1,
            direct_tag_bits: (!niche).then_some(1),
            uninhabited: false,
            layout: StorageLayoutIdV1(3),
        },
    ];
    if niche {
        variants.push(StorageVariantV1 {
            discriminant: 2,
            direct_tag_bits: None,
            uninhabited: false,
            layout: StorageLayoutIdV1(3),
        });
    }
    vec![
        tag,
        scalar(ScalarType::U64, 8),
        payload,
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(Box::new([])),
        },
        StorageLayoutV1 {
            size: 16,
            alignment: 8,
            kind: StorageLayoutKindV1::Variants {
                encoding: if niche {
                    StorageVariantEncodingV1::Niche {
                        tag: field(0, 0),
                        untagged_variant: 0,
                        first_niche_variant: 1,
                        last_niche_variant: 2,
                        niche_start: if pointer { u64::MAX.into() } else { 255 },
                    }
                } else {
                    StorageVariantEncodingV1::Direct { tag: field(0, 0) }
                },
                variants: variants.into_boxed_slice(),
            },
        },
    ]
}

#[test]
fn discriminant_selection_preserves_direct_signed_bits_and_niche_boundaries() {
    for (niche, pointer, count) in [(false, false, 2), (true, false, 3), (true, true, 3)] {
        for variant in 0..count {
            let mut candidate = module(vec![private(4)], vec![], set(variant));
            candidate.storage_layouts = enum_rows(niche, pointer);
            let before = candidate.storage_layouts.clone();
            accepts(&candidate);
            assert_eq!(candidate.storage_layouts, before);
        }
        let mut outside = module(vec![private(4)], vec![], set(count));
        outside.storage_layouts = enum_rows(niche, pointer);
        rejects(&outside, DiagnosticCode::InvalidMemoryAccess);
    }
    // Structural acceptance neither decodes a symbolic pointer niche nor marks
    // an untagged payload initialized; those are separate execution obligations.
}

fn sequence(partial: bool, premature_read: bool) -> Module {
    let mut candidate = module(
        vec![private(4), Type::Scalar(ScalarType::U64)],
        vec![],
        set(0),
    );
    candidate.storage_layouts = enum_rows(false, false);
    let operations = &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.clear();
    let mut push = |id, ty, operation| {
        operations.push(Operation::new(
            vec![ValueDef::new(ValueId(id), ty)],
            OperationKind::Storage(operation),
        ))
    };
    push(
        100,
        address(2, AddressSpace::Private, AccessMode::WriteOnly),
        construct(0),
    );
    push(
        101,
        address(1, AddressSpace::Private, AccessMode::WriteOnly),
        StorageOperationV1::Project {
            base: ValueId(100),
            step: StorageProjectionV1::Field(0),
        },
    );
    if premature_read {
        operations.push(Operation::new(
            vec![ValueDef::new(ValueId(102), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(101),
                access: memory(),
            }),
        ));
    } else if !partial {
        operations.push(Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(101),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ));
    }
    operations.push(Operation::new(vec![], OperationKind::Storage(set(0))));
    candidate
}

fn whole(candidate: &Module) -> Result<(), crate::BorrowedKernelIrVerificationErrorV1> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let checked = check_module_storage_v1(candidate, LIMITS, &mut budget).unwrap();
    let result =
        crate::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget).map(|_| ());
    assert_eq!(budget.storage(), FLOOR);
    result
}

#[test]
fn construction_sequence_preserves_payload_store_before_discriminant() {
    let candidate = sequence(false, false);
    whole(&candidate).unwrap();
    let operations = &candidate.functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert!(matches!(
        operations[2].kind,
        OperationKind::Storage(StorageOperationV1::WriteValue { .. })
    ));
    assert!(matches!(
        operations[3].kind,
        OperationKind::Storage(StorageOperationV1::SetDiscriminant { .. })
    ));
}

#[test]
fn partial_construction_is_not_a_read_or_initialization_certificate() {
    whole(&sequence(true, false)).unwrap();
    let Err(crate::BorrowedKernelIrVerificationErrorV1::Verification(errors)) =
        whole(&sequence(false, true))
    else {
        panic!("read through construction-only pointer must fail verification")
    };
    assert!(errors.contains(DiagnosticCode::InvalidMemoryAccess));
}

#[test]
fn construction_and_discriminant_still_require_the_exact_module_owner() {
    for operation in [construct(0), set(0)] {
        let results = if matches!(operation, StorageOperationV1::Project { .. }) {
            vec![address(5, AddressSpace::Private, AccessMode::WriteOnly)]
        } else {
            vec![]
        };
        let candidate = module(vec![private(8)], results, operation);
        let foreign = candidate.clone();
        assert_eq!(
            inspect(&candidate, Some(&foreign)),
            Err(ResourceError::Accounting)
        );
    }
}
