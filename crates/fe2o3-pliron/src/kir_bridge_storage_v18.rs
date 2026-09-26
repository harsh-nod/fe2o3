//! Exact-owner V18 profile and typed live-operand transport.
//! Table keys below are structural names, never graph or source authority.

use super::*;
use dialect_gpu::storage_operations_v18::{
    StorageAccessAttrV18, StorageDescriptorV18, StorageKindAttrV18, StorageOpV18,
    StorageOverlapAttrV18,
};
use dialect_gpu::storage_types_v18::{
    ExecutionRoleTypeV18, StorageObjectTypeV18, StorageOrdinalAttrV18, StorageTableKeyAttrV18,
};
use fe2o3_kernel_ir::{
    CanonicalStorageTableIdentityV18, ExecutionOperationV15, ExecutionRoleV15, MemoryAccess,
    StorageCopyOverlapV1, StorageLayoutIdV1, StorageOperationV1, StorageProjectionV1,
    VerifiedCanonicalKernelIrModuleV18,
};

#[derive(Clone, Copy)]
pub(super) struct ProfileV18<'input> {
    owner: &'input VerifiedCanonicalKernelIrModuleV18,
    key: CanonicalStorageTableIdentityV18,
}

impl<'input> ProfileV18<'input> {
    pub(super) fn new(
        owner: &'input VerifiedCanonicalKernelIrModuleV18,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18> {
        Ok(Self {
            owner,
            key: owner.storage_table_identity_with_budget_v18(budget)?,
        })
    }

