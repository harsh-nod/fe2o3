use super::*;

/// Inert nominal role carried alongside the exact ordinary Rust aggregate layout.
/// A role is not evidence of its producer, scope, epoch, or compiler authentication.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticExecutionRoleV29 {
    KernelContext,
    Workgroup,
    MaskedTileU32 { lanes: u16, elements: u16 },
    LaneFragmentU32 { lanes: u16, elements: u16 },
}

impl SemanticExecutionRoleV29 {
    pub const fn geometry(self) -> Option<(u16, u16)> {
        match self {
            Self::KernelContext | Self::Workgroup => None,
            Self::MaskedTileU32 { lanes, elements } | Self::LaneFragmentU32 { lanes, elements } => {
                Some((lanes, elements))
            }
        }
    }

    pub(super) fn validate_geometry(self) -> Result<(), SemanticMirErrorV1> {
        if self.geometry().is_some_and(|(lanes, elements)| {
            !(1..=256).contains(&lanes) || !(1..=125).contains(&elements)
        }) {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        Ok(())
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        self.validate_geometry()?;
        writer.u8(match self {
            Self::KernelContext => 14,
            Self::Workgroup => 15,
            Self::MaskedTileU32 { .. } => 16,
            Self::LaneFragmentU32 { .. } => 17,
        })?;
        if let Some((lanes, elements)) = self.geometry() {
            writer.u16(lanes)?;
            writer.u16(elements)?;
        }
        Ok(())
    }
}

fn field(
    types: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
) -> Result<&SemanticTypeDeclV1, SemanticMirErrorV1> {
    types
        .get(id.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)
}

fn empty_marker(ty: &SemanticTypeDeclV1) -> bool {
    ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary
        && ty.layout.size_bytes == Some(0)
        && ty.layout.alignment_bytes == 1
        && !ty.layout.uninhabited
        && matches!(&ty.shape, SemanticTypeShapeV1::Aggregate(fields) if fields.fields().is_empty())
}

const U32: SemanticScalarTypeV1 = SemanticScalarTypeV1::Integer {
    signed: false,
    bits: 32,
};
const U64: SemanticScalarTypeV1 = SemanticScalarTypeV1::Integer {
    signed: false,
    bits: 64,
};

/// Owned containment, not reachability: borrowing a role does not own it.
/// Reverse edges give O(types + fields + variants) propagation for any type order.
pub(super) fn owned_role_types(
    context: &mut ValidationContextV1<'_>,
) -> Result<Vec<bool>, SemanticMirErrorV1> {
    let types = &context.request.types;
    let allocation_error = || SemanticMirErrorV1::AllocationFailed {
        resource: SemanticMirResourceV1::Types,
    };
    charge_validation_work(context, types.len())?;
    let mut parents = Vec::new();
    parents
        .try_reserve_exact(types.len())
        .map_err(|_| allocation_error())?;
    parents.resize_with(types.len(), Vec::new);
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(types.len())
        .map_err(|_| allocation_error())?;
    owned.resize(types.len(), false);
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(types.len())
        .map_err(|_| allocation_error())?;
    for (parent, ty) in types.iter().enumerate() {
        context.one()?;
        if matches!(ty.rust_type_kind, SemanticRustTypeKindV1::Execution(_)) {
            owned[parent] = true;
            pending.push(parent);
        }
        if let SemanticTypeShapeV1::Enum { variants, .. } = &ty.shape {
            charge_validation_work(context, variants.len())?;
        }
        let mut edge = |id: SemanticTypeIdV1| -> Result<(), SemanticMirErrorV1> {
            context.one()?;
            let row = parents
                .get_mut(id.0 as usize)
                .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
            row.try_reserve(1).map_err(|_| allocation_error())?;
            row.push(parent);
            Ok(())
        };
        match &ty.shape {
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                edge(*element)?
            }
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                for id in fields.fields() {
                    edge(*id)?;
                }
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for variant in variants {
                    for id in variant.fields.fields() {
                        edge(*id)?;
                    }
                }
            }
            SemanticTypeShapeV1::Pointer(_)
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_)
            | SemanticTypeShapeV1::Opaque => {}
        }
    }
    while let Some(child) = pending.pop() {
        context.one()?;
        for parent in &parents[child] {
            context.one()?;
            if !owned[*parent] {
                owned[*parent] = true;
                pending.push(*parent);
            }
        }
    }
    Ok(owned)
}

