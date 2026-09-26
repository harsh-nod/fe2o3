use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLayoutIdentityV1, SemanticPointerTypeV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

fn scalar() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn array(
    element: u32,
    length: u64,
    size: u64,
    alignment: u64,
    stride: u64,
    count: u64,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size,
            alignment,
            SemanticFieldsShapeV1::array(stride, count),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            alignment,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(element),
            length,
        },
    )
}

#[test]
fn exact_array_layout_and_element_limit_are_independent_of_ssa_component_limits() {
    let ty = SemanticTypeIdV1::from_index(1);
    let types = [scalar(), array(0, 512, 2048, 4, 4, 512)];
    let slot = retained_array_slot_plan_v1(&types, ty, 512).unwrap();
    let (kernel_type, alignment, array) = slot.storage.scalar_array().unwrap();
    assert_eq!(*kernel_type, Type::Scalar(ScalarType::U32));
    assert_eq!(alignment, 4);
    assert_eq!(
        array,
        Some(SemanticRetainedArrayLayoutV1 {
            element: SemanticTypeIdV1::from_index(0),
            length: 512,
        })
    );
    assert!(matches!(
        retained_array_slot_plan_v1(&types, ty, 511),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 512,
            limit: 511,
        })
    ));
}

#[test]
fn array_plan_rejects_mismatched_rustc_count_stride_size_and_alignment() {
    let ty = SemanticTypeIdV1::from_index(1);
    for invalid in [
        array(0, 8, 32, 4, 4, 7),
        array(0, 8, 64, 4, 8, 8),
        array(0, 8, 36, 4, 4, 8),
        array(0, 8, 32, 8, 4, 8),
        array(0, 0, 0, 4, 4, 0),
    ] {
        assert!(matches!(
            retained_array_slot_plan_v1(&[scalar(), invalid], ty, 512),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "retained array has no exact nonempty fixed element layout",
                ..
            })
        ));
    }
    assert!(matches!(
        retained_array_slot_plan_v1(&[scalar(), array(0, u64::MAX, 4, 4, 4, 1)], ty, usize::MAX,),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array byte extent overflows",
            ..
        })
    ));
}

#[test]
fn arrays_admit_thin_pointer_elements_without_erasing_address_space_or_access() {
    let pointer = |metadata, size| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::new(Some(size), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(0),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    1,
                    64,
                    metadata,
                )
                .unwrap(),
            ),
        )
    };
    let ty = SemanticTypeIdV1::from_index(2);
    let types = [
        scalar(),
        pointer(SemanticPointerMetadataV1::None, 8),
        array(1, 8, 64, 8, 8, 8),
    ];
    let slot = retained_array_slot_plan_v1(&types, ty, 8).unwrap();
    assert_eq!(
        *slot.storage.scalar_array().unwrap().0,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )
    );
    assert!(matches!(
        retained_array_slot_plan_v1(
            &[
                scalar(),
                pointer(SemanticPointerMetadataV1::SliceLength, 16),
                array(1, 8, 128, 8, 16, 8)
            ],
            ty,
            8,
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array element is not an exact storable scalar or metadata-free pointer",
            ..
        })
    ));
}

#[test]
fn nested_array_elements_are_not_reinterpreted_as_scalar_storage() {
    let types = [
        scalar(),
        array(0, 8, 32, 4, 4, 8),
        array(1, 2, 64, 4, 32, 2),
    ];
    assert!(matches!(
        retained_array_slot_plan_v1(&types, SemanticTypeIdV1::from_index(2), 512),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array element is not an exact storable scalar or metadata-free pointer",
            ..
        })
    ));
}

#[test]
fn aggregate_expansion_checks_exact_prefixed_operation_limits_before_capacity() {
    // 8 * (Index + GEP + Load/Store), plus five operations already emitted.
    assert_eq!(retained_array_expansion_v1(8, 5, 4, 29).unwrap(), 8);
    assert!(matches!(
        retained_array_expansion_v1(8, 5, 4, 28),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 29,
            limit: 28,
        })
    ));
    assert_eq!(
        retained_array_expansion_v1(8, 5, MAX_BLOCK_OPERATIONS_V1 - 24, 29).unwrap(),
        8,
    );
    assert!(matches!(
        retained_array_expansion_v1(8, 5, MAX_BLOCK_OPERATIONS_V1 - 23, 29),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations, actual, limit,
        }) if actual == MAX_BLOCK_OPERATIONS_V1 + 1 && limit == MAX_BLOCK_OPERATIONS_V1
    ));
    assert!(retained_array_expansion_v1(u64::MAX, 0, 0, usize::MAX).is_err());
    assert!(retained_array_expansion_v1(8, usize::MAX, 0, usize::MAX).is_err());
}