    pub(super) fn owner(self) -> &'input VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }
    pub(super) fn key(self) -> CanonicalStorageTableIdentityV18 {
        self.key
    }
    fn attribute(self) -> StorageTableKeyAttrV18 {
        StorageTableKeyAttrV18::new(*self.key.digest(), self.key.encoded_length())
    }
    pub(super) fn validate_module(self, module: &Module) -> Result<(), KirBridgeErrorV1> {
        // Pointer equality joins this borrow; it does not mint an identity.
        if !std::ptr::eq(module, self.owner.module()) {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch);
        }
        Ok(())
    }

    pub(super) fn preflight_type(self, ty: &Type) -> Result<(), KirBridgeErrorV1> {
        match ty {
            Type::StorageObject(id) => {
                self.owner
                    .module()
                    .storage_layouts
                    .get(id.0 as usize)
                    .ok_or(KirBridgeErrorV1::UnsupportedType)?;
                Ok(())
            }
            Type::Execution(role) => role
                .validate()
                .map_err(|_| KirBridgeErrorV1::UnsupportedType),
            Type::Pointer(pointer) => self.preflight_type(&pointer.pointee),
            Type::Slice(slice) => self.preflight_type(&slice.element),
            Type::Unit | Type::Scalar(_) | Type::Vector(_) => {
                KirBridgeTypeProfileV12::V12.preflight_type(ty)
            }
        }
    }

    pub(super) fn preflight_operation(
        self,
        operation: &KirOperation,
        coordinate: KirBridgeCoordinateV1,
    ) -> Result<(), KirBridgeErrorV1> {
        match &operation.kind {
            OperationKind::Storage(_)
            | OperationKind::Gfx942OrderedRegion(_)
            | OperationKind::Gfx942OrderedProgram(_) => Ok(()),
            OperationKind::Execution(execution) => execution
                .validate_payload()
                .map_err(|_| KirBridgeErrorV1::UnsupportedOperation { coordinate }),
            _ => KirBridgeTypeProfileV12::V12.preflight_operation(operation, coordinate),
        }
    }

    pub(super) fn to_pliron(
        self,
        context: &Context,
        ty: &Type,
    ) -> Result<TypeHandle, KirBridgeErrorV1> {
        self.preflight_type(ty)?;
        Ok(match ty {
            Type::StorageObject(id) => {
                StorageObjectTypeV18::get(context, self.attribute(), StorageOrdinalAttrV18(id.0))
                    .into()
            }
            Type::Execution(role) => {
                let (role, lanes, elements) = match *role {
                    ExecutionRoleV15::Context => (1, 0, 0),
                    ExecutionRoleV15::Workgroup => (2, 0, 0),
                    ExecutionRoleV15::MaskedTileU32 { lanes, elements } => (3, lanes, elements),
                    ExecutionRoleV15::LaneFragmentU32 { lanes, elements } => (4, lanes, elements),
                };
                ExecutionRoleTypeV18::get(
                    context,
                    StorageOrdinalAttrV18(role),
                    StorageOrdinalAttrV18(u32::from(lanes)),
                    StorageOrdinalAttrV18(u32::from(elements)),
                )
                .into()
            }
            Type::Pointer(pointer) => PlironPointerType::get(
                context,
                self.to_pliron(context, &pointer.pointee)?,
                address_space_to_pliron(pointer.address_space)?,
                access_mode_to_pliron(pointer.access),
            )
            .into(),
            Type::Slice(slice) => PlironSliceType::get(
                context,
                self.to_pliron(context, &slice.element)?,
                address_space_to_pliron(slice.address_space)?,
                access_mode_to_pliron(slice.access),
            )
            .into(),
            Type::Unit | Type::Scalar(_) | Type::Vector(_) => {
                KirBridgeTypeProfileV12::V12.to_pliron(context, ty)?
            }
        })
    }

    pub(super) fn decode_type(
        self,
        context: &Context,
        ty: TypeHandle,
        depth: usize,
    ) -> Result<Type, KirBridgeErrorV1> {
        if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
            return Err(KirBridgeErrorV1::UnsupportedType);
        }
        let raw = ty.deref(context);
        if let Some(object) = raw.downcast_ref::<StorageObjectTypeV18>() {
            if object.table() != self.attribute() {
                return Err(KirBridgeErrorV1::GraphIdentityMismatch);
            }
            let result = Type::StorageObject(StorageLayoutIdV1(object.row()));
            self.preflight_type(&result)?;
            return Ok(result);
        }
        if let Some(role) = raw.downcast_ref::<ExecutionRoleTypeV18>() {
            let lanes =
                u16::try_from(role.lanes()).map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
            let elements =
                u16::try_from(role.elements()).map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
            let role = match role.role() {
                1 if lanes == 0 && elements == 0 => ExecutionRoleV15::Context,
                2 if lanes == 0 && elements == 0 => ExecutionRoleV15::Workgroup,
                3 => ExecutionRoleV15::MaskedTileU32 { lanes, elements },
                4 => ExecutionRoleV15::LaneFragmentU32 { lanes, elements },
                _ => return Err(KirBridgeErrorV1::UnsupportedType),
            };
            role.validate()
                .map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
            return Ok(Type::Execution(role));
        }
        if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
            return Ok(Type::pointer(
                self.decode_type(context, pointer.pointee(), depth + 1)?,
                address_space_from_pliron(pointer.address_space()),
                access_mode_from_pliron(pointer.access()),
            ));
        }
        if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
            return Ok(Type::slice(
                self.decode_type(context, slice.element(), depth + 1)?,
                address_space_from_pliron(slice.address_space()),
                access_mode_from_pliron(slice.access()),
            ));
        }
        KirBridgeTypeProfileV12::V12.decode_type_depth(context, ty, depth)
    }
}

fn access(value: MemoryAccess) -> Result<StorageAccessAttrV18, KirBridgeErrorV1> {
    Ok(StorageAccessAttrV18::new(
        address_space_to_pliron(value.address_space)?,
        value.alignment,
        value.volatile,
    ))
}

