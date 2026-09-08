fn execution_type_identity_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<ExecutionTypeIdentityV1, ProductionSemanticKirErrorV1> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(|| unsupported(0, None, None, "execution-capability type is missing"))?;
    let identity = ExecutionTypeIdentityV1::new(*declaration.identity().as_bytes());
    if !identity.is_complete() {
        return Err(unsupported(
            0,
            None,
            None,
            "execution-capability type identity is incomplete",
        ));
    }
    Ok(identity)
}

fn execution_element_layout_v1(
    types: &[SemanticTypeDeclV1],
    element: SemanticTypeIdV1,
) -> Result<ExecutionElementLayoutV1, ProductionSemanticKirErrorV1> {
    let layout = types
        .get(element.index() as usize)
        .ok_or_else(|| unsupported(0, None, None, "execution element type is missing"))?
        .layout();
    let byte_size = layout
        .size_bytes()
        .and_then(|size| u32::try_from(size).ok())
        .filter(|size| *size != 0)
        .ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "execution element has no bounded nonzero layout",
            )
        })?;
    let byte_alignment = u16::try_from(layout.alignment_bytes())
        .ok()
        .filter(|alignment| *alignment != 0 && alignment.is_power_of_two())
        .ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "execution element alignment is unsupported",
            )
        })?;
    let layout = ExecutionElementLayoutV1 {
        byte_size,
        byte_alignment,
    };
    if !layout.is_complete() {
        return Err(unsupported(
            0,
            None,
            None,
            "execution element layout is incomplete",
        ));
    }
    Ok(layout)
}

fn execution_scalar_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<ScalarType, ProductionSemanticKirErrorV1> {
    lower_scalar_type(types, ty)?
        .as_scalar()
        .ok_or_else(|| unsupported(0, None, None, "execution value is not a supported scalar"))
}

const fn lower_execution_scope_v1(
    scope: SemanticExecutionMemoryScopeV1,
) -> ExecutionMemoryScopeV1 {
    match scope {
        SemanticExecutionMemoryScopeV1::System => ExecutionMemoryScopeV1::System,
        SemanticExecutionMemoryScopeV1::Device => ExecutionMemoryScopeV1::Device,
        SemanticExecutionMemoryScopeV1::Workgroup => ExecutionMemoryScopeV1::Workgroup,
        SemanticExecutionMemoryScopeV1::Subgroup => ExecutionMemoryScopeV1::Subgroup,
    }
}

const fn lower_execution_ordering_v1(
    ordering: SemanticExecutionMemoryOrderingV1,
) -> ExecutionMemoryOrderingV1 {
    match ordering {
        SemanticExecutionMemoryOrderingV1::Relaxed => ExecutionMemoryOrderingV1::Relaxed,
        SemanticExecutionMemoryOrderingV1::Acquire => ExecutionMemoryOrderingV1::Acquire,
        SemanticExecutionMemoryOrderingV1::Release => ExecutionMemoryOrderingV1::Release,
        SemanticExecutionMemoryOrderingV1::AcquireRelease => {
            ExecutionMemoryOrderingV1::AcquireRelease
        }
        SemanticExecutionMemoryOrderingV1::SequentiallyConsistent => {
            ExecutionMemoryOrderingV1::SequentiallyConsistent
        }
    }
}

const fn lower_execution_spaces_v1(
    spaces: SemanticExecutionMemorySpacesV1,
) -> ExecutionMemorySpacesV1 {
    match spaces {
        SemanticExecutionMemorySpacesV1::Global => ExecutionMemorySpacesV1::Global,
        SemanticExecutionMemorySpacesV1::Workgroup => ExecutionMemorySpacesV1::Workgroup,
        SemanticExecutionMemorySpacesV1::GlobalAndWorkgroup => {
            ExecutionMemorySpacesV1::GlobalAndWorkgroup
        }
    }
}

const fn lower_execution_semantics_v1(
    semantics: SemanticExecutionMemorySemanticsV1,
) -> ExecutionMemorySemanticsV1 {
    ExecutionMemorySemanticsV1 {
        scope: lower_execution_scope_v1(semantics.scope()),
        ordering: lower_execution_ordering_v1(semantics.ordering()),
        spaces: lower_execution_spaces_v1(semantics.spaces()),
    }
}

const fn lower_execution_address_space_v1(
    space: SemanticExecutionMemoryAddressSpaceV1,
) -> ExecutionMemoryAddressSpaceV1 {
    match space {
        SemanticExecutionMemoryAddressSpaceV1::Private => ExecutionMemoryAddressSpaceV1::Private,
        SemanticExecutionMemoryAddressSpaceV1::Workgroup => {
            ExecutionMemoryAddressSpaceV1::Workgroup
        }
        SemanticExecutionMemoryAddressSpaceV1::Global => ExecutionMemoryAddressSpaceV1::Global,
    }
}