fn scalar(ty: &SemanticTypeDeclV1, scalar: SemanticScalarTypeV1) -> bool {
    ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary
        && !ty.layout.uninhabited
        && ty.shape == SemanticTypeShapeV1::Scalar(scalar)
}

fn array(
    types: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
    scalar_type: SemanticScalarTypeV1,
    elements: u16,
) -> Result<bool, SemanticMirErrorV1> {
    let ty = field(types, id)?;
    let SemanticTypeShapeV1::Array { element, length } = ty.shape else {
        return Ok(false);
    };
    Ok(ty.rust_type_kind == SemanticRustTypeKindV1::Ordinary
        && !ty.layout.uninhabited
        && length == u64::from(elements)
        && scalar(field(types, element)?, scalar_type))
}

/// Additional fixed-size checks after ordinary aggregate/layout/ABI validation.
pub(super) fn validate_execution_type(
    context: &mut ValidationContextV1<'_>,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let SemanticRustTypeKindV1::Execution(role) = ty.rust_type_kind else {
        return Ok(());
    };
    // The largest admitted shape examines four fields and three epoch markers.
    charge_validation_work(context, 16)?;
    role.validate_geometry()?;
    let SemanticTypeShapeV1::Aggregate(fields) = &ty.shape else {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    };
    let fields = fields.fields();
    let types = &context.request.types;
    let valid = match role {
        SemanticExecutionRoleV29::KernelContext => {
            fields.len() == 5
                && ty.layout.size_bytes == Some(0)
                && ty.layout.alignment_bytes == 1
                && fields
                    .iter()
                    .all(|id| field(types, *id).is_ok_and(empty_marker))
        }
        SemanticExecutionRoleV29::Workgroup if fields.len() == 4 => {
            let epoch = field(types, fields[2])?;
            let epoch_valid = epoch.rust_type_kind == SemanticRustTypeKindV1::Ordinary
                && !epoch.layout.uninhabited
                && epoch.layout.size_bytes == Some(0)
                && epoch.layout.alignment_bytes == 1
                && matches!(&epoch.shape, SemanticTypeShapeV1::Aggregate(markers)
                    if markers.fields().len() == 3 && markers.fields().iter().all(|id| field(types, *id).is_ok_and(empty_marker)));
            ty.layout.size_bytes == Some(16)
                && scalar(field(types, fields[0])?, U64)
                && scalar(field(types, fields[1])?, U64)
                && epoch_valid
                && empty_marker(field(types, fields[3])?)
        }
        SemanticExecutionRoleV29::MaskedTileU32 { elements, .. }
        | SemanticExecutionRoleV29::LaneFragmentU32 { elements, .. }
            if fields.len() == 4 =>
        {
            array(types, fields[0], U32, elements)?
                && array(types, fields[1], SemanticScalarTypeV1::Bool, elements)?
                && empty_marker(field(types, fields[2])?)
                && empty_marker(field(types, fields[3])?)
        }
        _ => false,
    };
    if valid && !ty.layout.uninhabited {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    }
}

/// Callable V29 descriptors. Geometry comes from the exact tile/fragment type.
/// Scope exit is a checked KIR materializer event, never a Rust callable ABI.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticExecutionOperationV29 {
    ContextIssue {
        context: SemanticTypeIdV1,
    },
    WorkgroupDerive {
        context: SemanticTypeIdV1,
        workgroup: SemanticTypeIdV1,
    },
    MaskedTileLoadU32 {
        workgroup: SemanticTypeIdV1,
        tile: SemanticTypeIdV1,
    },
    MaskedTileIntoFragmentU32 {
        tile: SemanticTypeIdV1,
        fragment: SemanticTypeIdV1,
    },
    LaneFragmentIntoPartsU32 {
        fragment: SemanticTypeIdV1,
        parts: SemanticTypeIdV1,
    },
}

pub(super) fn role(
    request: &InertSemanticMirRequestV1,
    id: SemanticTypeIdV1,
) -> Option<SemanticExecutionRoleV29> {
    match request.types.get(id.0 as usize)?.rust_type_kind {
        SemanticRustTypeKindV1::Execution(role) => Some(role),
        _ => None,
    }
}

