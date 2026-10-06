// Physical equations only. The unchanged-source relocation corpus separately
// requires both whole-array and element initialization to finish admission.
include!("production_source_descriptor_local_geometry_v29_tests.rs");
include!("production_source_index_fold_v29_tests.rs");
include!("production_source_mixed_memory_history_v29_tests.rs");
use fe2o3_kernel_analysis::CanonicalKirInventoryV18;
use fe2o3_kernel_ir::{CanonicalKirFunctionCoordinateV1, VerifiedCanonicalKernelIrModuleV18};

const ARRAY: ValueId = ValueId(10);
const COUNT: ValueId = ValueId(11);
const OFFSET: ValueId = ValueId(12);
const VALUE: ValueId = ValueId(13);
const ELEMENT: ValueId = ValueId(14);
const READ: ValueId = ValueId(15);
const UNUSED: ValueId = ValueId(16);
const BAD_OFFSET: ValueId = ValueId(17);
const LIMIT: usize = 10_000_000;
const FLOOR: usize = 97;

fn literal_array_module(length: u64, fault: usize) -> Module {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(COUNT, Type::Scalar(ScalarType::Index)),
            OperationKind::Constant(Constant::Index(length)),
        ),
        Operation::effect_free(
            ValueDef::new(ARRAY, pointer.clone()),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: Some(COUNT),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(OFFSET, Type::Scalar(ScalarType::Index)),
            OperationKind::Constant(Constant::Index(if fault == 1 {
                length
            } else {
                length - 1
            })),
        ),
        Operation::effect_free(
            ValueDef::new(ELEMENT, pointer.clone()),
            OperationKind::GetElementPointer {
                base: ARRAY,
                offset: OFFSET,
            },
        ),
        Operation::effect_free(
            ValueDef::new(VALUE, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: if fault == 4 { ARRAY } else { ELEMENT },
                value: VALUE,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(READ, Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ELEMENT,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(BAD_OFFSET, Type::Scalar(ScalarType::Index)),
            OperationKind::Constant(Constant::Index(if fault == 2 { length } else { 0 })),
        ),
        Operation::effect_free(
            ValueDef::new(UNUSED, pointer),
            OperationKind::GetElementPointer {
                base: ARRAY,
                offset: if fault == 3 { ValueId(0) } else { BAD_OFFSET },
            },
        ),
    ];
    if fault == 5 {
        block.operations.remove(5);
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("literal-array-history");
    module.functions.push(Function::internal_helper(
        "probe",
        Signature::new(vec![Type::Scalar(ScalarType::Index)], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn literal_array_slot(length: u64) -> ScopedSourceSlotV29 {
    ScopedSourceSlotV29 {
        instance: ProductionCallInstanceIdV1(0),
        origin: ScopedSlotOriginV29 {
            identity: ScopedAllocationIdentityV29::LegacyLocal(0),
            source: ScopedAllocationSourceV29::OriginalArray {
                schema: fe2o3_kernel_ir::StorageLayoutIdV1(0),
            },
            semantic_type: SemanticTypeIdV1::from_index(0),
            pointer: ARRAY,
        },
        representation: ScopedSlotRepresentationV29::ScalarArray(ScopedScalarArraySlotV29 {
            element_type: SemanticTypeIdV1::from_index(1),
            element: PrivateRetainedSlotFactsV1 {
                element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
                size: 4,
                alignment: 4,
            },
            length,
            bytes: length.checked_mul(4).unwrap(),
            count: Some((
                COUNT,
                PrivateArrayPhysicalLocationV1 {
                    block_ordinal: 0,
                    block: BlockId(0),
                    operation: 0,
                },
            )),
        }),
        allocation: PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: BlockId(0),
            operation: 1,
        },
    }
}

fn literal_array_history(
    length: u64,
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let module = literal_array_module(length, fault);
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = ArgumentBudgetV1::new(&mut setup_work, LIMIT);
    let (owner, owner_receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut setup,
        )
        .unwrap();
    setup
        .reserve_storage(owner_receipt.retained_storage())
        .unwrap();
    let (inventory, inventory_receipt) =
        CanonicalKirInventoryV18::derive_v18(&owner, &mut setup).unwrap();
    setup
        .reserve_storage(inventory_receipt.retained_storage())
        .unwrap();
    let (foreign_owner, foreign_receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &module,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut setup,
        )
        .unwrap();
    setup
        .reserve_storage(foreign_receipt.retained_storage())
        .unwrap();
    let (foreign_inventory, foreign_inventory_receipt) =
        CanonicalKirInventoryV18::derive_v18(&foreign_owner, &mut setup).unwrap();
    setup
        .reserve_storage(foreign_inventory_receipt.retained_storage())
        .unwrap();
    let function = inventory.functions()[0].function;
    let slots = [literal_array_slot(length)];
    let accesses: Vec<_> = function.body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            )
        })
        .map(|(operation, _)| SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(0),
            operation,
            slot: 0,
        })
        .collect();
    let selected = if fault == 6 {
        vec![SourceIndexLocationV29 {
            source: PendingSourceIndexV29 {
                instance: ProductionCallInstanceIdV1(0),
                event: 0,
                canonical: 0,
                load_anchor: None,
                index_slot: None,
                array_slot: 0,
                original: OFFSET,
                scalar: ScalarType::Index,
                length,
                base: ARRAY,
                offset: OFFSET,
                pointer: ELEMENT,
                block: BlockId(0),
                operation: 3,
            },
            load: None,
        }]
    } else {
        Vec::new()
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
        .solve_pending_indices(&slots, &accesses, &[], budget)?
        .graph;
        let result = scoped_slot_uses_v29::check_expanded_index_addresses_v29(
            function,
            &graph,
            &slots,
            &accesses,
            &[],
            &[],
            &[],
            &selected,
            &[],
            if fault == 7 {
                &foreign_inventory
            } else {
                &inventory
            },
            None,
            CanonicalKirFunctionCoordinateV1(0),
            budget,
        );
        drop(graph);
        result
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn literal_array_history_without_versions_checks_unused_geps_bounds_and_initialized_elements() {
    for length in [2, 4, 1_000_000] {
        let (positive, _, _) = literal_array_history(length, 0, LIMIT, LIMIT);
        assert!(positive.is_ok(), "length={length}: {positive:?}");
    }
    for fault in 1..=7 {
        assert!(literal_array_history(4, 0, LIMIT, LIMIT).0.is_ok());
        let error = literal_array_history(4, fault, LIMIT, LIMIT).0.unwrap_err();
        let expected = match fault {
            1..=3 => "scoped source-slot allocation census is incomplete or mismatched",
            4 | 5 => "scoped slot read is not initialized in its fresh physical activation",
            6 => "selected array history requires shared memory versions",
            7 => "source raw address differs from its actual formation or memory history",
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
fn literal_array_history_exact_and_one_short_limits_preserve_the_outer_floor() {
    let (positive, work, peak) = literal_array_history(4, 0, LIMIT, LIMIT);
    assert!(positive.is_ok(), "{positive:?}");
    assert!(literal_array_history(4, 0, work, peak).0.is_ok());
    for (work_limit, storage_limit, work_failure) in
        [(work - 1, peak, true), (work, peak - 1, false)]
    {
        let error = literal_array_history(4, 0, work_limit, storage_limit)
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

#[test]
fn original_array_geometry_requires_exact_scalar_extent_without_overflow() {
    let slot = literal_array_slot(4);
    assert!(ordinary_original_array_representation_v29(
        slot.origin.source,
        slot.representation
    ));
    assert!(!ordinary_original_array_representation_v29(
        ScopedAllocationSourceV29::Legacy,
        slot.representation
    ));
    let ScopedSlotRepresentationV29::ScalarArray(scalar) = slot.representation else {
        unreachable!()
    };
    for changed in [
        ScopedScalarArraySlotV29 {
            length: 0,
            bytes: 0,
            ..scalar
        },
        ScopedScalarArraySlotV29 {
            bytes: 15,
            ..scalar
        },
        ScopedScalarArraySlotV29 {
            length: u64::MAX,
            bytes: u64::MAX - 3,
            ..scalar
        },
    ] {
        assert!(!ordinary_original_array_representation_v29(
            slot.origin.source,
            ScopedSlotRepresentationV29::ScalarArray(changed)
        ));
    }
    for limit in [1, 0] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_array_geometry_v29(&[slot], false, &mut budget);
        if limit == 1 {
            assert_eq!(result.unwrap(), true);
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Work(_))));
        }
        assert_eq!(budget.storage(), 0);
    }
}