fn descriptor(operation: StorageOperationV1) -> Result<StorageDescriptorV18, KirBridgeErrorV1> {
    use StorageKindAttrV18 as K;
    let mut result = StorageDescriptorV18 {
        kind: K::ProjectField,
        selector: 0,
        overlap: StorageOverlapAttrV18::None,
        read: StorageAccessAttrV18::ABSENT,
        write: StorageAccessAttrV18::ABSENT,
    };
    match operation {
        StorageOperationV1::Project { step, .. } => match step {
            StorageProjectionV1::Field(index) => {
                result.kind = K::ProjectField;
                result.selector = index;
            }
            StorageProjectionV1::ArrayIndex(_) => result.kind = K::ProjectArray,
            StorageProjectionV1::Variant {
                index,
                access: memory,
            } => {
                result.kind = K::ProjectVariant;
                result.selector = index;
                result.read = access(memory)?;
            }
            StorageProjectionV1::VariantForWrite { index } => {
                result.kind = K::VariantForWrite;
                result.selector = index;
            }
        },
        StorageOperationV1::ReadValue { access: memory, .. } => {
            result.kind = K::ReadValue;
            result.read = access(memory)?;
        }
        StorageOperationV1::ReadDiscriminant { access: memory, .. } => {
            result.kind = K::ReadDiscriminant;
            result.read = access(memory)?;
        }
        StorageOperationV1::WriteValue { access: memory, .. } => {
            result.kind = K::WriteValue;
            result.write = access(memory)?;
        }
        StorageOperationV1::CopyObject {
            source_access,
            destination_access,
            overlap,
            ..
        } => {
            result.kind = K::CopyObject;
            result.read = access(source_access)?;
            result.write = access(destination_access)?;
            result.overlap = match overlap {
                StorageCopyOverlapV1::NonOverlapping => StorageOverlapAttrV18::NonOverlapping,
                StorageCopyOverlapV1::MayOverlap => StorageOverlapAttrV18::MayOverlap,
            };
        }
        StorageOperationV1::SetDiscriminant {
            variant,
            access: memory,
            ..
        } => {
            result.kind = K::SetDiscriminant;
            result.selector = variant;
            result.write = access(memory)?;
        }
    }
    Ok(result)
}

pub(super) fn build(
    context: &mut Context,
    function: usize,
    operation: &KirOperation,
    values: &BTreeMap<ValueId, Value>,
    profile: ProfileV18<'_>,
) -> Result<Ptr<Operation>, KirBridgeErrorV1> {
    let OperationKind::Storage(storage) = &operation.kind else {
        return Err(KirBridgeErrorV1::MalformedGraph);
    };
    let types = operation
        .results
        .iter()
        .map(|result| profile.to_pliron(context, &result.ty))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StorageOpV18::new(
        context,
        descriptor(*storage)?,
        values_for(values, function, &operation.operands())?,
        types,
    )
    .get_operation())
}

pub(super) fn is_storage(context: &Context, live: Ptr<Operation>) -> bool {
    Operation::is_op::<StorageOpV18>(live, context)
}

