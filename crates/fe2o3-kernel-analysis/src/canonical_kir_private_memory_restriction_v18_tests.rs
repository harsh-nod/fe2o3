use super::*;

pub(super) fn header_oracle() -> usize {
    fn slot<T>() -> usize {
        size_of::<T>() + size_of::<R<T>>()
    }
    slot::<&CanonicalKirInventoryV18<'_>>()
        + slot::<&CanonicalKirOperationRefV1<'_>>()
        + slot::<&[Option<Address>]>()
        + slot::<&mut Budget<'_>>()
        + slot::<&OperationKind>()
        + slot::<&ValueId>()
        + 3 * slot::<&Type>()
        + 3 * slot::<&PointerType>()
        + 3 * slot::<&StorageLayoutIdV1>()
        + slot::<&[CanonicalKirDefinitionRefV1<'_>]>()
        + slot::<Option<&CanonicalKirDefinitionRefV1<'_>>>()
        + 2 * slot::<&CanonicalKirDefinitionRefV1<'_>>()
        + slot::<&[CanonicalKirOperationRefV1<'_>]>()
        + slot::<Option<&CanonicalKirOperationRefV1<'_>>>()
        + slot::<&CanonicalKirOperationRefV1<'_>>()
        + slot::<Option<&Option<Address>>>()
        + slot::<&Option<Address>>()
        + slot::<Option<Address>>()
        + slot::<Option<Option<Address>>>()
        + slot::<Address>()
        + slot::<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1>()
        + 2 * slot::<usize>()
        + slot::<(ValueId, bool)>()
        + slot::<AccessMode>()
        + 2 * slot::<bool>()
        + slot::<()>()
        + slot::<Result<Option<usize>, CanonicalKirInventoryErrorV1>>()
}

fn restriction(input: u32, output: u32) -> Operation {
    let ty = Type::pointer(
        Type::StorageObject(StorageLayoutIdV1(0)),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    Operation::effect_free(
        ValueDef::new(ValueId(output), ty.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(input),
            to: ty,
        },
    )
}

fn alias_read(scalar: ScalarType, alignment: u32, pointer: u32, result: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(scalar)),
        OperationKind::Storage(StorageOperationV1::ReadValue {
            address: ValueId(pointer),
            access: MemoryAccess::new(AddressSpace::Private, alignment),
        }),
    )
}

fn restricted(scalar: ScalarType, bytes: u64, alignment: u32) -> Module {
    fixture(
        scalar,
        bytes,
        alignment,
        vec![block(
            100,
            vec![
                allocation(alignment),
                write(alignment),
                restriction(10, 30),
                alias_read(scalar, alignment, 30, 40),
            ],
            ret(),
        )],
    )
}

#[test]
fn typed_private_restriction_all_scalar_layouts_keep_allocation_and_memory_census() {
    let fixed = [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ];
    let mut cases = fixed
        .into_iter()
        .map(|scalar| (scalar, u64::from(scalar.bit_width().unwrap().div_ceil(8))))
        .collect::<Vec<_>>();
    cases.extend([1, 2, 4, 8, 16].map(|bytes| (ScalarType::Index, bytes)));
    for (scalar, bytes) in cases {
        for alignment in [1, bytes as u32] {
            with_inventory(&restricted(scalar, bytes, alignment), |inventory, floor| {
                exercise(
                    inventory,
                    floor,
                    WORK,
                    STORAGE,
                    1,
                    &[None, None, None, Some(1)],
                )
                .0
                .unwrap();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                let (proof, receipt) = check_canonical_kir_private_memory_v18(
                    inventory,
                    CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
                    &mut budget,
                )
                .unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(
                    (0..4).map(|op| proof.operation(op)).collect::<Vec<_>>(),
                    [true, true, false, true]
                );
                let allocation = inventory.operations()[0].results.start;
                let alias = inventory.operations()[2].results.start;
                assert_eq!(proof.address(alias), proof.address(allocation));
                let address = proof.address(alias).unwrap();
                assert_eq!(proof.access_restriction(2), Some(address));
                for other in [0, 1, 3, 4, usize::MAX] {
                    assert!(proof.access_restriction(other).is_none());
                }
                assert_eq!(
                    (
                        address.allocation(),
                        address.length(),
                        address.offset(),
                        address.stride()
                    ),
                    (0, 1, 0, bytes as usize)
                );
                assert!(!proof.grants_authority());
                assert!(std::ptr::eq(proof.inventory().owner(), inventory.owner()));
                drop(proof);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn typed_private_restriction_keeps_count_extent_and_multiple_aliases() {
    let mut module = restricted(ScalarType::U32, 4, 4);
    let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    if let OperationKind::Alloca { count, .. } = &mut ops[0].kind {
        *count = Some(ValueId(50));
    }
    ops.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(50), Type::INDEX),
            OperationKind::Constant(Constant::Index(3)),
        ),
    );
    ops.push(restriction(10, 31));
    ops.push(alias_read(ScalarType::U32, 4, 31, 41));
    with_inventory(&module, |inventory, floor| {
        exercise(
            inventory,
            floor,
            WORK,
            STORAGE,
            3,
            &[None, None, None, None, Some(2), None, Some(2)],
        )
        .0
        .unwrap();
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 2, &[]).0,
            Err(Error::Unsupported {
                phase: "private",
                detail: "bounded nonzero allocation extent"
            })
        ));
    });
}