fn optional_array_probe_slots_v29(
    mode: u8,
) -> BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1> {
    let scalar = |array| SemanticRetainedLocalSlotV1 {
        pointer: ValueId(41),
        semantic_type: SemanticTypeIdV1::from_index(1),
        storage: SemanticRetainedStorageV29::ScalarArray {
            kernel_type: Type::Scalar(ScalarType::U32),
            alignment: 4,
            array,
        },
    };
    let object = || SemanticRetainedLocalSlotV1 {
        pointer: ValueId(42),
        semantic_type: SemanticTypeIdV1::from_index(2),
        storage: SemanticRetainedStorageV29::Object {
            cell: 3,
            schema: fe2o3_kernel_ir::StorageLayoutIdV1(9),
            bytes: 16,
            alignment: 4,
        },
    };
    let mut slots = BTreeMap::new();
    if matches!(mode, 0 | 3 | 6) {
        slots.insert(ScopedAllocationIdentityV29::OriginalObject {
            local: if mode == 6 { 8 } else { 7 }, generation: 3,
        }, object());
    }
    if matches!(mode, 1 | 3 | 6) {
        slots.insert(ScopedAllocationIdentityV29::LegacyLocal(7), scalar(Some(
            SemanticRetainedArrayLayoutV1 {
                element: SemanticTypeIdV1::from_index(0), length: 4,
            },
        )));
    }
    if mode == 2 {
        slots.insert(ScopedAllocationIdentityV29::LegacyLocal(7), scalar(None));
    }
    if mode == 4 {
        slots.insert(ScopedAllocationIdentityV29::LegacyLocal(7), object());
    }
    assert!(mode <= 6);
    slots
}

fn assert_optional_array_probe_v29(
    mode: u8,
    slots: &BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>,
    result: Result<Option<&SemanticRetainedLocalSlotV1>, ProductionSemanticKirErrorV1>,
) {
    match mode {
        0 | 2 | 5 => assert!(matches!(result, Ok(None))),
        1 | 6 => {
            let actual = result.unwrap().expect("the exact legacy array must be selected");
            assert!(std::ptr::eq(actual, slots.get(&ScopedAllocationIdentityV29::LegacyLocal(7)).unwrap()));
            assert_eq!(actual.pointer, ValueId(41));
        }
        3 | 4 => assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0, block: None, statement: None,
            detail: "typed allocation identity or representation requires its exact source contract",
        }))),
        _ => unreachable!(),
    }
}

#[test]
fn optional_array_classification_preserves_strict_legacy_consumption() {
    for mode in 0..7 {
        // Inert map fixtures exercise classification only, not source admission.
        let slots = optional_array_probe_slots_v29(mode);
        assert_optional_array_probe_v29(mode, &slots,
            lookup_optional_retained_array_v29(&slots, 7, None));
        assert!(matches!(lookup_optional_retained_array_v29(&slots, 9, None), Ok(None)));
        if mode == 0 {
            assert!(matches!(lookup_legacy_retained_slot_v29(&slots, 7, None),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0, block: None, statement: None,
                    detail: "typed allocation identity or representation requires its exact source contract",
                })));
        }
    }
}

#[test]
fn optional_array_classification_prepays_exact_lookup_work_without_storage() {
    for mode in 0..7 {
        let slots = optional_array_probe_slots_v29(mode);
        // The existing tree lookup contract charges 16 units per search level.
        // Optional presence adds one search and one branch; selecting a legacy
        // key then pays both original strict searches and their two checks.
        let lookup = (slots.len().checked_ilog2().unwrap_or(0) as usize + 2) * 16;
        let selected = matches!(mode, 1 | 2 | 3 | 4 | 6);
        let exact = if selected { 3 * lookup + 3 } else { lookup + 1 };
        for limit in [0, exact - 1, exact] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = lookup_optional_retained_array_v29(&slots, 7, Some(&mut budget));
            if limit == exact {
                assert_optional_array_probe_v29(mode, &slots, result);
                assert_eq!(budget.work(), exact);
                assert_eq!(budget.failed_work(), None);
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_),
                ))));
                assert!(budget.work() <= limit);
                assert_eq!(budget.failed_work(), Some(if limit == 0 { lookup } else { exact }));
            }
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}