const fn lower_execution_access_v1(
    access: SemanticExecutionMemoryAccessV1,
) -> ExecutionMemoryAccessV1 {
    match access {
        SemanticExecutionMemoryAccessV1::ReadOnly => ExecutionMemoryAccessV1::ReadOnly,
        SemanticExecutionMemoryAccessV1::ExclusiveReadWrite => {
            ExecutionMemoryAccessV1::ExclusiveReadWrite
        }
        SemanticExecutionMemoryAccessV1::DisjointWrite => ExecutionMemoryAccessV1::DisjointWrite,
        SemanticExecutionMemoryAccessV1::AtomicReadWrite => {
            ExecutionMemoryAccessV1::AtomicReadWrite
        }
    }
}

const fn lower_execution_atomic_kind_v1(
    kind: SemanticExecutionAtomicKindV1,
) -> ExecutionAtomicKindV1 {
    match kind {
        SemanticExecutionAtomicKindV1::BindGlobalLocation => {
            ExecutionAtomicKindV1::BindGlobalLocation
        }
        SemanticExecutionAtomicKindV1::Load => ExecutionAtomicKindV1::Load,
        SemanticExecutionAtomicKindV1::Store => ExecutionAtomicKindV1::Store,
        SemanticExecutionAtomicKindV1::FetchAdd => ExecutionAtomicKindV1::FetchAdd,
        SemanticExecutionAtomicKindV1::CompareExchange => ExecutionAtomicKindV1::CompareExchange,
        SemanticExecutionAtomicKindV1::BindGlobalView => ExecutionAtomicKindV1::BindGlobalView,
    }
}

const fn lower_execution_collective_kind_v1(
    kind: SemanticExecutionCollectiveKindV1,
) -> ExecutionCollectiveKindV1 {
    match kind {
        SemanticExecutionCollectiveKindV1::ReduceSum => ExecutionCollectiveKindV1::ReduceSum,
        SemanticExecutionCollectiveKindV1::InclusiveScanSum => {
            ExecutionCollectiveKindV1::InclusiveScanSum
        }
        SemanticExecutionCollectiveKindV1::ExclusiveScanSum => {
            ExecutionCollectiveKindV1::ExclusiveScanSum
        }
    }
}

