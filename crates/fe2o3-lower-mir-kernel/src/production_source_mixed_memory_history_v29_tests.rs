// Inert physical equations only; genuine source admission is covered by the
// same-candidate mixed source fixture, not by these constructed slot labels.
fn mixed_history_module_v29(length: u64, fault: usize) -> Module {
    use fe2o3_kernel_ir::{
        StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1,
    };
    let mut module = literal_array_module(
        length,
        match fault {
            1 => 4,
            4 => 3,
            _ => 0,
        },
    );
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: Kind::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: Id(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: Id(0),
                    },
                ]
                .into(),
            ),
        },
    ];
    let pointer = |schema| {
        Type::pointer(
            Type::StorageObject(Id(schema)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        )
    };
    let project = |id, field| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), pointer(0)),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: ValueId(20),
                step: ScopedObjectProjectionV29::Field(field),
            }),
        )
    };
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    assert_eq!(block.operations.len(), 9);
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(20), pointer(1)),
            OperationKind::Alloca {
                element: Type::StorageObject(Id(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        project(21, 0),
        project(22, 1),
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: ValueId(21),
                value: VALUE,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(23), Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: ValueId(if fault == 2 { 22 } else { 21 }),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(24), Type::INDEX),
            OperationKind::Constant(Constant::Index(u64::from(fault == 3))),
        ),
        if fault == 3 {
            Operation::effect_free(
                ValueDef::new(ValueId(25), pointer(1)),
                OperationKind::GetElementPointer {
                    base: ValueId(20),
                    offset: ValueId(24),
                },
            )
        } else {
            // An unused typed subobject is still part of the actual geometry
            // census. Generic GEP is invalid for StorageObject even at zero.
            project(25, 0)
        },
    ]);
    module
}

fn run_mixed_history_v29(
    length: u64,
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let module = mixed_history_module_v29(length, fault);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = ArgumentBudgetV1::new(&mut setup_work, LIMIT);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut setup,
        )
        .unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut setup).unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    let function = inventory.functions()[0].function;
    let slots = [
        literal_array_slot(length),
        ScopedSourceSlotV29 {
            instance: ProductionCallInstanceIdV1(0),
            origin: ScopedSlotOriginV29 {
                identity: ScopedAllocationIdentityV29::OriginalObject {
                    local: 1,
                    generation: 0,
                },
                source: ScopedAllocationSourceV29::OriginalObject {
                    cell: 1,
                    schema: Id(1),
                },
                semantic_type: SemanticTypeIdV1::from_index(2),
                pointer: ValueId(20),
            },
            representation: ScopedSlotRepresentationV29::Object {
                schema: Id(1),
                bytes: 8,
                alignment: 4,
            },
            allocation: PrivateArrayPhysicalLocationV1 {
                block_ordinal: 0,
                block: BlockId(0),
                operation: 9,
            },
        },
    ];
    let accesses: Vec<_> = [(5, 0), (6, 0), (12, 1), (13, 1)]
        .into_iter()
        .map(|(operation, slot)| SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(0),
            operation,
            slot,
        })
        .collect();
    let kills = match fault {
        5 => vec![SourceAddressKillV29 {
            block: BlockId(0),
            gap: 13,
            slot: 1,
        }],
        6 => vec![SourceAddressKillV29 {
            block: BlockId(0),
            gap: 6,
            slot: 0,
        }],
        _ => Vec::new(),
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_inventory(
            &inventory,
            CanonicalKirFunctionCoordinateV1(0),
            &slots,
            &accesses,
            budget,
        )?
        .solve_pending_indices(&slots, &accesses, &kills, budget)?
        .graph;
        scoped_slot_uses_v29::check_expanded_index_addresses_v29(
            function,
            &graph,
            &slots,
            &accesses,
            &kills,
            &[],
            &[],
            &[],
            &[],
            &inventory,
            None,
            CanonicalKirFunctionCoordinateV1(0),
            budget,
        )
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn mixed_object_ranges_and_literal_array_cells_have_disjoint_initialization_and_kills() {
    for length in [2, 4, 1_000_000] {
        let (positive, _, _) = run_mixed_history_v29(length, 0, LIMIT, LIMIT);
        assert!(positive.is_ok(), "length={length}: {positive:?}");
    }
    for fault in [1, 2, 4, 5, 6] {
        assert!(run_mixed_history_v29(4, 0, LIMIT, LIMIT).0.is_ok());
        let error = run_mixed_history_v29(4, fault, LIMIT, LIMIT).0.unwrap_err();
        let expected = match fault {
            1 | 2 | 5 => "scoped slot read is not initialized in its fresh physical activation",
            4 => "scoped source-slot allocation census is incomplete or mismatched",
            // A whole-slot kill uses Literal(0), while this sparse fixture
            // observes only Literal(3); roster validation refuses it first.
            6 => "scoped slot history event has no observed cell",
            _ => unreachable!(),
        };
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None, detail,
        } if detail == expected),
            "fault={fault}: {error:?}"
        );
    }
}

#[test]
fn mixed_history_unused_generic_object_gep_is_rejected_by_canonical_verification() {
    assert!(run_mixed_history_v29(4, 0, LIMIT, LIMIT).0.is_ok());
    let module = mixed_history_module_v29(4, 3);
    // The legacy API rejects the layout table before visiting any operation;
    // that refusal cannot stand in for the storage-aware bad-GEP check.
    let legacy = fe2o3_kernel_ir::verify_module(&module).unwrap_err();
    let [diagnostic] = legacy.diagnostics() else {
        panic!("one exact profile refusal: {legacy:?}");
    };
    assert_eq!(
        diagnostic.code,
        fe2o3_kernel_ir::DiagnosticCode::InvalidSemanticOperation
    );
    assert_eq!(diagnostic.location.operation, None);
    assert_eq!(
        diagnostic.message,
        "module storage layouts require a storage-aware verification profile"
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget =
        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, LIMIT);
    let storage = fe2o3_kernel_ir::check_module_storage_v1(
        &module,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let error =
        match fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(storage, None, &mut budget)
        {
            Err(fe2o3_kernel_ir::BorrowedKernelIrVerificationErrorV1::Verification(error)) => error,
            other => panic!("expected exact storage-aware semantic refusal: {other:?}"),
        };
    let [diagnostic] = error.diagnostics() else {
        panic!("one exact typed-operation refusal: {error:?}");
    };
    assert_eq!(
        diagnostic.code,
        fe2o3_kernel_ir::DiagnosticCode::InvalidMemoryAccess
    );
    assert_eq!(diagnostic.location.operation, Some(15));
    assert_eq!(
        diagnostic.message,
        "generic operations cannot bridge or erase a storage layout; use the typed storage family"
    );
}

#[test]
fn mixed_history_whole_transaction_has_exact_and_one_short_resource_boundaries() {
    let (positive, work, peak) = run_mixed_history_v29(4, 0, LIMIT, LIMIT);
    assert!(positive.is_ok(), "{positive:?}");
    assert!(run_mixed_history_v29(4, 0, work, peak).0.is_ok());
    for (work_limit, storage_limit, work_failure) in
        [(work - 1, peak, true), (work, peak - 1, false)]
    {
        let error = run_mixed_history_v29(4, 0, work_limit, storage_limit)
            .0
            .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            ) && work_failure
                || matches!(
                    error,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ) && !work_failure,
            "{error:?}"
        );
    }
}
