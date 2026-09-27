use fe2o3_kernel_ir::{StorageLayoutKindV1, StorageLayoutV1};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as ProbeError;
include!("production_pipeline_native_allocation_observation_v18_tests.rs");

fn require_exact_probe_owner<T>(expected: &T, actual: &T) -> Result<(), ProbeError> {
    if !std::ptr::eq(expected, actual) {
        return Err(ProbeError::Binding(
            "native allocation diagnostic owner mismatch",
        ));
    }
    Ok(())
}

fn with_allocation_probe_scope(
    budget: &mut super::Budget<'_>,
    operations: usize,
    action: impl FnOnce(&mut super::Budget<'_>) -> Result<Observation, ProbeError>,
) -> Result<Observation, ProbeError> {
    let capture = std::mem::size_of_val(&action)
        .checked_add(std::mem::align_of_val(&action))
        .ok_or(super::Resource::Arithmetic)?;
    let scratch = allocation_probe_headers()?
        .checked_add(capture)
        .ok_or(super::Resource::Arithmetic)?;
    let work = operations
        .checked_mul(8)
        .and_then(|n| n.checked_add(128))
        .ok_or(super::Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 0, work, scratch, action)
}

fn type_observation(ty: &Type, layouts: &[StorageLayoutV1]) -> Result<TypeObservation, ProbeError> {
    let mut result = TypeObservation {
        kind: TypeTag::Unit,
        scalar: None,
        layout: None,
    };
    result.kind = match ty {
        Type::Unit => TypeTag::Unit,
        Type::Scalar(scalar) => {
            result.scalar = Some((*scalar).into());
            TypeTag::Scalar
        }
        Type::StorageObject(id) => {
            let row = layouts.get(id.0 as usize).ok_or(ProbeError::Binding(
                "native allocation diagnostic layout is absent",
            ))?;
            let (kind, scalar, members) = match &row.kind {
                StorageLayoutKindV1::Scalar(value) => (LayoutTag::Scalar, Some((*value).into()), 0),
                StorageLayoutKindV1::Vector(_) => (LayoutTag::Vector, None, 0),
                StorageLayoutKindV1::Pointer(_) => (LayoutTag::Pointer, None, 0),
                StorageLayoutKindV1::Record(fields) => (LayoutTag::Record, None, fields.len()),
                StorageLayoutKindV1::Union(fields) => (LayoutTag::Union, None, fields.len()),
                StorageLayoutKindV1::Array { .. } => (LayoutTag::Array, None, 1),
                StorageLayoutKindV1::Slice { .. } => (LayoutTag::Slice, None, 2),
                StorageLayoutKindV1::Variants { variants, .. } => {
                    (LayoutTag::Variants, None, variants.len())
                }
            };
            result.layout = Some(LayoutObservation {
                ordinal: id.0,
                kind,
                size: row.size,
                alignment: row.alignment,
                scalar,
                members,
            });
            TypeTag::StorageObject
        }
        Type::Execution(_) => TypeTag::Execution,
        Type::Vector(_) => TypeTag::Vector,
        Type::Pointer(_) => TypeTag::Pointer,
        Type::Slice(_) => TypeTag::Slice,
    };
    Ok(result)
}

fn allocation_observation(
    operation: &fe2o3_kernel_ir::Operation,
    layouts: &[StorageLayoutV1],
) -> Result<Option<AllocationObservation>, ProbeError> {
    let Op::Alloca {
        element,
        count,
        address_space,
        alignment,
    } = &operation.kind
    else {
        return Ok(None);
    };
    let [result] = operation.results.as_slice() else {
        return Err(ProbeError::Binding(
            "native allocation diagnostic result census",
        ));
    };
    let Type::Pointer(pointer) = &result.ty else {
        return Err(ProbeError::Binding(
            "native allocation diagnostic result is not a pointer",
        ));
    };
    Ok(Some(AllocationObservation {
        element: type_observation(element, layouts)?,
        address_space: (*address_space).into(),
        alignment: *alignment,
        count: count.map(|id| id.0),
        result: result.id.0,
        result_address_space: pointer.address_space.into(),
        result_access: pointer.access.into(),
        result_pointee: type_observation(&pointer.pointee, layouts)?,
        source: None,
    }))
}

fn allocation_source_observation(
    original: &super::Original<'_>,
    optimized: &super::Optimized<'_>,
    output: Coordinate,
    budget: &mut super::Budget<'_>,
) -> Result<Option<AllocationSourceObservation>, ProbeError> {
    let source = original.source(budget)?;
    let inventory = optimized.input_inventory(budget)?;
    let mut found = None;
    for root in 0..source.root_count(budget)? {
        let (_, function) = source.root(root, budget)?;
        let row = inventory
            .functions()
            .get(function)
            .ok_or(ProbeError::Binding(
                "native allocation diagnostic original root",
            ))?;
        for operation in &inventory.operations()[row.operations.clone()] {
            budget.charge_work(1)?;
            if !matches!(operation.operation.kind, Op::Alloca { .. }) {
                continue;
            }
            let Some(allocation) = optimized.allocation(root, operation.coordinate, budget)? else {
                continue;
            };
            if allocation.output() != Some(output) {
                continue;
            }
            if found.is_some() {
                return Err(ProbeError::Binding(
                    "native allocation diagnostic source is ambiguous",
                ));
            }
            let (function, _) = source.instance(root, allocation.instance(), budget)?;
            let input = allocation.input();
            let owner = source.source_ssa(budget)?;
            found = Some(AllocationSourceObservation {
                root,
                instance: allocation.instance(),
                function: function.index(),
                input: [input.block.function.0, input.block.block, input.operation],
                semantic_sha256: *owner.source_semantic_sha256(),
                private_slot_identity_available: false,
            });
        }
    }
    Ok(found)
}

fn allocation_probe_headers() -> Result<usize, super::Resource> {
    use std::mem::size_of;
    fn h<T>() -> Result<usize, super::Resource> {
        size_of::<Result<T, ProbeError>>()
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<T>()))
            .ok_or(super::Resource::Arithmetic)
    }
    let mut total = 0usize;
    for bytes in [
        h::<Observation>()?,
        h::<Option<AllocationObservation>>()?,
        h::<AllocationObservation>()?,
        h::<()>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        h::<TypeObservation>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        h::<LayoutObservation>()?,
        h::<Option<LayoutObservation>>()?,
        h::<(LayoutTag, Option<ScalarTag>, usize)>()?,
        h::<Option<ScalarTag>>()?,
        h::<Option<AllocationSourceObservation>>()?,
        h::<AllocationSourceObservation>()?,
        h::<&mut AllocationObservation>()?,
        h::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceAllocationV18<'_>>()?,
        h::<Option<fe2o3_lower_mir_kernel::ProductionOptimizedSourceAllocationV18<'_>>>()?,
        h::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?
            .checked_mul(3)
            .ok_or(super::Resource::Arithmetic)?,
        h::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()?,
        h::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()?,
        h::<std::ops::Range<usize>>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        h::<Option<usize>>()?,
        h::<usize>()?,
        h::<(
            fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
            usize,
        )>()?,
        h::<(
            fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
            Option<(usize, fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1)>,
        )>()?,
        h::<&StorageLayoutV1>()?,
        h::<Option<&StorageLayoutV1>>()?,
        h::<&StorageLayoutKindV1>()?,
        h::<&[StorageLayoutV1]>()?,
        h::<&Box<[fe2o3_kernel_ir::StorageFieldV1]>>()?,
        h::<&Box<[fe2o3_kernel_ir::StorageVariantV1]>>()?,
        h::<&fe2o3_kernel_ir::ScalarType>()?,
        h::<&fe2o3_kernel_ir::StorageLayoutIdV1>()?,
        h::<&[fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>]>()?,
        h::<&[fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>]>()?,
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&fe2o3_kernel_ir::Module>()?,
        h::<&fe2o3_kernel_ir::Operation>()?,
        h::<&Op>()?,
        h::<&Option<fe2o3_kernel_ir::ValueId>>()?,
        h::<Option<fe2o3_kernel_ir::ValueId>>()?,
        h::<&fe2o3_kernel_ir::AddressSpace>()?,
        h::<&u32>()?,
        h::<Option<(Coordinate, Need)>>()?,
        h::<Option<Need>>()?,
        h::<Option<&'static str>>()?,
        h::<&fe2o3_kernel_ir::PointerType>()?,
        h::<&fe2o3_kernel_ir::ValueDef>()?,
        h::<&[fe2o3_kernel_ir::ValueDef]>()?,
        h::<Option<u32>>()?,
        h::<Coordinate>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        h::<&Type>()?
            .checked_mul(2)
            .ok_or(super::Resource::Arithmetic)?,
        size_of::<std::thread::Result<Result<Observation, ProbeError>>>(),
    ] {
        total = total
            .checked_add(bytes)
            .ok_or(super::Resource::Arithmetic)?;
    }
    Ok(total)
}

