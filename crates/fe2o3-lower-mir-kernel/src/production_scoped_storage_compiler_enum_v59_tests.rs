// Inert movement receipts exercise equality only, not source admission.
fn compiler_movement_fixture_v59() -> (ScopedStoragePayloadV59, Operation) {
    let pointer = ValueId(10);
    let value = ValueId(11);
    let storage = CompilerEnumPointerStorageV57 {
        schema: fe2o3_kernel_ir::StorageLayoutIdV1(1),
        pointer: fe2o3_kernel_ir::StoragePointerV1 {
            pointee: fe2o3_kernel_ir::StorageLayoutIdV1(0),
            value_space: AddressSpace::Private,
            encoded_space: AddressSpace::Generic,
            access: AccessMode::ReadOnly,
            stored_bits: 64,
        },
        size: 8,
        alignment: 8,
    };
    let record = ScopedCompilerEnumAccessV55 {
        anchor: 0,
        local: SemanticLocalIdV1::from_index(3),
        variant: 1,
        field: 0,
        component: 0,
        pointer,
        role: ScopedCompilerEnumRoleV55::Store {
            site: ExecutionSiteV29::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 0,
            },
            source: None,
            value,
        },
    };
    let operation = Operation::new(
        vec![],
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: pointer,
            value,
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    );
    (
        ScopedStoragePayloadV59::CompilerEnum { record, storage },
        operation,
    )
}

#[test]
fn compiler_enum_storage_movement_refuses_changed_family_operands_access_and_results() {
    let (payload, original) = compiler_movement_fixture_v59();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    payload.check_operation(&original, &mut budget).unwrap();
    for fault in 0..8 {
        let mut operation = original.clone();
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address,
            value,
            access,
        }) = &mut operation.kind
        else {
            unreachable!()
        };
        match fault {
            0 => *address = ValueId(12),
            1 => *value = *address,
            2 => access.alignment = 4,
            3 => access.address_space = AddressSpace::Global,
            4 => access.volatile = true,
            5 => {
                operation.kind = OperationKind::Store {
                    pointer: *address,
                    value: *value,
                    access: *access,
                }
            }
            6 => {
                operation.kind = OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                    address: *address,
                    access: *access,
                })
            }
            7 => operation
                .results
                .push(ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32))),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                payload.check_operation(&operation, &mut budget),
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "movement fault {fault}"
        );
    }
    let ScopedStoragePayloadV59::CompilerEnum {
        mut record,
        storage,
    } = payload
    else {
        unreachable!()
    };
    record.role = ScopedCompilerEnumRoleV55::Load {
        block: SemanticBlockIdV1::from_index(0),
        result: ValueId(11),
    };
    assert!(
        ScopedStoragePayloadV59::CompilerEnum { record, storage }
            .check_operation(&original, &mut budget)
            .is_err()
    );
}

