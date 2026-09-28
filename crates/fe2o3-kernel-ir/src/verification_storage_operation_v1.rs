//! Structural/type checks for the closed storage family.
//! Runtime bounds, initialization, active variants and provenance are not proved here.

use crate::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, DiagnosticCode, MemoryAccess,
    Operation, OperationKind, ScalarType, StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1,
    StorageOperationV1, StorageProjectionV1, StructurallyCheckedModuleStorageV1, Type, ValueId,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1,
};

#[derive(Clone, Copy)]
struct StorageAddressV1 {
    layout: StorageLayoutIdV1,
    space: AddressSpace,
    rights: AccessMode,
}

fn storage_row_v1<'a>(
    storage: &'a StructurallyCheckedModuleStorageV1<'_>,
    id: StorageLayoutIdV1,
    budget: &mut Budget<'_>,
) -> Result<Option<&'a StorageLayoutV1>, ResourceError> {
    budget.charge_work(1)?;
    Ok(storage.layouts().row(id))
}

fn rights_do_not_strengthen_v1(from: AccessMode, to: AccessMode) -> bool {
    from == AccessMode::ReadWrite || from == to
}

/// A finite Type chain selects a view of this table; layout pointer cycles never
/// trigger recursive type construction. An object descriptor is permitted only
/// below a pointer/slice node, never as the represented SSA value itself.
fn storage_value_type_matches_v1(
    storage: &StructurallyCheckedModuleStorageV1<'_>,
    mut layout: StorageLayoutIdV1,
    mut ty: &Type,
    budget: &mut Budget<'_>,
) -> Result<bool, ResourceError> {
    let mut nested = false;
    loop {
        budget.charge_work(2)?;
        let Some(row) = storage.layouts().row(layout) else {
            return Ok(false);
        };
        if let Type::StorageObject(actual) = ty {
            return Ok(nested && *actual == layout);
        }
        match (&row.kind, ty) {
            (StorageLayoutKindV1::Scalar(expected), Type::Scalar(actual)) => {
                return Ok(expected == actual);
            }
            (StorageLayoutKindV1::Vector(expected), Type::Vector(actual)) => {
                return Ok(expected == actual);
            }
            (StorageLayoutKindV1::Pointer(expected), Type::Pointer(actual)) => {
                if expected.value_space != actual.address_space
                    || expected.access != actual.access
                    || (expected.value_space == AddressSpace::Constant
                        && expected.access != AccessMode::ReadOnly)
                {
                    return Ok(false);
                }
                layout = expected.pointee;
                ty = &actual.pointee;
            }
            (
                StorageLayoutKindV1::Slice {
                    element,
                    value_space,
                    access,
                    ..
                },
                Type::Slice(actual),
            ) => {
                if *value_space != actual.address_space
                    || *access != actual.access
                    || (*value_space == AddressSpace::Constant && *access != AccessMode::ReadOnly)
                {
                    return Ok(false);
                }
                layout = *element;
                ty = &actual.element;
            }
            _ => return Ok(false),
        }
        nested = true;
    }
}

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    /// Root's module pass supplies its one retained checked view. This local
    /// reference association is not a persisted pointer token or source proof.
    pub(crate) fn verify_storage_operation_v1(
        &mut self,
        storage: &StructurallyCheckedModuleStorageV1<'module>,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), ResourceError> {
        self.budget.charge_work(1)?;
        if !std::ptr::eq(storage.module(), self.module) {
            return Err(ResourceError::Accounting);
        }
        let OperationKind::Storage(storage_operation) = &operation.kind else {
            return Err(ResourceError::Accounting);
        };
        match *storage_operation {
            StorageOperationV1::Project { base, step } => {
                self.verify_storage_projection_v1(storage, operation, base, step, location)
            }
            StorageOperationV1::ReadValue { address, access } => {
                let Some(address) = self.storage_address_v1(storage, address, location)? else {
                    return Ok(());
                };
                self.storage_access_v1(address, access, false, location)?;
                self.storage_value_result_v1(storage, operation, address.layout, location)
            }
            StorageOperationV1::ReadDiscriminant { address, access } => {
                self.budget.charge_work(2)?;
                if operation.results.len() != 1 {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::ResultArity,
                        "discriminant read requires exactly one U128 result",
                    )?;
                } else if operation.results[0].ty != Type::Scalar(ScalarType::U128) {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::TypeMismatch,
                        "discriminant read returns logical U128 raw bits",
                    )?;
                }
                let Some(address) = self.storage_address_v1(storage, address, location)? else {
                    return Ok(());
                };
                self.storage_access_v1(address, access, false, location)?;
                if access.volatile {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "discriminant read is not a volatile memory operation",
                    )?;
                }
                let row = storage_row_v1(storage, address.layout, self.budget)?
                    .ok_or(ResourceError::Accounting)?;
                let StorageLayoutKindV1::Variants { encoding, .. } = &row.kind else {
                    return self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "discriminant read requires an enum storage layout",
                    );
                };
                self.budget.charge_work(1)?;
                let alignment = encoding
                    .tag()
                    .placement_alignment(row.alignment)
                    .ok_or(ResourceError::Accounting)?;
                if access.alignment > alignment {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "discriminant read overstates its declared placement alignment",
                    )?;
                }
                Ok(())
            }
            StorageOperationV1::WriteValue {
                address,
                value,
                access,
            } => {
                self.expect_no_results_v1(operation, location)?;
                let Some(address) = self.storage_address_v1(storage, address, location)? else {
                    return Ok(());
                };
                self.storage_access_v1(address, access, true, location)?;
                let Some(actual) = self.definition_type_v1(value)? else {
                    return self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "storage write value is not defined",
                    );
                };
                if !storage_value_type_matches_v1(storage, address.layout, actual, self.budget)? {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::TypeMismatch,
                        "storage write requires the exact represented value type",
                    )?;
                }
                Ok(())
            }
            StorageOperationV1::CopyObject {
                source,
                destination,
                source_access,
                destination_access,
                ..
            } => {
                self.expect_no_results_v1(operation, location)?;
                let source = self.storage_address_v1(storage, source, location)?;
                let destination = self.storage_address_v1(storage, destination, location)?;
                let (Some(source), Some(destination)) = (source, destination) else {
                    return Ok(());
                };
                self.storage_access_v1(source, source_access, false, location)?;
                self.storage_access_v1(destination, destination_access, true, location)?;
                self.budget.charge_work(1)?;
                if source.layout != destination.layout {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::TypeMismatch,
                        "storage copy requires the same layout ID in the current module",
                    )?;
                }
                // Both overlap modes still require actual range/lifecycle checks.
                // MayOverlap additionally snapshots representation state before writes.
                Ok(())
            }
            StorageOperationV1::SetDiscriminant {
                address,
                variant,
                access,
            } => {
                self.expect_no_results_v1(operation, location)?;
                let Some(address) = self.storage_address_v1(storage, address, location)? else {
                    return Ok(());
                };
                self.storage_access_v1(address, access, true, location)?;
                self.budget.charge_work(1)?;
                if access.volatile {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "set discriminant is not a volatile memory operation",
                    )?;
                }
                let row = storage_row_v1(storage, address.layout, self.budget)?
                    .ok_or(ResourceError::Accounting)?;
                let StorageLayoutKindV1::Variants { encoding, variants } = &row.kind else {
                    return self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "set discriminant requires an enum storage layout",
                    );
                };
                self.budget.charge_work(2)?;
                if variants
                    .get(variant as usize)
                    .is_none_or(|variant| variant.uninhabited)
                {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "set discriminant requires an inhabited declared variant",
                    )?;
                }
                let alignment = encoding
                    .tag()
                    .placement_alignment(row.alignment)
                    .ok_or(ResourceError::Accounting)?;
                if access.alignment > alignment {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "discriminant store overstates its declared placement alignment",
                    )?;
                }
                // This is structural validation only. In particular the untagged
                // niche case performs no read or runtime initialization proof.
                Ok(())
            }
        }
    }

    fn storage_address_v1(
        &mut self,
        storage: &StructurallyCheckedModuleStorageV1<'module>,
        value: ValueId,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<Option<StorageAddressV1>, ResourceError> {
        let Some(ty) = self.definition_type_v1(value)? else {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidOperandType,
                "storage address is not defined",
            )?;
            return Ok(None);
        };
        self.budget.charge_work(1)?;
        let Type::Pointer(pointer) = ty else {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidOperandType,
                "storage address requires a pointer to a storage object",
            )?;
            return Ok(None);
        };
        let Type::StorageObject(layout) = &*pointer.pointee else {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidOperandType,
                "storage address requires a pointer to a storage object",
            )?;
            return Ok(None);
        };
        let facts = StorageAddressV1 {
            layout: *layout,
            space: pointer.address_space,
            rights: pointer.access,
        };
        if storage_row_v1(storage, facts.layout, self.budget)?.is_none() {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "storage address layout is absent from the current module",
            )?;
            return Ok(None);
        }
        Ok(Some(facts))
    }

    fn storage_access_v1(
        &mut self,
        address: StorageAddressV1,
        access: MemoryAccess,
        write: bool,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), ResourceError> {
        self.budget.charge_work(3)?;
        self.verify_alignment_v1(access.alignment, location)?;
        if access.address_space != address.space {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "storage access space must match the holder address",
            )?;
        }
        let allowed = if write {
            address.space != AddressSpace::Constant
                && matches!(
                    address.rights,
                    AccessMode::WriteOnly | AccessMode::ReadWrite
                )
        } else {
            address.rights != AccessMode::WriteOnly
        };
        if !allowed {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "storage access exceeds the holder pointer's rights",
            )?;
        }
        Ok(())
    }

    fn storage_value_result_v1(
        &mut self,
        storage: &StructurallyCheckedModuleStorageV1<'module>,
        operation: &Operation,
        layout: StorageLayoutIdV1,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), ResourceError> {
        self.budget.charge_work(1)?;
        if operation.results.len() != 1 {
            self.emit_fixed(
                location,
                DiagnosticCode::ResultArity,
                "storage read requires exactly one represented value result",
            )?;
            return Ok(());
        }
        if !storage_value_type_matches_v1(storage, layout, &operation.results[0].ty, self.budget)? {
            self.emit_fixed(
                location,
                DiagnosticCode::TypeMismatch,
                "storage read result does not match the represented value type",
            )?;
        }
        Ok(())
    }

    fn verify_storage_projection_v1(
        &mut self,
        storage: &StructurallyCheckedModuleStorageV1<'module>,
        operation: &Operation,
        base: ValueId,
        step: StorageProjectionV1,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<(), ResourceError> {
        let Some(base) = self.storage_address_v1(storage, base, location)? else {
            return Ok(());
        };
        let row =
            storage_row_v1(storage, base.layout, self.budget)?.ok_or(ResourceError::Accounting)?;
        self.budget.charge_work(2)?;
        let child = match (step, &row.kind) {
            (
                StorageProjectionV1::Field(index),
                StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields),
            ) => fields.get(index as usize).map(|field| field.layout),
            (
                StorageProjectionV1::Field(index),
                StorageLayoutKindV1::Slice { data, length, .. },
            ) => match index {
                0 => Some(data.layout),
                1 => Some(length.layout),
                _ => None,
            },
            (
                StorageProjectionV1::ArrayIndex(index),
                StorageLayoutKindV1::Array {
                    element, length, ..
                },
            ) => {
                self.expect_type_v1(index, &Type::INDEX, location)?;
                // A zero-length array has no admitted in-bounds element.
                (*length != 0).then_some(*element)
            }
            (
                StorageProjectionV1::Variant { index, access },
                StorageLayoutKindV1::Variants { encoding, variants },
            ) => {
                self.storage_access_v1(base, access, false, location)?;
                self.budget.charge_work(1)?;
                let tag = encoding.tag();
                let declared_alignment = tag
                    .placement_alignment(row.alignment)
                    .ok_or(ResourceError::Accounting)?;
                if access.alignment > declared_alignment {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "variant tag read overstates its declared placement alignment",
                    )?;
                }
                variants
                    .get(index as usize)
                    .filter(|variant| !variant.uninhabited)
                    .map(|variant| variant.layout)
            }
            (
                StorageProjectionV1::VariantForWrite { index },
                StorageLayoutKindV1::Variants { variants, .. },
            ) => {
                self.budget.charge_work(2)?;
                if base.space == AddressSpace::Constant
                    || !matches!(base.rights, AccessMode::WriteOnly | AccessMode::ReadWrite)
                {
                    self.emit_fixed(
                        location,
                        DiagnosticCode::InvalidMemoryAccess,
                        "variant construction requires a writable holder",
                    )?;
                }
                variants
                    .get(index as usize)
                    .filter(|variant| !variant.uninhabited)
                    .map(|variant| variant.layout)
            }
            _ => None,
        };
        let Some(child) = child else {
            return self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "storage projection does not name an admitted child layout",
            );
        };
        if storage_row_v1(storage, child, self.budget)?.is_none() {
            return Err(ResourceError::Accounting);
        }
        self.budget.charge_work(1)?;
        if operation.results.len() != 1 {
            return self.emit_fixed(
                location,
                DiagnosticCode::ResultArity,
                "storage projection requires exactly one pointer result",
            );
        }
        let matches = match &operation.results[0].ty {
            Type::Pointer(pointer) => {
                pointer.address_space == base.space
                    && rights_do_not_strengthen_v1(base.rights, pointer.access)
                    && (!matches!(step, StorageProjectionV1::VariantForWrite { .. })
                        || pointer.access == AccessMode::WriteOnly)
                    && matches!(&*pointer.pointee,
                        Type::StorageObject(actual) if *actual == child)
            }
            _ => false,
        };
        if !matches {
            self.emit_fixed(
                location,
                DiagnosticCode::TypeMismatch,
                "storage projection must preserve child layout, holder space and access rights",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "verification_storage_operation_v1_tests.rs"]
mod tests;