#[test]
fn typed_private_restriction_cfg_tracks_initialization_and_exact_writers_separately() {
    let positive = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(
                100,
                vec![allocation(4), write(4), restriction(10, 30)],
                conditional(200, 300),
            ),
            block(300, vec![], branch(400)),
            block(200, vec![], branch(400)),
            block(400, vec![alias_read(ScalarType::U32, 4, 30, 40)], ret()),
        ],
    );
    with_inventory(&positive, |inventory, floor| {
        exercise(
            inventory,
            floor,
            WORK,
            STORAGE,
            1,
            &[None, None, None, Some(1)],
        )
        .0
        .unwrap();
    });
    let mut conflict = positive.clone();
    conflict.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(4));
    with_inventory(&conflict, |inventory, floor| {
        // Both paths initialize the allocation, but the read cannot claim
        // either predecessor's write as its single reaching value anchor.
        exercise(inventory, floor, WORK, STORAGE, 1, &[None; 5])
            .0
            .unwrap();
    });
    let mut absent = positive.clone();
    absent.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(1);
    with_inventory(&absent, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                phase: "private",
                detail: "Load requires initialized storage on every path"
            })
        ));
    });
    let mut looped = positive.clone();
    looped.functions[0].body.as_mut().unwrap().blocks[3].terminator = Some(conditional(400, 500));
    looped.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(block(500, vec![], ret()));
    with_inventory(&looped, |inventory, floor| {
        exercise(
            inventory,
            floor,
            WORK,
            STORAGE,
            1,
            &[None, None, None, Some(1)],
        )
        .0
        .unwrap();
    });
    looped.functions[0].body.as_mut().unwrap().blocks[3]
        .operations
        .push(write(4));
    with_inventory(&looped, |inventory, floor| {
        exercise(inventory, floor, WORK, STORAGE, 1, &[None; 5])
            .0
            .unwrap();
    });
    let mut reentry = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![], branch(200)),
            block(
                200,
                vec![allocation(4), write(4), restriction(10, 30)],
                branch(300),
            ),
            block(
                300,
                vec![alias_read(ScalarType::U32, 4, 30, 40)],
                conditional(200, 400),
            ),
            block(400, vec![], ret()),
        ],
    );
    with_inventory(&reentry, |inventory, floor| {
        exercise(
            inventory,
            floor,
            WORK,
            STORAGE,
            1,
            &[None, None, None, Some(1)],
        )
        .0
        .unwrap();
    });
    let blocks = &mut reentry.functions[0].body.as_mut().unwrap().blocks;
    blocks[1].operations.remove(1);
    blocks[2].operations.push(write(4));
    with_inventory(&reentry, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                phase: "private",
                detail: "Load requires initialized storage on every path"
            })
        ));
    });
}

#[test]
fn typed_private_restriction_join_requires_exact_work_and_storage_without_a_writer() {
    let module = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(
                100,
                vec![allocation(4), write(4), restriction(10, 30)],
                conditional(200, 300),
            ),
            block(300, vec![write(4)], branch(400)),
            block(200, vec![], branch(400)),
            block(400, vec![alias_read(ScalarType::U32, 4, 30, 40)], ret()),
        ],
    );
    with_inventory(&module, |inventory, floor| {
        let expected = [None; 5];
        let measured = exercise(inventory, floor, WORK, STORAGE, 1, &expected);
        measured.0.unwrap();
        exercise(inventory, floor, measured.1, measured.2, 1, &expected)
            .0
            .unwrap();
        assert!(matches!(
            exercise(inventory, floor, measured.1 - 1, measured.2, 1, &expected).0,
            Err(Error::Resource(Resource::Work(error)))
                if error.limit() == measured.1 - 1 && error.actual() > error.limit(),
        ));
        assert!(matches!(
            exercise(inventory, floor, measured.1, measured.2 - 1, 1, &expected).0,
            Err(Error::Resource(Resource::Storage(error)))
                if error.limit() == measured.2 - 1 && error.actual() > error.limit(),
        ));
    });
}

