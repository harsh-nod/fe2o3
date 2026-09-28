#[test]
fn source_slice_holder_identity_checks_value_element_address_space_and_permissions() {
    // Component equations are not source admission. The original descriptor
    // plus private-local consuming tests exercise the authentic Use/archive
    // path and retain all actual descriptor/currentness obligations.
    let semantic = SemanticTypeIdV1::from_index(0);
    for space in [
        AddressSpace::Generic,
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Workgroup,
    ] {
        for access in [
            AccessMode::ReadOnly,
            AccessMode::ReadWrite,
            AccessMode::WriteOnly,
        ] {
            for nested in [false, true] {
                let element = if nested {
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadOnly,
                    )
                } else {
                    Type::Scalar(ScalarType::U32)
                };
                let original = SemanticValueBindingV1::Value {
                    id: ValueId(81),
                    ty: Type::slice(element, space, access),
                };
                for mutation in 0..6 {
                    let mut candidate = original.clone();
                    let SemanticValueBindingV1::Value { id, ty } = &mut candidate else {
                        unreachable!()
                    };
                    let Type::Slice(slice) = ty else {
                        unreachable!()
                    };
                    match mutation {
                        0 => {}
                        1 => *id = ValueId(82),
                        2 => {
                            slice.address_space = if space == AddressSpace::Generic {
                                AddressSpace::Global
                            } else {
                                AddressSpace::Generic
                            }
                        }
                        3 => {
                            slice.access = if access == AccessMode::ReadOnly {
                                AccessMode::ReadWrite
                            } else {
                                AccessMode::ReadOnly
                            }
                        }
                        4 => slice.element = Box::new(Type::Scalar(ScalarType::U64)),
                        _ => *ty = Type::pointer((*slice.element).clone(), space, access),
                    }
                    let left = SemanticValueBindingV1::Aggregate(vec![
                        SemanticValueBindingV1::Unit,
                        candidate,
                    ]);
                    let right = SemanticValueBindingV1::Aggregate(vec![
                        SemanticValueBindingV1::Unit,
                        original.clone(),
                    ]);
                    let projections = [
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), semantic)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic)
                            .unwrap(),
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::ConstantIndex {
                                offset: 999,
                                minimum_length: 1000,
                                from_end: false,
                            },
                            semantic,
                        )
                        .unwrap(),
                    ];
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
                    let mut budget = ArgumentBudgetV1::new(&mut work, 23);
                    budget.reserve_storage(23).unwrap();
                    let expected = if mutation == 5 {
                        None
                    } else {
                        Some(mutation == 0)
                    };
                    assert_eq!(
                        execution_archive_pointer_holder_same_v29(
                            &left,
                            &right,
                            &projections,
                            &mut budget
                        )
                        .unwrap(),
                        expected
                    );
                    assert_eq!((budget.storage(), budget.peak_storage()), (23, 23));
                    // The deliberately unchecked suffix is still not authorized.
                }
            }
        }
    }
}

#[test]
fn source_slice_holder_identity_keeps_typed_object_and_shape_routes_closed() {
    let semantic = SemanticTypeIdV1::from_index(0);
    let dereference =
        [SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic).unwrap()];
    let slice = SemanticValueBindingV1::Value {
        id: ValueId(81),
        ty: Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    };
    let object = SemanticValueBindingV1::Value {
        id: ValueId(81),
        ty: Type::slice(
            Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(7)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    for other in [
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::MovedExecution,
        cfg_option_pointer_equation_fixture().1,
        object.clone(),
    ] {
        assert_eq!(
            execution_archive_pointer_holder_same_v29(&slice, &other, &dereference, &mut budget)
                .unwrap(),
            None
        );
    }
    assert_eq!(
        execution_archive_pointer_holder_same_v29(&object, &object, &dereference, &mut budget)
            .unwrap(),
        None
    );
    assert_eq!(
        execution_archive_pointer_holder_same_v29(&slice, &slice, &[], &mut budget).unwrap(),
        None
    );
    let field = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), semantic).unwrap()];
    assert_eq!(
        execution_archive_pointer_holder_same_v29(&slice, &slice, &field, &mut budget).unwrap(),
        None
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_slice_holder_identity_has_exact_structural_work_and_zero_scratch() {
    let semantic = SemanticTypeIdV1::from_index(0);
    let dereference =
        [SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, semantic).unwrap()];
    let binding = SemanticValueBindingV1::Value {
        id: ValueId(81),
        ty: Type::slice(
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    };
    // One dispatch, one projection, and two complete three-node type trees.
    let required = 1 + 1 + 2 * 3;
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(required - usize::from(short));
        let mut budget = ArgumentBudgetV1::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        let result = execution_archive_pointer_holder_same_v29(
            &binding,
            &binding,
            &dereference,
            &mut budget,
        );
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            assert_eq!(result.unwrap(), Some(true));
            assert_eq!(budget.work(), required);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (31, 31));
    }
}
