use fe2o3_kernel_ir::ExecutionRoleV15;
use fe2o3_mir_model::semantic_mir_v1::{SemanticExecutionRoleV29, SemanticTypeIdentityV1};
use production_call_instance_ids_v1::{ProductionCallInstanceIdV1, ProductionCallOccurrenceV1};

/// Nominal coordinates only. These records do not authenticate a producer,
/// establish reaching definitions, or prove a borrow's lifetime or exclusivity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticExecutionIdentityV29 {
    semantic_type: SemanticTypeIdV1,
    type_identity: SemanticTypeIdentityV1,
    producer: ProductionCallOccurrenceV1,
    value: ValueId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticExecutionBindingV29 {
    identity: SemanticExecutionIdentityV29,
    role: SemanticExecutionRoleV29,
    context: SemanticExecutionIdentityV29,
    workgroup: Option<SemanticExecutionIdentityV29>,
}

fn semantic_execution_kir_role_v29(
    role: SemanticExecutionRoleV29,
) -> Result<ExecutionRoleV15, &'static str> {
    let role = match role {
        SemanticExecutionRoleV29::KernelContext => ExecutionRoleV15::Context,
        SemanticExecutionRoleV29::Workgroup => ExecutionRoleV15::Workgroup,
        SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements } => {
            ExecutionRoleV15::MaskedTileU32 { lanes, elements }
        }
        SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements } => {
            ExecutionRoleV15::LaneFragmentU32 { lanes, elements }
        }
    };
    role.validate()
        .map_err(|_| "execution binding has invalid geometry")?;
    Ok(role)
}

fn semantic_execution_type_v29(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
) -> Result<(SemanticTypeIdentityV1, SemanticExecutionRoleV29), &'static str> {
    let declaration = types
        .get(semantic_type.index() as usize)
        .ok_or("execution binding source type is unavailable")?;
    let fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(role) =
        declaration.rust_type_kind()
    else {
        return Err("execution binding requires a nominal execution type");
    };
    semantic_execution_kir_role_v29(role)?;
    Ok((declaration.identity(), role))
}

impl SemanticExecutionIdentityV29 {
    fn check_type(
        self,
        types: &[SemanticTypeDeclV1],
        role: SemanticExecutionRoleV29,
    ) -> Result<(), &'static str> {
        if semantic_execution_type_v29(types, self.semantic_type)? != (self.type_identity, role) {
            return Err("execution binding nominal type identity changed");
        }
        Ok(())
    }
}