impl SemanticExecutionOperationV29 {
    pub(super) fn type_ids(self) -> impl Iterator<Item = SemanticTypeIdV1> {
        let (first, second) = match self {
            Self::ContextIssue { context } => (context, None),
            Self::WorkgroupDerive { context, workgroup } => (context, Some(workgroup)),
            Self::MaskedTileLoadU32 { workgroup, tile } => (workgroup, Some(tile)),
            Self::MaskedTileIntoFragmentU32 { tile, fragment } => (tile, Some(fragment)),
            Self::LaneFragmentIntoPartsU32 { fragment, parts } => (fragment, Some(parts)),
        };
        [Some(first), second].into_iter().flatten()
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        writer.u8(match self {
            Self::ContextIssue { .. } => 81,
            Self::WorkgroupDerive { .. } => 82,
            Self::MaskedTileLoadU32 { .. } => 84,
            Self::MaskedTileIntoFragmentU32 { .. } => 85,
            Self::LaneFragmentIntoPartsU32 { .. } => 86,
        })?;
        for id in self.type_ids() {
            writer.u32(id.0)?;
        }
        Ok(())
    }

    pub(super) fn signature_matches(
        self,
        request: &InertSemanticMirRequestV1,
        inputs: &[SemanticTypeIdV1],
        output: SemanticTypeIdV1,
    ) -> bool {
        use SemanticExecutionRoleV29 as Role;
        match self {
            Self::ContextIssue { context } => {
                inputs.is_empty()
                    && output == context
                    && role(request, context) == Some(Role::KernelContext)
            }
            Self::WorkgroupDerive { context, workgroup } => {
                matches!(inputs, [input] if mutable_reference_to(request, *input, context))
                    && output == workgroup
                    && role(request, context) == Some(Role::KernelContext)
                    && role(request, workgroup) == Some(Role::Workgroup)
            }
            Self::MaskedTileLoadU32 { workgroup, tile } => {
                let [scope, input, base] = inputs else {
                    return false;
                };
                let Some(Role::MaskedTileU32 { .. }) = role(request, tile) else {
                    return false;
                };
                let Some(ty) = request.types.get(tile.0 as usize) else {
                    return false;
                };
                let SemanticTypeShapeV1::Aggregate(fields) = &ty.shape else {
                    return false;
                };
                let Some(values) = fields
                    .fields()
                    .first()
                    .and_then(|id| request.types.get(id.0 as usize))
                else {
                    return false;
                };
                let SemanticTypeShapeV1::Array { element, .. } = values.shape else {
                    return false;
                };
                output == tile
                    && role(request, workgroup) == Some(Role::Workgroup)
                    && shared_reference_to(request, *scope, workgroup)
                    && shared_slice_reference_with_element(request, *input, element)
                    && is_unsigned_integer_with_bits(request, *base, 64)
            }
            Self::MaskedTileIntoFragmentU32 { tile, fragment } => {
                let Some(Role::MaskedTileU32 { lanes, elements }) = role(request, tile) else {
                    return false;
                };
                inputs == [tile]
                    && output == fragment
                    && role(request, fragment) == Some(Role::LaneFragmentU32 { lanes, elements })
            }
            Self::LaneFragmentIntoPartsU32 { fragment, parts } => {
                let Some(Role::LaneFragmentU32 { elements, .. }) = role(request, fragment) else {
                    return false;
                };
                let Some(parts_decl) = request.types.get(parts.0 as usize) else {
                    return false;
                };
                let SemanticTypeShapeV1::Tuple(fields) = &parts_decl.shape else {
                    return false;
                };
                let [values, active] = fields.fields() else {
                    return false;
                };
                inputs == [fragment]
                    && output == parts
                    && parts_decl.rust_type_kind == SemanticRustTypeKindV1::Ordinary
                    && !parts_decl.layout.uninhabited
                    && array(&request.types, *values, U32, elements).unwrap_or(false)
                    && array(
                        &request.types,
                        *active,
                        SemanticScalarTypeV1::Bool,
                        elements,
                    )
                    .unwrap_or(false)
            }
        }
    }
}