pub(super) fn extract(
    context: &Context,
    live: Ptr<Operation>,
    reverse: &HashMap<Value, ValueId>,
    origins: &KirBridgeOriginsV1,
) -> Result<OperationKind, KirBridgeErrorV1> {
    let Some(OperationKind::Storage(template)) = origins.preserved_operations.get(&live) else {
        return Err(KirBridgeErrorV1::GraphIdentityMismatch);
    };
    let operation =
        Operation::get_op::<StorageOpV18>(live, context).ok_or(KirBridgeErrorV1::MalformedGraph)?;
    if operation.descriptor(context) != Some(descriptor(*template)?) {
        return Err(KirBridgeErrorV1::GraphIdentityMismatch);
    }
    let raw = live.deref(context);
    if raw.get_num_operands() != template.operand_count() {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    let operand = |index| id_for(reverse, raw.get_operand(index));
    let result = match *template {
        StorageOperationV1::Project { step, .. } => StorageOperationV1::Project {
            base: operand(0)?,
            step: match step {
                StorageProjectionV1::ArrayIndex(_) => StorageProjectionV1::ArrayIndex(operand(1)?),
                other => other,
            },
        },
        StorageOperationV1::ReadValue { access, .. } => StorageOperationV1::ReadValue {
            address: operand(0)?,
            access,
        },
        StorageOperationV1::ReadDiscriminant { access, .. } => StorageOperationV1::ReadDiscriminant {
            address: operand(0)?,
            access,
        },
        StorageOperationV1::WriteValue { access, .. } => StorageOperationV1::WriteValue {
            address: operand(0)?,
            value: operand(1)?,
            access,
        },
        StorageOperationV1::CopyObject {
            source_access,
            destination_access,
            overlap,
            ..
        } => StorageOperationV1::CopyObject {
            source: operand(0)?,
            destination: operand(1)?,
            source_access,
            destination_access,
            overlap,
        },
        StorageOperationV1::SetDiscriminant {
            variant, access, ..
        } => StorageOperationV1::SetDiscriminant {
            address: operand(0)?,
            variant,
            access,
        },
    };
    Ok(OperationKind::Storage(result))
}

pub(super) fn preserved_kind(
    kind: &OperationKind,
) -> Result<PreservedOperationKindAttr, KirBridgeErrorV1> {
    Ok(match kind {
        OperationKind::Execution(_) => PreservedOperationKindAttr::ExecutionV18,
        OperationKind::Gfx942OrderedRegion(_) => PreservedOperationKindAttr::OrderedRegionV18,
        OperationKind::Gfx942OrderedProgram(_) => PreservedOperationKindAttr::OrderedProgramV18,
        _ => return preserved_operation_kind(kind),
    })
}

pub(super) fn remap(
    template: &OperationKind,
    live: Vec<ValueId>,
) -> Result<OperationKind, KirBridgeErrorV1> {
    if template.operands().len() != live.len() {
        return Err(KirBridgeErrorV1::MalformedGraph);
    }
    Ok(match template {
        OperationKind::Execution(execution) => {
            use ExecutionOperationV15 as E;
            let mapped = match execution {
                E::ContextIssue => E::ContextIssue,
                E::WorkgroupDerive { .. } => E::WorkgroupDerive { context: live[0] },
                E::ScopeEnd { .. } => E::ScopeEnd {
                    workgroup: live[0],
                    discarded: live[1..].to_vec(),
                },
                E::MaskedTileLoadU32 {
                    lanes, elements, ..
                } => E::MaskedTileLoadU32 {
                    workgroup: live[0],
                    input: live[1],
                    base: live[2],
                    lanes: *lanes,
                    elements: *elements,
                },
                E::TileIntoFragmentU32 {
                    lanes, elements, ..
                } => E::TileIntoFragmentU32 {
                    tile: live[0],
                    lanes: *lanes,
                    elements: *elements,
                },
                E::FragmentIntoPartsU32 {
                    lanes, elements, ..
                } => E::FragmentIntoPartsU32 {
                    fragment: live[0],
                    lanes: *lanes,
                    elements: *elements,
                },
            };
            // Discard order is a semantic wire invariant, not silently sorted.
            mapped
                .validate_payload()
                .map_err(|_| KirBridgeErrorV1::MalformedGraph)?;
            OperationKind::Execution(mapped)
        }
        OperationKind::Gfx942OrderedRegion(region) => OperationKind::Gfx942OrderedRegion(
            fe2o3_kernel_ir::Gfx942OrderedRegionV1::new(
                region.source(),
                region.registers(),
                [live[0], live[1], live[2]],
            )
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        ),
        OperationKind::Gfx942OrderedProgram(program) => OperationKind::Gfx942OrderedProgram(
            fe2o3_kernel_ir::Gfx942OrderedProgramV1::new(
                program.source(),
                program.registers(),
                [live[0], live[1], live[2]],
                *program.program(),
            )
            .map_err(|_| KirBridgeErrorV1::MalformedGraph)?,
        ),
        _ => return remap_preserved_operation(template, live),
    })
}