impl SemanticExecutionBindingV29 {
    #[cfg(test)]
    fn new_identity(
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        producer: ProductionCallOccurrenceV1,
        value: ValueId,
    ) -> Result<(SemanticExecutionIdentityV29, SemanticExecutionRoleV29), &'static str> {
        let (type_identity, role) = semantic_execution_type_v29(types, semantic_type)?;
        Ok((
            SemanticExecutionIdentityV29 {
                semantic_type,
                type_identity,
                producer,
                value,
            },
            role,
        ))
    }

    #[cfg(test)]
    fn context(
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        producer: ProductionCallOccurrenceV1,
        value: ValueId,
    ) -> Result<Self, &'static str> {
        let (identity, role) = Self::new_identity(types, semantic_type, producer, value)?;
        if role != SemanticExecutionRoleV29::KernelContext {
            return Err("context issuance requires the exact context role");
        }
        Ok(Self {
            identity,
            role,
            context: identity,
            workgroup: None,
        })
    }

    #[cfg(test)]
    fn workgroup(
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        producer: ProductionCallOccurrenceV1,
        value: ValueId,
        context: &Self,
    ) -> Result<Self, &'static str> {
        context.check_type(types, context.semantic_type())?;
        let (identity, role) = Self::new_identity(types, semantic_type, producer, value)?;
        if role != SemanticExecutionRoleV29::Workgroup
            || context.role != SemanticExecutionRoleV29::KernelContext
        {
            return Err("workgroup derivation requires a context and a workgroup role");
        }
        Ok(Self {
            identity,
            role,
            context: context.identity,
            workgroup: Some(identity),
        })
    }

    #[cfg(test)]
    fn tile(
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        producer: ProductionCallOccurrenceV1,
        value: ValueId,
        workgroup: &Self,
    ) -> Result<Self, &'static str> {
        workgroup.check_type(types, workgroup.semantic_type())?;
        let (identity, role) = Self::new_identity(types, semantic_type, producer, value)?;
        if !matches!(role, SemanticExecutionRoleV29::MaskedTileU32 { .. })
            || workgroup.role != SemanticExecutionRoleV29::Workgroup
        {
            return Err("masked tile binding requires an exact workgroup producer");
        }
        Ok(Self {
            identity,
            role,
            context: workgroup.context,
            workgroup: Some(workgroup.identity),
        })
    }

    #[cfg(test)]
    fn fragment(
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        producer: ProductionCallOccurrenceV1,
        value: ValueId,
        tile: &Self,
    ) -> Result<Self, &'static str> {
        tile.check_type(types, tile.semantic_type())?;
        let (identity, role) = Self::new_identity(types, semantic_type, producer, value)?;
        let SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements } = tile.role else {
            return Err("fragment binding requires an exact masked tile producer");
        };
        if role != (SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements }) {
            return Err("fragment binding changes the tile geometry");
        }
        Ok(Self {
            identity,
            role,
            context: tile.context,
            workgroup: tile.workgroup,
        })
    }

    fn check_type(
        &self,
        types: &[SemanticTypeDeclV1],
        expected: SemanticTypeIdV1,
    ) -> Result<(), &'static str> {
        if self.semantic_type() != expected {
            return Err("execution binding source type differs from the exact operand type");
        }
        self.identity.check_type(types, self.role)?;
        self.context
            .check_type(types, SemanticExecutionRoleV29::KernelContext)?;
        if let Some(workgroup) = self.workgroup {
            workgroup.check_type(types, SemanticExecutionRoleV29::Workgroup)?;
        }
        Ok(())
    }

    const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.identity.semantic_type
    }
    #[cfg(test)]
    const fn role(&self) -> SemanticExecutionRoleV29 {
        self.role
    }
    #[cfg(test)]
    const fn value(&self) -> ValueId {
        self.identity.value
    }
    const fn producer(&self) -> ProductionCallOccurrenceV1 {
        self.identity.producer
    }
    #[cfg(test)]
    const fn context_identity(&self) -> SemanticExecutionIdentityV29 {
        self.context
    }
    #[cfg(test)]
    const fn workgroup_identity(&self) -> Option<SemanticExecutionIdentityV29> {
        self.workgroup
    }

    #[cfg(test)]
    fn kir_type(&self) -> Result<Type, &'static str> {
        semantic_execution_kir_role_v29(self.role).map(Type::Execution)
    }
}

struct SemanticExecutionBorrowSourceV29<'a> {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    statement: usize,
    destination: &'a SemanticPlaceV1,
    kind: SemanticBorrowKindV1,
    source: &'a SemanticPlaceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SemanticExecutionBorrowOccurrenceV29 {
    instance: ProductionCallInstanceIdV1,
    block: SemanticBlockIdV1,
    statement: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SemanticExecutionBorrowBindingV29 {
    reference_type: SemanticTypeIdV1,
    reference_identity: SemanticTypeIdentityV1,
    occurrence: SemanticExecutionBorrowOccurrenceV29,
    destination_local: SemanticLocalIdV1,
    source_local: SemanticLocalIdV1,
    kind: SemanticBorrowKindV1,
    borrowed: SemanticExecutionBindingV29,
}