#[test]
fn native_allocation_detail_uses_exact_selected_layout_and_pointer() {
    use fe2o3_kernel_ir::{Operation, StorageFieldV1, StorageLayoutIdV1, ValueDef, ValueId};
    let layouts = [
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: StorageLayoutIdV1(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    for element in [
        Type::Scalar(ScalarType::U32),
        Type::StorageObject(StorageLayoutIdV1(0)),
        Type::StorageObject(StorageLayoutIdV1(1)),
    ] {
        let operation = Operation::new(
            vec![ValueDef::new(
                ValueId(31),
                Type::pointer(
                    element.clone(),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            Op::Alloca {
                element: element.clone(),
                count: Some(ValueId(17)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        );
        let detail = allocation_observation(&operation, &layouts)
            .unwrap()
            .unwrap();
        assert_eq!(
            (detail.result, detail.count, detail.alignment),
            (31, Some(17), 4)
        );
        assert_eq!(
            (
                detail.address_space,
                detail.result_address_space,
                detail.result_access
            ),
            (SpaceTag::Private, SpaceTag::Private, AccessTag::ReadWrite)
        );
        assert_eq!(detail.element, detail.result_pointee);
        assert!(detail.source.is_none());
        detail.validate_shape().unwrap();
        match element {
            Type::Scalar(_) => assert_eq!(
                (
                    detail.element.kind,
                    detail.element.scalar,
                    detail.element.layout
                ),
                (TypeTag::Scalar, Some(ScalarTag::U32), None)
            ),
            Type::StorageObject(id) => {
                let row = detail.element.layout.unwrap();
                assert_eq!(
                    (row.ordinal, row.size, row.alignment),
                    (id.0, layouts[id.0 as usize].size, 4)
                );
                assert_eq!(
                    row.kind,
                    if id.0 == 0 {
                        LayoutTag::Scalar
                    } else {
                        LayoutTag::Record
                    }
                );
                assert_eq!(row.members, if id.0 == 0 { 0 } else { 2 });
            }
            _ => unreachable!(),
        }
    }
    let malformed = Operation::new(
        vec![],
        Op::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    );
    assert!(matches!(
        allocation_observation(&malformed, &layouts),
        Err(ProbeError::Binding(
            "native allocation diagnostic result census"
        ))
    ));
    assert!(matches!(
        type_observation(&Type::StorageObject(StorageLayoutIdV1(2)), &layouts),
        Err(ProbeError::Binding(
            "native allocation diagnostic layout is absent"
        ))
    ));
    let non_alloca = Operation::new(vec![], Op::Execution(Execution::ContextIssue));
    assert_eq!(allocation_observation(&non_alloca, &layouts).unwrap(), None);
}

#[test]
fn native_allocation_detail_owner_comparison_is_identity_not_content() {
    let owner = [3u32, 5];
    let copied = owner;
    require_exact_probe_owner(&owner, &owner).unwrap();
    assert!(matches!(
        require_exact_probe_owner(&owner, &copied),
        Err(ProbeError::Binding(
            "native allocation diagnostic owner mismatch"
        ))
    ));
}

#[test]
fn native_allocation_diagnostic_scope_has_exact_header_work_cuts_and_cleanup() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    fn empty(_: &mut super::Budget<'_>) -> Result<Observation, ProbeError> {
        Ok(Observation {
            first_unresolved: None,
            first_unresolved_kind: None,
            first_unresolved_allocation: None,
            lifecycle: 0,
            operation_count: 3,
            consumers: 0,
            native_entries: 0,
            private_memory: false,
        })
    }
    let action: fn(&mut super::Budget<'_>) -> Result<Observation, ProbeError> = empty;
    let headers = allocation_probe_headers().unwrap()
        + std::mem::size_of_val(&action)
        + std::mem::align_of_val(&action);
    let required_work = 128 + 8 * 3;
    let mut exact = Account::new(Work::new(required_work), 13 + headers);
    let row = exact
        .with_budget(|budget| {
            budget.reserve_storage(13)?;
            with_allocation_probe_scope(budget, 3, action)
        })
        .unwrap();
    assert_eq!(row.operation_count, 3);
    assert_eq!(
        (exact.work(), exact.storage(), exact.peak_storage()),
        (required_work, 13, 13 + headers)
    );
    for storage_short in [false, true] {
        let mut denied = Account::new(
            Work::new(required_work - usize::from(!storage_short)),
            13 + headers - usize::from(storage_short),
        );
        let refused = denied.with_budget(|budget| {
            budget.reserve_storage(13)?;
            with_allocation_probe_scope(budget, 3, action)
        });
        match refused.unwrap_err() {
            ProbeError::Resource(super::Resource::Storage(limit)) if storage_short => assert_eq!(
                (limit.actual(), limit.limit()),
                (13 + headers, 12 + headers)
            ),
            ProbeError::Resource(super::Resource::Work(limit)) if !storage_short => assert_eq!(
                (limit.actual(), limit.limit()),
                (required_work, required_work - 1)
            ),
            other => panic!("wrong diagnostic cut: {other:?}"),
        }
        assert_eq!(denied.storage(), 13);
    }
    let mut error = Account::new(Work::new(required_work), 13 + headers);
    let refused = error.with_budget(|budget| {
        budget.reserve_storage(13)?;
        with_allocation_probe_scope(budget, 3, |_| {
            Err(ProbeError::Binding("selected diagnostic error"))
        })
    });
    assert!(matches!(
        refused,
        Err(ProbeError::Binding("selected diagnostic error"))
    ));
    assert_eq!(error.storage(), 13);
    let mut unwind = Account::new(Work::new(required_work), 13 + headers);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        unwind.with_budget(|budget| {
            budget.reserve_storage(13)?;
            with_allocation_probe_scope(budget, 3, |_| std::panic::panic_any(73u32))
        })
    }));
    assert_eq!(*caught.unwrap_err().downcast::<u32>().unwrap(), 73);
    assert_eq!(unwind.storage(), 13);
}