#[test]
fn compiler_enum_storage_movement_keeps_holder_and_value_pointer_types_distinct() {
    let (ScopedStoragePayloadV59::CompilerEnum { storage, .. }, _) =
        compiler_movement_fixture_v59()
    else {
        unreachable!()
    };
    let holder =
        ScopedStorageTypeV29::Pointer(storage.schema, AddressSpace::Private, AccessMode::ReadWrite);
    let value = ScopedStorageTypeV29::Pointer(
        storage.pointer.pointee,
        storage.pointer.value_space,
        storage.pointer.access,
    );
    let holder_type = Type::pointer(
        Type::StorageObject(storage.schema),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let value_type = Type::pointer(
        Type::StorageObject(storage.pointer.pointee),
        storage.pointer.value_space,
        storage.pointer.access,
    );
    assert!(holder.matches(&holder_type));
    assert!(value.matches(&value_type));
    assert!(!holder.matches(&value_type));
    assert!(!value.matches(&holder_type));
    for ty in [
        Type::pointer(
            Type::StorageObject(storage.schema),
            storage.pointer.value_space,
            storage.pointer.access,
        ),
        Type::pointer(
            Type::StorageObject(storage.pointer.pointee),
            AddressSpace::Global,
            storage.pointer.access,
        ),
        Type::pointer(
            Type::StorageObject(storage.pointer.pointee),
            storage.pointer.value_space,
            AccessMode::ReadWrite,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            storage.pointer.value_space,
            storage.pointer.access,
        ),
    ] {
        assert!(!value.matches(&ty));
    }
}

fn compiler_movement_census_v59(
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let (_, operation) = compiler_movement_fixture_v59();
    let mut first = BasicBlock::new(BlockId(1));
    first.operations = vec![
        operation.clone(),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(10),
                value: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
        operation.clone(),
    ];
    first.terminator = Some(Terminator::Return { values: vec![] });
    let mut second = BasicBlock::new(BlockId(2));
    second.operations = vec![operation];
    second.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::internal_helper(
        "compiler-movement-census",
        Signature::new(vec![], vec![]),
        vec![],
        vec![second, first],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let mut scratch = 0;
        assert_eq!(
            scoped_storage_count_v59(&function, budget).map_err(source_address_call_error_v29)?,
            3
        );
        let actual = scoped_storage_actual_v29(&function, budget, &mut scratch)
            .map_err(source_address_call_error_v29)?;
        assert_eq!(
            actual.iter().map(|row| row.point).collect::<Vec<_>>(),
            vec![(BlockId(1), 0), (BlockId(1), 2), (BlockId(2), 0)]
        );
        assert!(actual.iter().all(|row| !row.seen));
        Ok(())
    });
    assert_eq!(budget.storage(), 37);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn compiler_enum_storage_census_counts_every_typed_operation_with_exact_resource_bounds() {
    let measured = compiler_movement_census_v59(usize::MAX, usize::MAX);
    measured.0.unwrap();
    let exact = compiler_movement_census_v59(measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for short_work in [false, true] {
        let work = measured.1 - usize::from(short_work);
        let storage = measured.2 - usize::from(!short_work);
        let result = compiler_movement_census_v59(work, storage).0;
        match result {
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error),
            )) if short_work => assert_eq!((error.actual(), error.limit()), (measured.1, work)),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error),
            )) if !short_work => assert_eq!((error.actual(), error.limit()), (measured.2, storage)),
            other => panic!("wrong short resource result: {other:?}"),
        }
    }
}

#[test]
fn compiler_enum_storage_movement_headers_match_independent_owned_shapes() {
    #[allow(dead_code)]
    #[derive(Clone, Copy)]
    enum Payload {
        Object(ScopedObjectPayloadV29),
        CompilerEnum {
            record: ScopedCompilerEnumAccessV55,
            storage: CompilerEnumPointerStorageV57,
        },
    }
    #[allow(dead_code)]
    struct Row {
        instance: ProductionCallInstanceIdV1,
        span: usize,
        source: InstanceSpanSourceV1,
        offset: u32,
        call_offset: Option<u32>,
        payload: Payload,
        inputs: [Option<(ValueId, ScopedStorageTypeV29)>; 2],
        result: Option<ScopedStorageTypeV29>,
        next: Option<usize>,
    }
    #[allow(dead_code)]
    struct Checked<'a> {
        record: &'a ScopedCompilerEnumAccessV55,
        anchor: &'a ScopedMemoryAnchorV29,
        spill: &'a ExecutionEnumSpillV48,
        binding: &'a SemanticValueBindingV1,
    }
    type Frame = (
        Payload,
        [Option<(ValueId, ScopedStorageTypeV29)>; 2],
        Checked<'static>,
        CompilerEnumPointerStorageV57,
        [usize; 4],
        [&'static (); 8],
    );
    assert_eq!(size_of::<Payload>(), size_of::<ScopedStoragePayloadV59>());
    assert_eq!(size_of::<Row>(), size_of::<ScopedStorageSourceV29>());
    let expected =
        size_of::<Frame>() + 2 * size_of::<Result<Frame, ProductionSemanticKirErrorV1>>();
    assert_eq!(scoped_storage_compiler_headers_v59().unwrap(), expected);
    for limit in [expected, expected - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = budget.reserve_storage(scoped_storage_compiler_headers_v59().unwrap());
        if limit == expected {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        } else {
            let Err(ArgumentResourceV1::Storage(error)) = result else {
                panic!("expected exact storage denial")
            };
            assert_eq!((error.actual(), error.limit()), (expected, limit));
        }
    }
}