fn lower_execution_operation_v1(
    types: &[SemanticTypeDeclV1],
    operation: SemanticExecutionCapabilityOperationV1,
    dynamic_extent: Option<ExecutionDynamicExtentV1>,
) -> Result<ExecutionCapabilityOperationV1, ProductionSemanticKirErrorV1> {
    let id = |ty| execution_type_identity_v1(types, ty);
    let layout = |element| execution_element_layout_v1(types, element);
    use SemanticExecutionCapabilityOperationV1 as Op;
    Ok(match operation {
        Op::WorkgroupDerive { context, workgroup } => {
            ExecutionCapabilityOperationV1::WorkgroupDerive {
                context: id(context)?,
                workgroup: id(workgroup)?,
            }
        }
        Op::SubgroupDerive {
            workgroup,
            subgroup,
            width,
        } => ExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: id(workgroup)?,
            subgroup: id(subgroup)?,
            width,
        },
        Op::LdsAllocate {
            workgroup,
            lds,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: id(workgroup)?,
            lds: id(lds)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::LdsInitializeByInvocation {
            input_lds,
            workgroup,
            output_lds,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
            input_lds: id(input_lds)?,
            workgroup: id(workgroup)?,
            output_lds: id(output_lds)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::LdsPublish {
            input_workgroup,
            input_lds,
            output_lds,
            transition,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::LdsPublish {
            input_workgroup: id(input_workgroup)?,
            input_lds: id(input_lds)?,
            output_lds: id(output_lds)?,
            transition: id(transition)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::LdsReadPublished {
            lds_reference,
            lds,
            workgroup,
            index,
            option,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::LdsReadPublished {
            lds_reference: id(lds_reference)?,
            lds: id(lds)?,
            workgroup: id(workgroup)?,
            index: id(index)?,
            option: id(option)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::WorkgroupBarrier {
            input_workgroup,
            output_workgroup,
            semantics,
        } => ExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup: id(input_workgroup)?,
            output_workgroup: id(output_workgroup)?,
            semantics: lower_execution_semantics_v1(semantics),
        },
        Op::SubgroupBarrier {
            input_workgroup,
            semantics,
            subgroup,
            transition,
            width,
        } => ExecutionCapabilityOperationV1::SubgroupBarrier {
            input_workgroup: id(input_workgroup)?,
            semantics: lower_execution_semantics_v1(semantics),
            subgroup: id(subgroup)?,
            transition: id(transition)?,
            width,
        },
        Op::WorkgroupFence {
            workgroup,
            result,
            semantics,
        } => ExecutionCapabilityOperationV1::WorkgroupFence {
            workgroup: id(workgroup)?,
            result: id(result)?,
            semantics: lower_execution_semantics_v1(semantics),
        },
        Op::SubgroupFence {
            semantics,
            subgroup_reference,
            subgroup,
            epoch,
            result,
            width,
        } => ExecutionCapabilityOperationV1::SubgroupFence {
            semantics: lower_execution_semantics_v1(semantics),
            subgroup_reference: id(subgroup_reference)?,
            subgroup: id(subgroup)?,
            epoch: id(epoch)?,
            result: id(result)?,
            width,
        },
        Op::Atomic {
            kind,
            authority,
            location_input,
            location,
            element,
            operand,
            replacement,
            result,
            address_space,
            scope,
            success,
            failure,
        } => ExecutionCapabilityOperationV1::Atomic {
            kind: lower_execution_atomic_kind_v1(kind),
            authority: id(authority)?,
            location_input: id(location_input)?,
            location: id(location)?,
            element: id(element)?,
            operand: operand.map(id).transpose()?,
            replacement: replacement.map(id).transpose()?,
            result: id(result)?,
            value_type: execution_scalar_v1(types, element)?,
            address_space: lower_execution_address_space_v1(address_space),
            scope: lower_execution_scope_v1(scope),
            success: success.map(lower_execution_ordering_v1),
            failure: failure.map(lower_execution_ordering_v1),
        },
        Op::WorkgroupCollective {
            kind,
            input_workgroup,
            scratch,
            element,
            transition,
            elements,
        } => ExecutionCapabilityOperationV1::WorkgroupCollective {
            kind: lower_execution_collective_kind_v1(kind),
            input_workgroup: id(input_workgroup)?,
            scratch: id(scratch)?,
            element: id(element)?,
            transition: id(transition)?,
            value_type: execution_scalar_v1(types, element)?,
            layout: layout(element)?,
            elements,
        },
        Op::SubgroupCollective {
            kind,
            subgroup_reference,
            subgroup,
            epoch,
            element,
            width,
        } => ExecutionCapabilityOperationV1::SubgroupCollective {
            kind: lower_execution_collective_kind_v1(kind),
            subgroup_reference: id(subgroup_reference)?,
            subgroup: id(subgroup)?,
            epoch: id(epoch)?,
            element: id(element)?,
            value_type: execution_scalar_v1(types, element)?,
            width,
        },
        Op::MatrixAccess {
            subgroup,
            epoch,
            matrix,
            subgroup_brand,
            width,
        } => ExecutionCapabilityOperationV1::MatrixAccess {
            subgroup: id(subgroup)?,
            epoch: id(epoch)?,
            matrix: id(matrix)?,
            subgroup_brand: *subgroup_brand.as_bytes(),
            width,
        },
        Op::AsyncCopy {
            workgroup,
            source_reference,
            source,
            index,
            destination,
            pending,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::AsyncCopy {
            workgroup: id(workgroup)?,
            source_reference: id(source_reference)?,
            source: id(source)?,
            index: id(index)?,
            destination: id(destination)?,
            pending: id(pending)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::AsyncWait {
            input_workgroup,
            pending,
            output_lds,
            transition,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::AsyncWait {
            input_workgroup: id(input_workgroup)?,
            pending: id(pending)?,
            output_lds: id(output_lds)?,
            transition: id(transition)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::RawMemoryBind {
            authority,
            pointer,
            length,
            view,
            element,
            space,
            access,
            index_space,
            atomic_scope,
            unsafe_obligation,
        } => ExecutionCapabilityOperationV1::RawMemoryBind {
            authority: id(authority)?,
            pointer: id(pointer)?,
            length: id(length)?,
            extent: dynamic_extent.ok_or_else(|| {
                unsupported(0, None, None, "raw-memory bind lacks a checked dynamic extent")
            })?,
            view: id(view)?,
            element: id(element)?,
            layout: layout(element)?,
            space: lower_execution_address_space_v1(space),
            access: lower_execution_access_v1(access),
            index_space: index_space.map(id).transpose()?,
            atomic_scope: atomic_scope.map(lower_execution_scope_v1),
            unsafe_obligation: id(unsafe_obligation)?,
        },
        Op::PrivateMemoryAllocate {
            context,
            view,
            element,
            elements,
        } => ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
            context: id(context)?,
            view: id(view)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
        },
        Op::WorkgroupMemoryIndex { workgroup, witness } => {
            ExecutionCapabilityOperationV1::WorkgroupMemoryIndex {
                workgroup: id(workgroup)?,
                witness: id(witness)?,
            }
        }
        Op::WorkgroupMemoryAllocate {
            workgroup,
            view,
            element,
            elements,
            index_space,
        } => ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
            workgroup: id(workgroup)?,
            view: id(view)?,
            element: id(element)?,
            layout: layout(element)?,
            elements,
            index_space: id(index_space)?,
        },
        Op::WorkgroupMemoryPublish {
            input_workgroup,
            input_view,
            output_view,
            transition,
            element,
        } => ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
            input_workgroup: id(input_workgroup)?,
            input_view: id(input_view)?,
            output_view: id(output_view)?,
            transition: id(transition)?,
            element: id(element)?,
            layout: layout(element)?,
        },
        Op::MemoryLoad {
            view,
            workgroup,
            index,
            option,
            element,
            space,
            access,
        } => ExecutionCapabilityOperationV1::MemoryLoad {
            view: id(view)?,
            workgroup: workgroup.map(id).transpose()?,
            index: id(index)?,
            option: id(option)?,
            element: id(element)?,
            layout: layout(element)?,
            space: lower_execution_address_space_v1(space),
            access: lower_execution_access_v1(access),
        },
        Op::MemoryStore {
            view,
            workgroup,
            index,
            element,
            result,
            space,
            access,
        } => ExecutionCapabilityOperationV1::MemoryStore {
            view: id(view)?,
            workgroup: workgroup.map(id).transpose()?,
            index: id(index)?,
            element: id(element)?,
            layout: layout(element)?,
            result: id(result)?,
            space: lower_execution_address_space_v1(space),
            access: lower_execution_access_v1(access),
        },
    })
}

fn execution_scalar_maximum_v1(ty: ScalarType) -> Option<u64> {
    match ty {
        ScalarType::U8 => Some(u8::MAX.into()),
        ScalarType::U16 => Some(u16::MAX.into()),
        ScalarType::U32 => Some(u32::MAX.into()),
        ScalarType::U64 | ScalarType::Index => Some(u64::MAX),
        ScalarType::I8 => Some(i8::MAX as u64),
        ScalarType::I16 => Some(i16::MAX as u64),
        ScalarType::I32 => Some(i32::MAX as u64),
        ScalarType::I64 => Some(i64::MAX as u64),
        ScalarType::Bool
        | ScalarType::I128
        | ScalarType::U128
        | ScalarType::F16
        | ScalarType::Bf16
        | ScalarType::F32
        | ScalarType::F64 => None,
    }
}

fn execution_extent_ceiling_v1(
    object_size_bound_bytes: u64,
    layout: ExecutionElementLayoutV1,
    value_type: ScalarType,
) -> Option<u64> {
    let scalar_maximum = execution_scalar_maximum_v1(value_type)?;
    object_size_bound_bytes
        .checked_sub(1)
        .map(|bytes| bytes / u64::from(layout.byte_size))
        .map(|elements| elements.min(scalar_maximum))
        .filter(|elements| *elements != 0)
}

fn execution_result_types_v1(
    types: &[SemanticTypeDeclV1],
    operation: &ExecutionCapabilityOperationV1,
    provenance: &ExecutionCapabilityProvenanceV1,
    workgroup_brand: Option<[u8; 32]>,
    epoch_before: Option<[u8; 32]>,
    epoch_after: Option<[u8; 32]>,
    operand_types: &[Type],
    target_object_size_bound_bytes: u64,
) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
    let result_epoch = if operation.is_kernel_scoped() {
        None
    } else {
        epoch_after.or(epoch_before)
    };
    let cap = |source_type, role| {
        Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type,
            provenance: provenance.clone(),
            workgroup_brand,
            epoch: result_epoch,
            role,
        })
    };
    let lds = |source_type, element, layout, elements, state| {
        cap(
            source_type,
            ExecutionCapabilityRoleV1::Lds {
                element,
                layout,
                elements,
                state,
            },
        )
    };
    use ExecutionCapabilityOperationV1 as Op;
    Ok(match operation {
        Op::WorkgroupDerive { workgroup, .. } => {
            vec![cap(*workgroup, ExecutionCapabilityRoleV1::Workgroup)]
        }
        Op::SubgroupDerive {
            subgroup, width, ..
        } => vec![cap(
            *subgroup,
            ExecutionCapabilityRoleV1::Subgroup { width: *width },
        )],
        Op::LdsAllocate {
            lds: lds_source,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *lds_source,
            *element,
            *layout,
            *elements,
            ExecutionLdsStateV1::Uninitialized,
        )],
        Op::LdsInitializeByInvocation {
            output_lds,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *output_lds,
            *element,
            *layout,
            *elements,
            ExecutionLdsStateV1::InvocationInitialized,
        )],
        Op::LdsPublish {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *output_lds,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Published,
            ),
        ],
        Op::LdsReadPublished {
            element, layout, ..
        }
        | Op::MemoryLoad {
            element, layout, ..
        } => vec![
            Type::Scalar(execution_scalar_from_identity_v1(types, *element, *layout)?),
            Type::BOOL,
        ],
        Op::WorkgroupBarrier {
            output_workgroup, ..
        } => vec![cap(
            *output_workgroup,
            ExecutionCapabilityRoleV1::Workgroup,
        )],
        Op::SubgroupBarrier {
            transition, width, ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            cap(
                *transition,
                ExecutionCapabilityRoleV1::Subgroup { width: *width },
            ),
        ],
        Op::WorkgroupFence { .. } | Op::SubgroupFence { .. } => Vec::new(),
        Op::Atomic {
            kind,
            location,
            result,
            element,
            value_type,
            scope,
            ..
        } => match kind {
            ExecutionAtomicKindV1::BindGlobalLocation => vec![
                cap(
                    *location,
                    ExecutionCapabilityRoleV1::ScopedAtomic {
                        element: *element,
                        space: ExecutionMemoryAddressSpaceV1::Global,
                        scope: *scope,
                    },
                ),
                Type::BOOL,
            ],
            ExecutionAtomicKindV1::BindGlobalView => {
                let layout = execution_layout_from_identity_v1(types, *element)?;
                let extent = execution_extent_ceiling_v1(
                    target_object_size_bound_bytes,
                    layout,
                    ScalarType::Index,
                )
                .ok_or_else(|| {
                    unsupported(0, None, None, "atomic view has no bounded target extent")
                })?;
                vec![cap(
                    *result,
                    ExecutionCapabilityRoleV1::MemoryView {
                        element: *element,
                        layout,
                        space: ExecutionMemoryAddressSpaceV1::Global,
                        access: ExecutionMemoryAccessV1::AtomicReadWrite,
                        extent: ExecutionMemoryExtentV1::Static(extent),
                        initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                        index_space: None,
                        atomic_scope: Some(*scope),
                    },
                )]
            }
            ExecutionAtomicKindV1::Load | ExecutionAtomicKindV1::FetchAdd => {
                vec![Type::Scalar(*value_type)]
            }
            ExecutionAtomicKindV1::Store => Vec::new(),
            ExecutionAtomicKindV1::CompareExchange => {
                vec![Type::Scalar(*value_type), Type::BOOL]
            }
        },
        Op::WorkgroupCollective {
            transition,
            element,
            value_type,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *transition,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Uninitialized,
            ),
            Type::Scalar(*value_type),
        ],
        Op::SubgroupCollective { value_type, .. } => vec![Type::Scalar(*value_type)],
        Op::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } => vec![cap(
            *matrix,
            ExecutionCapabilityRoleV1::Matrix {
                subgroup_brand: *subgroup_brand,
                width: *width,
            },
        )],
        Op::AsyncCopy {
            pending,
            element,
            layout,
            elements,
            ..
        } => vec![cap(
            *pending,
            ExecutionCapabilityRoleV1::PendingAsyncCopy {
                element: *element,
                layout: *layout,
                elements: *elements,
            },
        )],
        Op::AsyncWait {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
            lds(
                *output_lds,
                *element,
                *layout,
                *elements,
                ExecutionLdsStateV1::Published,
            ),
        ],
        Op::RawMemoryBind {
            view,
            extent,
            element,
            layout,
            space,
            access,
            index_space,
            atomic_scope,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: *space,
                access: *access,
                extent: ExecutionMemoryExtentV1::Dynamic(*extent),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: *index_space,
                atomic_scope: *atomic_scope,
            },
        )],
        Op::PrivateMemoryAllocate {
            view,
            element,
            layout,
            elements,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: ExecutionMemoryAddressSpaceV1::Private,
                access: ExecutionMemoryAccessV1::ExclusiveReadWrite,
                extent: ExecutionMemoryExtentV1::Static(*elements),
                initialization: ExecutionMemoryInitializationV1::FullyInitialized,
                index_space: None,
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryIndex { witness, .. } => vec![cap(
            *witness,
            ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
        )],
        Op::WorkgroupMemoryAllocate {
            view,
            element,
            layout,
            elements,
            index_space,
            ..
        } => vec![cap(
            *view,
            ExecutionCapabilityRoleV1::MemoryView {
                element: *element,
                layout: *layout,
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                access: ExecutionMemoryAccessV1::DisjointWrite,
                extent: ExecutionMemoryExtentV1::Static(*elements),
                initialization: ExecutionMemoryInitializationV1::Uninitialized,
                index_space: Some(*index_space),
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryPublish {
            output_view,
            transition,
            element,
            layout,
            ..
        } => {
            let extent = operand_types.get(1).and_then(|ty| match ty {
                Type::ExecutionCapability(capability) => match capability.role {
                    ExecutionCapabilityRoleV1::MemoryView { extent, .. } => Some(extent),
                    _ => None,
                },
                _ => None,
            });
            let extent = extent.ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "workgroup publication lost its exact input extent",
                )
            })?;
            vec![
                cap(*transition, ExecutionCapabilityRoleV1::Workgroup),
                cap(
                    *output_view,
                    ExecutionCapabilityRoleV1::MemoryView {
                        element: *element,
                        layout: *layout,
                        space: ExecutionMemoryAddressSpaceV1::Workgroup,
                        access: ExecutionMemoryAccessV1::ReadOnly,
                        extent,
                        initialization: ExecutionMemoryInitializationV1::Published,
                        index_space: None,
                        atomic_scope: None,
                    },
                ),
            ]
        }
        Op::MemoryStore { .. } => vec![Type::BOOL],
    })
}