impl SemanticExecutionBorrowBindingV29 {
    fn from_source(
        types: &[SemanticTypeDeclV1],
        source: SemanticExecutionBorrowSourceV29<'_>,
        borrowed: &SemanticExecutionBindingV29,
    ) -> Result<Self, &'static str> {
        if !source.destination.projections().is_empty() || !source.source.projections().is_empty() {
            return Err("projected execution borrow requires occurrence-qualified projection");
        }
        borrowed.check_type(types, source.source.ty())?;
        let reference_type = source.destination.ty();
        let declaration = types
            .get(reference_type.index() as usize)
            .ok_or("execution borrow reference type is unavailable")?;
        let SemanticTypeShapeV1::Pointer(reference) = declaration.shape() else {
            return Err("execution borrow requires an exact source reference");
        };
        let mutability = match source.kind {
            SemanticBorrowKindV1::Mutable => SemanticMutabilityV1::Mutable,
            SemanticBorrowKindV1::Shared => SemanticMutabilityV1::Immutable,
            SemanticBorrowKindV1::Fake => return Err("fake execution borrow is unsupported"),
        };
        if reference.kind() != SemanticPointerKindV1::Reference
            || reference.mutability() != mutability
            || reference.address_space() != 0
            || reference.pointer_width_bits() != 64
            || reference.metadata() != SemanticPointerMetadataV1::None
            || reference.pointee() != borrowed.semantic_type()
        {
            return Err("execution borrow changes the exact reference or referent type");
        }
        Ok(Self {
            reference_type,
            reference_identity: declaration.identity(),
            occurrence: SemanticExecutionBorrowOccurrenceV29 {
                instance: source.instance,
                block: source.block,
                statement: source.statement,
            },
            destination_local: source.destination.local(),
            source_local: source.source.local(),
            kind: source.kind,
            borrowed: borrowed.clone(),
        })
    }

    fn check_type(
        &self,
        types: &[SemanticTypeDeclV1],
        expected: SemanticTypeIdV1,
    ) -> Result<(), &'static str> {
        let declaration = types
            .get(expected.index() as usize)
            .ok_or("execution borrow reference type is unavailable")?;
        if expected != self.reference_type || declaration.identity() != self.reference_identity {
            return Err("execution borrow nominal reference type changed");
        }
        let SemanticTypeShapeV1::Pointer(reference) = declaration.shape() else {
            return Err("execution borrow requires an exact source reference");
        };
        let expected_mutability = match self.kind {
            SemanticBorrowKindV1::Shared => SemanticMutabilityV1::Immutable,
            SemanticBorrowKindV1::Mutable => SemanticMutabilityV1::Mutable,
            SemanticBorrowKindV1::Fake => return Err("fake execution borrow is unsupported"),
        };
        if reference.kind() != SemanticPointerKindV1::Reference
            || reference.mutability() != expected_mutability
            || reference.address_space() != 0
            || reference.pointer_width_bits() != 64
            || reference.metadata() != SemanticPointerMetadataV1::None
            || reference.pointee() != self.borrowed.semantic_type()
        {
            return Err("execution borrow changes the exact reference or referent type");
        }
        self.borrowed.check_type(types, reference.pointee())
    }

    fn dereference(
        &self,
        types: &[SemanticTypeDeclV1],
        reference_type: SemanticTypeIdV1,
        projection: &SemanticProjectionV1,
    ) -> Result<&SemanticExecutionBindingV29, &'static str> {
        self.check_type(types, reference_type)?;
        if projection.kind() != SemanticProjectionKindV1::Dereference
            || projection.result_type() != self.borrowed.semantic_type()
        {
            return Err("execution borrow projection changes the exact referent type");
        }
        Ok(&self.borrowed)
    }

    const fn reference_type(&self) -> SemanticTypeIdV1 {
        self.reference_type
    }
    const fn kind(&self) -> SemanticBorrowKindV1 {
        self.kind
    }
    #[cfg(test)]
    const fn source_local(&self) -> SemanticLocalIdV1 {
        self.source_local
    }
    #[cfg(test)]
    const fn destination_local(&self) -> SemanticLocalIdV1 {
        self.destination_local
    }
    #[cfg(test)]
    const fn occurrence(&self) -> SemanticExecutionBorrowOccurrenceV29 {
        self.occurrence
    }
    const fn borrowed(&self) -> &SemanticExecutionBindingV29 {
        &self.borrowed
    }
}

#[cfg(test)]
#[path = "production_execution_bindings_v1_tests.rs"]
mod execution_binding_tests;
