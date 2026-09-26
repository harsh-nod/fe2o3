//! Internal context for the shared verifier, never an alternate module owner.

use crate::{
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, CastKind, DiagnosticCode,
    Module, Operation, OperationKind, StructurallyCheckedModuleStorageV1, Type,
    VerificationDiagnosticLocationV1, VerificationFunctionPassV1, verification_type_facts_v15,
};

#[derive(Clone, Copy)]
pub(crate) enum VerificationStorageContextV1<'checked, 'module> {
    Legacy(&'module Module),
    Storage(&'checked StructurallyCheckedModuleStorageV1<'module>),
}

impl<'checked, 'module> VerificationStorageContextV1<'checked, 'module> {
    pub(crate) fn module(self) -> &'module Module {
        match self {
            Self::Legacy(module) => module,
            Self::Storage(storage) => storage.module(),
        }
    }

    pub(crate) fn storage(self) -> Option<&'checked StructurallyCheckedModuleStorageV1<'module>> {
        match self {
            Self::Legacy(_) => None,
            Self::Storage(storage) => Some(storage),
        }
    }
}

impl<'a, 'module, 'work> VerificationFunctionPassV1<'a, 'module, 'work> {
    pub(crate) fn verify_storage_allocation_element_v1(
        &mut self,
        storage: &StructurallyCheckedModuleStorageV1<'module>,
        element: &Type,
        alignment: u32,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<bool, ResourceError> {
        self.budget.charge_work(1)?;
        if !std::ptr::eq(self.module, storage.module()) {
            return Err(ResourceError::Accounting);
        }
        let facts = verification_type_facts_v15(element, self.budget)?;
        let Some(id) = facts.storage_object else {
            return Ok(facts.storable);
        };
        if !facts.bare_storage_object {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "storage-bearing allocation elements require an explicit root layout",
            )?;
            return Ok(false);
        }
        self.budget.charge_work(1)?;
        let Some(row) = storage.layouts().row(id) else {
            return Ok(false);
        };
        self.budget.charge_work(1)?;
        if alignment < row.alignment {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidAlignment,
                "storage allocation alignment is smaller than its checked layout alignment",
            )?;
        }
        Ok(true)
    }

    pub(crate) fn reject_storage_legacy_bridge_v1(
        &mut self,
        operation: &Operation,
        location: &VerificationDiagnosticLocationV1<'_>,
    ) -> Result<bool, ResourceError> {
        // These consumers preserve exact types or have their own table-aware
        // checks. Pointer casts still run their exact pointee/access/space rule.
        if matches!(
            operation.kind,
            OperationKind::Storage(_)
                | OperationKind::Alloca { .. }
                | OperationKind::WorkgroupMemory(_)
                | OperationKind::Select { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Call { .. }
                | OperationKind::SliceLength { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::VerificationContract(_)
                | OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess | CastKind::PointerToGeneric | CastKind::SliceToGeneric,
                    ..
                }
        ) {
            return Ok(false);
        }
        let mut contains = false;
        operation.kind.try_visit_operands(|value| {
            self.budget.charge_work(1)?;
            if let Some(ty) = self.definition_type_v1(value)? {
                contains |= verification_type_facts_v15(ty, self.budget)?
                    .storage_object
                    .is_some();
            }
            Ok::<(), ResourceError>(())
        })?;
        self.budget.charge_work(operation.results.len())?;
        for result in &operation.results {
            contains |= verification_type_facts_v15(&result.ty, self.budget)?
                .storage_object
                .is_some();
        }
        if contains {
            self.emit_fixed(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "generic operations cannot bridge or erase a storage layout; use the typed storage family",
            )?;
        }
        Ok(contains)
    }
}