fn execution_type_id_from_identity_v1(
    types: &[SemanticTypeDeclV1],
    identity: ExecutionTypeIdentityV1,
) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
    let (index, _) = types
        .iter()
        .enumerate()
        .find(|(_, declaration)| declaration.identity().as_bytes() == &identity.bytes())
        .ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "execution element identity is absent from semantic MIR",
            )
        })?;
    let index = u32::try_from(index)
        .map_err(|_| unsupported(0, None, None, "execution type index does not fit semantic MIR"))?;
    Ok(SemanticTypeIdV1::from_index(index))
}

fn execution_layout_from_identity_v1(
    types: &[SemanticTypeDeclV1],
    element: ExecutionTypeIdentityV1,
) -> Result<ExecutionElementLayoutV1, ProductionSemanticKirErrorV1> {
    execution_element_layout_v1(types, execution_type_id_from_identity_v1(types, element)?)
}

fn execution_scalar_from_identity_v1(
    types: &[SemanticTypeDeclV1],
    element: ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
) -> Result<ScalarType, ProductionSemanticKirErrorV1> {
    let element = execution_type_id_from_identity_v1(types, element)?;
    if execution_element_layout_v1(types, element)? != layout {
        return Err(unsupported(
            0,
            None,
            None,
            "execution element layout changed during result lowering",
        ));
    }
    execution_scalar_v1(types, element)
}