#[test]
fn typed_private_restriction_same_layout_other_allocation_cannot_supply_initialization() {
    let mut module = restricted(ScalarType::U32, 4, 4);
    let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let mut second = allocation(4);
    second.results[0].id = ValueId(11);
    ops.insert(1, second);
    ops[3] = restriction(11, 30);
    with_inventory(&module, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 2, &[]).0,
            Err(Error::Unsupported {
                phase: "private",
                detail: "Load requires initialized storage on every path"
            })
        ));
    });
}

#[test]
fn typed_private_restriction_escape_generic_and_volatile_remain_closed() {
    for mode in 0..3 {
        let mut module = restricted(ScalarType::U32, 4, 4);
        let function = &mut module.functions[0];
        let block = &mut function.body.as_mut().unwrap().blocks[0];
        let expected = match mode {
            0 => {
                function
                    .signature
                    .results
                    .push(block.operations[2].results[0].ty.clone());
                block.terminator = Some(Terminator::Return {
                    values: vec![ValueId(30)],
                });
                "no private pointer control transport"
            }
            1 => {
                block.operations.pop();
                let ty = Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(0)),
                    AddressSpace::Generic,
                    AccessMode::ReadOnly,
                );
                block.operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(60), ty.clone()),
                    OperationKind::Cast {
                        kind: CastKind::PointerToGeneric,
                        value: ValueId(30),
                        to: ty,
                    },
                ));
                "private pointer does not escape"
            }
            2 => {
                let OperationKind::Storage(StorageOperationV1::ReadValue { access, .. }) =
                    &mut block.operations[3].kind
                else {
                    unreachable!()
                };
                access.volatile = true;
                "one ordinary nonvolatile memory effect"
            }
            _ => unreachable!(),
        };
        with_inventory(&module, |inventory, floor| {
            assert!(
                matches!(exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
                Err(Error::Unsupported { phase: "private", detail }) if detail == expected)
            );
        });
    }
}

