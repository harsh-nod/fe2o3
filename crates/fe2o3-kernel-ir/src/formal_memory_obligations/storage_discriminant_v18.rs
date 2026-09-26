//! Exact physical subject for a tag read. This is an obligation, not a proof of
//! initialization, active variant, pointer provenance or source correspondence.

use crate::{
    AccessMode, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Coordinate,
    MemoryAccess, Operation, OperationKind, PointerType, ScalarType, StorageFieldV1,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1,
    StorageVariantEncodingV1, StorageVariantV1, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV18,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalStorageDiscriminantReadErrorV18 {
    Resource(Resource),
    Coordinate,
    AddressDefinition,
    Operation,
    Layout,
}

impl From<Resource> for CanonicalStorageDiscriminantReadErrorV18 {
    fn from(value: Resource) -> Self { Self::Resource(value) }
}

impl std::fmt::Display for CanonicalStorageDiscriminantReadErrorV18 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "stored discriminant obligation: {self:?}")
    }
}

impl std::error::Error for CanonicalStorageDiscriminantReadErrorV18 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

/// Every obligation remains pending until the production physical/source
/// completion rechecks this exact immutable owner and occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageDiscriminantReadRequirementV18 {
    TagBounds,
    TagInitialization,
    LifetimeAndProvenance,
    CurrentEnclosingGuards,
    ValidLogicalDecoding,
    ReadOrderingAndRaceFreedom,
    OriginalSourceCorrespondence,
}

pub struct CanonicalStorageDiscriminantReadObligationV18<'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    coordinate: Coordinate,
    definition: Definition,
    operation: &'owner Operation,
    address: ValueId,
    pointer: &'owner PointerType,
    layout: StorageLayoutIdV1,
    tag: StorageFieldV1,
    tag_row: &'owner StorageLayoutV1,
    encoding: &'owner StorageVariantEncodingV1,
    variants: &'owner [StorageVariantV1],
    access: MemoryAccess,
    result: &'owner ValueDef,
}

impl<'owner> CanonicalStorageDiscriminantReadObligationV18<'owner> {
    pub fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 { self.owner }
    pub fn coordinate(&self) -> Coordinate { self.coordinate }
    pub fn address_definition(&self) -> Definition { self.definition }
    pub fn operation(&self) -> &'owner Operation { self.operation }
    pub fn address(&self) -> ValueId { self.address }
    pub fn pointer(&self) -> &'owner PointerType { self.pointer }
    pub fn object_layout(&self) -> StorageLayoutIdV1 { self.layout }
    pub fn tag(&self) -> StorageFieldV1 { self.tag }
    pub fn tag_layout(&self) -> &'owner StorageLayoutV1 { self.tag_row }
    pub fn encoding(&self) -> &'owner StorageVariantEncodingV1 { self.encoding }
    pub fn variants(&self) -> &'owner [StorageVariantV1] { self.variants }
    pub fn access(&self) -> MemoryAccess { self.access }
    pub fn result(&self) -> &'owner ValueDef { self.result }
    pub fn requirements(&self) -> [StorageDiscriminantReadRequirementV18; 7] {
        use StorageDiscriminantReadRequirementV18::*;
        [TagBounds, TagInitialization, LifetimeAndProvenance, CurrentEnclosingGuards,
            ValidLogicalDecoding, ReadOrderingAndRaceFreedom, OriginalSourceCorrespondence]
    }
}

/// Resolves inert coordinates in the actual V18 owner. The supplied definition
/// must be the operation's real SSA operand in the same function. No address
/// range, initialized payload or active-variant claim is accepted from callers.
/// The returned borrowed header is retained storage charged to this budget.
pub fn derive_canonical_storage_discriminant_read_v18<'owner>(
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    coordinate: Coordinate,
    definition: Definition,
    budget: &mut Budget<'_>,
) -> Result<CanonicalStorageDiscriminantReadObligationV18<'owner>, CanonicalStorageDiscriminantReadErrorV18> {
    use CanonicalStorageDiscriminantReadErrorV18 as Error;
    let bytes = std::mem::size_of::<CanonicalStorageDiscriminantReadObligationV18<'_>>();
    budget.reserve_storage(bytes)?;
    let result = (|| {
    budget.charge_work(24)?;
    let module = owner.module();
    let function_coordinate = coordinate.block.function;
    let function = module.functions.get(function_coordinate.0 as usize).ok_or(Error::Coordinate)?;
    let body = function.body.as_ref().ok_or(Error::Coordinate)?;
    let operation = body.blocks.get(coordinate.block.block as usize)
        .and_then(|block| block.operations.get(coordinate.operation as usize)).ok_or(Error::Coordinate)?;
    let OperationKind::Storage(StorageOperationV1::ReadDiscriminant { address, access }) = operation.kind else {
        return Err(Error::Operation);
    };
    let (actual, ty) = match definition {
        Definition::FunctionArgument { function, argument } if function == function_coordinate => {
            (*body.parameters.get(argument as usize).ok_or(Error::AddressDefinition)?,
                module.functions[function.0 as usize].signature.parameters.get(argument as usize).ok_or(Error::AddressDefinition)?)
        }
        Definition::BlockArgument { block, argument } if block.function == function_coordinate => {
            let value = body.blocks.get(block.block as usize)
                .and_then(|block| block.parameters.get(argument as usize)).ok_or(Error::AddressDefinition)?;
            (value.id, &value.ty)
        }
        Definition::Result { operation, result } if operation.block.function == function_coordinate => {
            let value = body.blocks.get(operation.block.block as usize)
                .and_then(|block| block.operations.get(operation.operation as usize))
                .and_then(|operation| operation.results.get(result as usize)).ok_or(Error::AddressDefinition)?;
            (value.id, &value.ty)
        }
        _ => return Err(Error::AddressDefinition),
    };
    if actual != address { return Err(Error::AddressDefinition); }
    let Type::Pointer(pointer) = ty else { return Err(Error::AddressDefinition); };
    let Type::StorageObject(layout) = *pointer.pointee else { return Err(Error::Layout); };
    if pointer.access == AccessMode::WriteOnly || pointer.address_space != access.address_space
        || access.volatile || !access.alignment.is_power_of_two()
        || operation.results.len() != 1 || operation.results[0].ty != Type::Scalar(ScalarType::U128)
    { return Err(Error::Operation); }
    let object = module.storage_layouts.get(layout.0 as usize).ok_or(Error::Layout)?;
    let StorageLayoutKindV1::Variants { encoding, variants } = &object.kind else { return Err(Error::Layout); };
    let tag = encoding.tag();
    let tag_row = module.storage_layouts.get(tag.layout.0 as usize).ok_or(Error::Layout)?;
    if tag.offset.checked_add(tag_row.size).is_none_or(|end| end > object.size)
        || tag.placement_alignment(object.alignment).is_none_or(|alignment| access.alignment > alignment)
    { return Err(Error::Layout); }
    Ok(CanonicalStorageDiscriminantReadObligationV18 {
        owner, coordinate, definition, operation, address, pointer, layout, tag, tag_row,
        encoding, variants, access, result: &operation.results[0],
    })
    })();
    if result.is_err() {
        budget.release_storage(bytes)?;
    }
    result
}

#[cfg(test)]
#[path = "storage_discriminant_v18_tests.rs"]
mod tests;