impl SemanticFunctionLoweringV1<'_> {
    fn lower_execution_capability_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operations: &mut Vec<Operation>,
        contract: SemanticExecutionCapabilityContractV1,
        callable_source_identity: SemanticFunctionIdentityV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let authenticated = self.kernel_context.ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "execution terminal lacks an authenticated kernel context",
            )
        })?;
        let source_arguments = contract.signature().arguments().collect::<Vec<_>>();
        if !self.is_kernel_entry
            || contract.source_identity() != callable_source_identity
            || !global_capability_provenance_matches_v1(authenticated, contract.provenance())
            || call.arguments().len() != source_arguments.len()
            || call
                .arguments()
                .iter()
                .map(semantic_operand_type)
                .ne(source_arguments.iter().copied())
            || call
                .destination()
                .map(|destination| destination.place().ty())
                != Some(contract.signature().output())
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "execution terminal substituted its signature, source, or root provenance",
            ));
        }

        let raw_memory = matches!(
            contract.operation(),
            SemanticExecutionCapabilityOperationV1::RawMemoryBind { .. }
        );
        let materialized_arguments = if raw_memory {
            source_arguments.len().checked_sub(1).ok_or_else(|| {
                unsupported(0, Some(block.index()), None, "raw-memory signature is truncated")
            })?
        } else {
            source_arguments.len()
        };
        let mut operands = Vec::with_capacity(materialized_arguments + 2);
        let mut operand_types = Vec::with_capacity(materialized_arguments + 2);
        for argument in call.arguments().iter().take(materialized_arguments) {
            let values = self
                .lower_operand(block, None, argument, operations)?
                .values()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let [(value, ty)] = values.as_slice() else {
                return Err(unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "execution terminal argument is not one exact logical SSA value",
                ));
            };
            operands.push(*value);
            operand_types.push(ty.clone());
        }

        let dynamic_extent = if let SemanticExecutionCapabilityOperationV1::RawMemoryBind {
            length,
            element,
            ..
        } = contract.operation()
        {
            let length_type = execution_scalar_v1(self.types, length)?;
            let layout = execution_element_layout_v1(self.types, element)?;
            let upper_bound = execution_extent_ceiling_v1(
                self.target_object_size_bound_bytes,
                layout,
                length_type,
            )
            .ok_or_else(|| {
                unsupported(
                    self.semantic_function.index(),
                    Some(block.index()),
                    None,
                    "raw-memory extent has no nonzero target resource ceiling",
                )
            })?;
            let length_value = *operands.get(2).ok_or_else(|| {
                unsupported(0, Some(block.index()), None, "raw-memory length is missing")
            })?;
            if operand_types.get(2) != Some(&Type::Scalar(length_type)) {
                return Err(unsupported(
                    0,
                    Some(block.index()),
                    None,
                    "raw-memory length transport type changed",
                ));
            }
            let bound = self.emit_id(
                operations,
                Type::Scalar(length_type),
                OperationKind::Constant(integer_constant(
                    &Type::Scalar(length_type),
                    u128::from(upper_bound),
                )?),
            )?;
            let upper_check = self.emit_id(
                operations,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: length_value,
                    rhs: bound,
                },
            )?;
            operands.push(upper_check);
            operand_types.push(Type::BOOL);
            let nonnegative_check_operand = if length_type.is_signed_integer() {
                let zero = self.emit_id(
                    operations,
                    Type::Scalar(length_type),
                    OperationKind::Constant(integer_constant(&Type::Scalar(length_type), 0)?),
                )?;
                let check = self.emit_id(
                    operations,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::GreaterThanOrEqual,
                        lhs: length_value,
                        rhs: zero,
                    },
                )?;
                operands.push(check);
                operand_types.push(Type::BOOL);
                Some(4)
            } else {
                None
            };
            Some(ExecutionDynamicExtentV1 {
                operand: 2,
                source_argument: 2,
                source_type: execution_type_identity_v1(self.types, length)?,
                value_type: length_type,
                upper_bound,
                bound_check_operand: 3,
                nonnegative_check_operand,
            })
        } else {
            None
        };

        let operation =
            lower_execution_operation_v1(self.types, contract.operation(), dynamic_extent)?;
        let provenance = ExecutionCapabilityProvenanceV1 {
            root: authenticated.context_type.root().clone(),
            kernel_binding: *contract.provenance().kernel_binding().as_bytes(),
            frontend_unit: *contract.provenance().frontend_unit().as_bytes(),
            kernel_marker: *contract.provenance().kernel_marker().as_bytes(),
            target_brand: *contract.provenance().target_brand().as_bytes(),
            launch_brand: *contract.provenance().launch_brand().as_bytes(),
            issuance: *contract.provenance().issuance().as_bytes(),
        };
        let signature = ExecutionCapabilitySignatureV1::new(
            &source_arguments
                .iter()
                .copied()
                .map(|ty| execution_type_identity_v1(self.types, ty))
                .collect::<Result<Vec<_>, _>>()?,
            execution_type_identity_v1(self.types, contract.signature().output())?,
        )
        .ok_or_else(|| unsupported(0, Some(block.index()), None, "execution signature is too wide"))?;
        let workgroup_brand = contract.workgroup_brand().map(|identity| *identity.as_bytes());
        let epoch_before = contract.epoch_before().map(|identity| *identity.as_bytes());
        let epoch_after = contract.epoch_after().map(|identity| *identity.as_bytes());
        let result_types = execution_result_types_v1(
            self.types,
            &operation,
            &provenance,
            workgroup_brand,
            epoch_before,
            epoch_after,
            &operand_types,
            self.target_object_size_bound_bytes,
        )?;
        let kir_contract = ExecutionCapabilityOpV1 {
            operands,
            operation: operation.clone(),
            signature,
            provenance,
            workgroup_brand,
            epoch_before,
            epoch_after,
            obligations: ExecutionSafetyObligationsV1::from_bits(contract.obligations().bits()),
            source: ExecutionCapabilitySourceV1 {
                function: *self.function.identity().as_bytes(),
                operation: *contract.source_identity().as_bytes(),
                block: block.index(),
            },
        };
        if !kir_contract.is_complete()
            || result_types.iter().any(|ty| {
                matches!(ty, Type::ExecutionCapability(capability) if !capability.is_complete())
            })
        {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "lowered execution-capability contract is incomplete",
            ));
        }
        let results = self.emit_results(
            operations,
            result_types,
            OperationKind::ExecutionCapability(kir_contract),
        )?;
        self.execution_result_binding_v1(
            block,
            operations,
            contract.operation(),
            contract.signature().output(),
            &results,
        )
    }

    fn execution_result_binding_v1(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        operation: SemanticExecutionCapabilityOperationV1,
        output: SemanticTypeIdV1,
        results: &[ValueDef],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        use SemanticExecutionCapabilityOperationV1 as Op;
        match operation {
            Op::LdsReadPublished {
                option, element, ..
            }
            | Op::MemoryLoad {
                option, element, ..
            } => {
                let [loaded, present] = results else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                self.execution_option_binding_v1(
                    block,
                    operations,
                    option,
                    element,
                    Some(present.id),
                    loaded,
                )
            }
            Op::Atomic {
                kind: SemanticExecutionAtomicKindV1::BindGlobalLocation,
                location,
                result,
                ..
            } => {
                let [location_value, present] = results else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                self.execution_option_binding_v1(
                    block,
                    operations,
                    result,
                    location,
                    Some(present.id),
                    location_value,
                )
            }
            Op::WorkgroupMemoryIndex { witness, .. } => {
                let [witness_value] = results else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                self.execution_option_binding_v1(
                    block,
                    operations,
                    output,
                    witness,
                    None,
                    witness_value,
                )
            }
            Op::Atomic {
                kind: SemanticExecutionAtomicKindV1::CompareExchange,
                element,
                result,
                ..
            } => {
                let [value, succeeded] = results else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                self.execution_result_enum_binding_v1(
                    block,
                    operations,
                    result,
                    element,
                    succeeded.id,
                    value,
                )
            }
            _ => self.execution_structural_binding_v1(block, output, results),
        }
    }

    fn execution_structural_binding_v1(
        &self,
        block: SemanticBlockIdV1,
        output: SemanticTypeIdV1,
        results: &[ValueDef],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let shape = self
            .types
            .get(output.index() as usize)
            .ok_or_else(|| unsupported(0, Some(block.index()), None, "execution result type is missing"))?
            .shape();
        if matches!(shape, SemanticTypeShapeV1::Unit) {
            return if results.is_empty() {
                Ok(SemanticValueBindingV1::Unit)
            } else {
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            };
        }
        if let SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) = shape {
            if fields.fields().len() == results.len() {
                return Ok(SemanticValueBindingV1::Aggregate(
                    results
                        .iter()
                        .map(|result| SemanticValueBindingV1::Value {
                            id: result.id,
                            ty: result.ty.clone(),
                        })
                        .collect(),
                ));
            }
        }
        let [result] = results else {
            return Err(unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                None,
                "execution result does not match its exact Rust destination shape",
            ));
        };
        Ok(SemanticValueBindingV1::Value {
            id: result.id,
            ty: result.ty.clone(),
        })
    }

    fn execution_option_binding_v1(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        option: SemanticTypeIdV1,
        payload: SemanticTypeIdV1,
        present: Option<ValueId>,
        value: &ValueDef,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let (discriminant, variants) = semantic_enum_shape(self.types, option)?;
        let some_variant = unique_enum_variant_with_field(variants, payload).ok_or_else(|| {
            unsupported(0, Some(block.index()), None, "execution result is not an exact Option payload")
        })?;
        let mut none_variants = variants.iter().enumerate().filter_map(|(index, variant)| {
            variant.fields().fields().is_empty().then_some(index as u32)
        });
        let none_variant = none_variants
            .next()
            .filter(|_| none_variants.next().is_none())
            .ok_or_else(|| unsupported(0, Some(block.index()), None, "execution Option lacks one None variant"))?;
        let discriminant_type = lower_scalar_type(self.types, discriminant)?;
        let discriminant = if let Some(present) = present {
            let none = self.emit_id(
                operations,
                discriminant_type.clone(),
                OperationKind::Constant(integer_constant(
                    &discriminant_type,
                    variants[none_variant as usize].discriminant(),
                )?),
            )?;
            let some = self.emit_id(
                operations,
                discriminant_type.clone(),
                OperationKind::Constant(integer_constant(
                    &discriminant_type,
                    variants[some_variant as usize].discriminant(),
                )?),
            )?;
            self.emit_id(
                operations,
                discriminant_type.clone(),
                OperationKind::Select {
                    condition: present,
                    true_value: some,
                    false_value: none,
                },
            )?
        } else {
            self.emit_id(
                operations,
                discriminant_type.clone(),
                OperationKind::Constant(integer_constant(
                    &discriminant_type,
                    variants[some_variant as usize].discriminant(),
                )?),
            )?
        };
        Ok(SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty: discriminant_type,
            semantic_type: option,
            variant: present.is_none().then_some(some_variant),
            payloads: BTreeMap::from([
                (none_variant, Vec::new()),
                (
                    some_variant,
                    vec![SemanticValueBindingV1::Value {
                        id: value.id,
                        ty: value.ty.clone(),
                    }],
                ),
            ]),
        })
    }

    fn execution_result_enum_binding_v1(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &mut Vec<Operation>,
        result: SemanticTypeIdV1,
        payload: SemanticTypeIdV1,
        succeeded: ValueId,
        value: &ValueDef,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let (discriminant, variants) = semantic_enum_shape(self.types, result)?;
        if variants.len() != 2
            || variants
                .iter()
                .any(|variant| variant.fields().fields() != [payload])
        {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "atomic compare-exchange result is not exact Result<T, T>",
            ));
        }
        let discriminant_type = lower_scalar_type(self.types, discriminant)?;
        let failure = self.emit_id(
            operations,
            discriminant_type.clone(),
            OperationKind::Constant(integer_constant(
                &discriminant_type,
                variants[1].discriminant(),
            )?),
        )?;
        let success = self.emit_id(
            operations,
            discriminant_type.clone(),
            OperationKind::Constant(integer_constant(
                &discriminant_type,
                variants[0].discriminant(),
            )?),
        )?;
        let discriminant = self.emit_id(
            operations,
            discriminant_type.clone(),
            OperationKind::Select {
                condition: succeeded,
                true_value: success,
                false_value: failure,
            },
        )?;
        let payload = SemanticValueBindingV1::Value {
            id: value.id,
            ty: value.ty.clone(),
        };
        Ok(SemanticValueBindingV1::Enum {
            discriminant,
            discriminant_ty: discriminant_type,
            semantic_type: result,
            variant: None,
            payloads: BTreeMap::from([(0, vec![payload.clone()]), (1, vec![payload])]),
        })
    }
}