#[test]
fn typed_private_restriction_invalid_casts_and_readonly_writes_fail_canonical_admission() {
    with_inventory(&restricted(ScalarType::U32, 4, 4), |inventory, floor| {
        exercise(
            inventory,
            floor,
            WORK,
            STORAGE,
            1,
            &[None, None, None, Some(1)],
        )
        .0
        .unwrap();
    });
    for mode in 0..5 {
        let mut module = restricted(ScalarType::U32, 4, 4);
        if mode == 3 {
            module
                .storage_layouts
                .push(module.storage_layouts[0].clone());
        }
        let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mode {
            0 => {
                let mut invalid = write(4);
                let OperationKind::Storage(StorageOperationV1::WriteValue { address, .. }) =
                    &mut invalid.kind
                else {
                    unreachable!()
                };
                *address = ValueId(30);
                ops[3] = invalid;
            }
            1 | 2 | 3 => {
                let OperationKind::Cast { to, .. } = &mut ops[2].kind else {
                    unreachable!()
                };
                let Type::Pointer(pointer) = to else {
                    unreachable!()
                };
                match mode {
                    1 => pointer.access = AccessMode::ReadWrite,
                    2 => pointer.address_space = AddressSpace::Workgroup,
                    3 => pointer.pointee = Box::new(Type::StorageObject(StorageLayoutIdV1(1))),
                    _ => unreachable!(),
                }
                let result_type = to.clone();
                ops[2].results[0].ty = result_type;
                ops.pop();
            }
            4 => {
                let OperationKind::Cast { kind, .. } = &mut ops[2].kind else {
                    unreachable!()
                };
                *kind = CastKind::PointerToGeneric;
                ops.pop();
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let owner =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                LAYOUTS,
                &mut budget,
            );
        assert!(
            owner.is_err(),
            "invalid canonical restriction mode {mode} admitted"
        );
    }
}

#[test]
fn typed_private_restriction_exact_work_storage_and_one_short_keep_no_partial_proof() {
    with_inventory(&restricted(ScalarType::U32, 4, 4), |inventory, floor| {
        let expected = [None, None, None, Some(1)];
        let measured = exercise(inventory, floor, WORK, STORAGE, 1, &expected);
        measured.0.unwrap();
        let exact = exercise(inventory, floor, measured.1, measured.2, 1, &expected);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(
            matches!(exercise(inventory, floor, measured.1 - 1, measured.2, 1, &[]).0,
            Err(Error::Resource(Resource::Work(error))) if error.limit() == measured.1 - 1
                && error.actual() > error.limit())
        );
        assert!(
            matches!(exercise(inventory, floor, measured.1, measured.2 - 1, 1, &[]).0,
            Err(Error::Resource(Resource::Storage(error))) if error.limit() == measured.2 - 1
                && error.actual() > error.limit())
        );
    });
}

#[test]
fn typed_private_restriction_has_no_v12_or_ordinary_scalar_pointer_admission() {
    let mut module = local(ScalarType::U32, 4, 4);
    module.storage_layouts.clear();
    let ops = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    ops.clear();
    let rw = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let ro = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    );
    ops.push(Operation::effect_free(
        ValueDef::new(ValueId(10), rw),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    ops.push(Operation::effect_free(
        ValueDef::new(ValueId(30), ro.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(10),
            to: ro,
        },
    ));
    with_inventory(&module, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                phase: "private",
                detail: "restriction belongs to typed scalar allocation"
            })
        ));
    });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        check_canonical_kir_private_memory_v1(
            &inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
            &mut budget
        ),
        Err(Error::Unsupported {
            phase: "private",
            detail: "direct allocation or constant element address"
        })
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn typed_private_restriction_helper_pays_exact_lookup_and_layout_work_and_rejects_bad_lineage() {
    with_inventory(&restricted(ScalarType::U32, 4, 4), |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (proof, receipt) = check_canonical_kir_private_memory_v18(
            inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let query_headers = headers().unwrap();
        budget.reserve_storage(query_headers).unwrap();
        let row = &inventory.operations()[2];
        let source = inventory.operations()[0].results.start;
        assert_eq!(restriction_headers().unwrap(), header_oracle());
        // Value keys [10,20,30,40,900]: three two-work binary-search steps,
        // plus 36 fixed joins and scalar-layout/stride work 8+3.
        for limit in [53, 52] {
            let mut query_work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut query = Budget::new(&mut query_work, 23 + query_headers);
            query.reserve_storage(23 + query_headers).unwrap();
            let result = restrict_address(inventory, row, &proof.definitions, &mut query);
            if limit == 53 {
                assert_eq!(result.unwrap(), *proof.address(source).unwrap());
                assert_eq!(query.work(), 53);
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(error)))
                    if error.actual() == 53 && error.limit() == 52));
                assert_eq!(query.work(), 50);
            }
            assert_eq!(query.storage(), 23 + query_headers);
        }
        // These fresh-meter copied-table cases test only the internal join;
        // authentic owner admission is covered by the whole constructors.
        for mode in 0..5 {
            let mut copied = proof.definitions.clone();
            let expected = match mode {
                0 => {
                    copied[source] = None;
                    "restriction has exact allocation lineage"
                }
                1 => {
                    copied[source].as_mut().unwrap().allocation = usize::MAX;
                    "restriction has exact allocation lineage"
                }
                2 => {
                    copied[source].as_mut().unwrap().allocation = 1;
                    "restriction belongs to typed scalar allocation"
                }
                3 => {
                    copied[source].as_mut().unwrap().offset = 1;
                    "restriction has direct typed allocation lineage"
                }
                4 => {
                    copied[source].as_mut().unwrap().stride = 8;
                    "restriction preserves exact private scalar layout"
                }
                _ => unreachable!(),
            };
            let mut query_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut query = Budget::new(&mut query_work, 23 + query_headers);
            query.reserve_storage(23 + query_headers).unwrap();
            assert!(
                matches!(restrict_address(inventory, row, &copied, &mut query),
                Err(Error::Unsupported { phase: "private", detail }) if detail == expected)
            );
        }
        // Copied table refusals do not change the original proof or its owner.
        assert_eq!(
            restrict_address(inventory, row, &proof.definitions, &mut budget).unwrap(),
            *proof.address(source).unwrap()
        );
        with_inventory(&restricted(ScalarType::U32, 4, 4), |foreign, _| {
            assert!(!proof.is_for(foreign));
            assert!(!std::ptr::eq(proof.inventory().owner(), foreign.owner()));
        });
        budget.release_storage(query_headers).unwrap();
        drop(proof);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
