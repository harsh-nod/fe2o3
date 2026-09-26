//! Paid conditional V5 text and metadata only, never proof or native authority.
use super::{nominal_v3 as shared, *};
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5, COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5,
    CompilerDescriptorSourceErrorV5, CompilerDescriptorSourceIdentityV5,
    CompilerDescriptorSourceV5 as Source,
};
use fe2o3_kernel_descriptor::{DESCRIPTOR_QUERY_STORAGE_V5, DescriptorWireErrorV5};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{error::Error, mem::size_of};

const PREFIX: &str =
    "\nmodule asm \".section .fe2o3.kd.v5,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";

#[derive(Debug)]
pub(crate) enum ConditionalModuleErrorV5 {
    Resource(Resource),
    Construction(CompilerModuleConstructionError),
    Source(CompilerDescriptorSourceErrorV5<Resource>),
    Descriptor(DescriptorWireErrorV5<Resource>),
    Metadata(&'static str),
    Panicked,
}
type E = ConditionalModuleErrorV5;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl shared::TextError for E {
    fn construction(error: CompilerModuleConstructionError) -> Self {
        Self::Construction(error)
    }
    fn metadata(rule: &'static str) -> Self {
        Self::Metadata(rule)
    }
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional compiler module V5: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Construction(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            _ => None,
        }
    }
}

/// Full module header plus actual text/vector/name capacities, UNRESERVED.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConditionalModuleStorageV5(usize);
impl ConditionalModuleStorageV5 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

fn revalidate_source(source: &Source, budget: &mut Budget<'_>) -> R<()> {
    let scratch = COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5;
    let extent = source
        .storage()
        .retained_storage()
        .checked_add(scratch)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    source
        .revalidate(extent, &mut |w| budget.charge_work(w))
        .map_err(E::Source)?;
    budget.release_storage(scratch)?;
    Ok(())
}

fn check_descriptor_symbols(
    module: &InertCompilerModuleTextV1,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<()> {
    let floor = budget.storage();
    {
        budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5)?;
        let extent = source
            .storage()
            .retained_storage()
            .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5)
            .ok_or(Resource::Arithmetic)?;
        let table = source
            .table(extent, &mut |w| budget.charge_work(w))
            .map_err(E::Source)?;
        budget.reserve_storage(
            DESCRIPTOR_QUERY_STORAGE_V5 + shared::DESCRIPTOR_NAMES_HEADER_STORAGE,
        )?;
        shared::check_descriptor_names::<E>(
            module,
            table.kernel_count(),
            budget,
            |index, budget| {
                let row = table
                    .kernel(index, &mut |w| budget.charge_work(w))
                    .map_err(E::Descriptor)?;
                Ok(shared::DescriptorName {
                    entry: row.entry_name(),
                    symbol: row.descriptor_symbol(),
                })
            },
        )?;
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

// Allocation-free reader of the exact emitted suffix. This does not emit LLVM
// or regenerate stored metadata from the producer's original symbol closure.
fn check_suffix(text: &str, wire: &[u8], budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        text.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(wire.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if text.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES {
        return Err(E::Metadata("complete native text bound"));
    }
    let suffix = shared::suffix_length_using::<E>(wire.len(), PREFIX)?;
    let start = text
        .len()
        .checked_sub(suffix)
        .filter(|&n| n != 0)
        .ok_or(E::Metadata("complete V5 descriptor suffix"))?;
    let prefix = text.get(..start).ok_or(E::Metadata("V5 suffix boundary"))?;
    if prefix.contains(".fe2o3.kd.") {
        return Err(E::Metadata("descriptor section already present"));
    }
    fn take(rest: &mut &[u8], expected: &[u8]) -> R<()> {
        *rest = rest
            .strip_prefix(expected)
            .ok_or(E::Metadata("exact V5 descriptor suffix"))?;
        Ok(())
    }
    let mut rest = &text.as_bytes()[start..];
    take(&mut rest, PREFIX.as_bytes())?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for chunk in wire.chunks(16) {
        take(&mut rest, b"module asm \".byte ")?;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
                take(&mut rest, b", ")?;
            }
            take(&mut rest, b"0x")?;
            take(
                &mut rest,
                &[HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 15)]],
            )?;
        }
        take(&mut rest, b"\"\n")?;
    }
    if !rest.is_empty() {
        return Err(E::Metadata("trailing V5 descriptor bytes"));
    }
    Ok(())
}

/// Checks actual stored roles, typed V5 identity and the complete exact suffix.
/// Canonical LLVM/F semantics and original source provenance remain separate.
pub(crate) fn check_conditional_compiler_module_metadata_v5(
    actual_f: &Owner,
    module: &InertCompilerModuleTextV1,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<()> {
    // Check the entry floor, not a later scope's scratch reservation.
    let retained = shared::module_storage_using::<E>(module, budget)?
        .checked_add(source.storage().retained_storage())
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < retained {
        return Err(Resource::Accounting.into());
    }
    shared::scoped_using::<_, E>(budget, |budget| {
        shared::preflight::<E>(actual_f, budget)?;
        revalidate_source(source, budget)?;
        budget.charge_work(size_of::<CompilerDescriptorSourceIdentityV5>() + 2)?;
        if module.descriptor_source_identity
            != Some(DescriptorSourceIdentity::V5(source.identity()))
        {
            return Err(E::Metadata("V5 binding tag/identity"));
        }
        shared::check_symbols::<E>(actual_f, module, budget)?;
        check_descriptor_symbols(module, source, budget)?;
        check_suffix(module.llvm_ir(), source.canonical_bytes(), budget)
    })
}

/// Retains the prefix from the existing canonical LLVM engine, never lowers F
/// again. All inputs remain prepaid; reserve the returned full-capacity receipt
/// before further use. Inert text/metadata alone grants no proof/native authority.
pub(crate) fn retain_conditional_compiler_module_text_v5(
    actual_f: &Owner,
    canonical_llvm_prefix: &str,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<(InertCompilerModuleTextV1, ConditionalModuleStorageV5)> {
    let required = source
        .storage()
        .retained_storage()
        .checked_add(canonical_llvm_prefix.len())
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < required {
        return Err(Resource::Accounting.into());
    }
    shared::scoped_using::<_, E>(budget, |budget| {
        shared::preflight::<E>(actual_f, budget)?;
        revalidate_source(source, budget)?;
        budget.reserve_storage(size_of::<InertCompilerModuleTextV1>())?;
        let closure = shared::symbols::<E>(actual_f, budget)?;
        let llvm_ir = shared::embedded_text_using::<E>(
            canonical_llvm_prefix,
            source.canonical_bytes(),
            PREFIX,
            budget,
        )?;
        let module = InertCompilerModuleTextV1 {
            llvm_ir,
            kernel_entries: closure.kernel_entries,
            device_definitions: closure.device_definitions,
            internal_helpers: closure.internal_helpers,
            device_ffi_exports: closure.device_ffi_exports,
            external_declarations: closure.external_declarations,
            descriptor_source_identity: Some(DescriptorSourceIdentity::V5(source.identity())),
        };
        check_conditional_compiler_module_metadata_v5(actual_f, &module, source, budget)?;
        let storage =
            ConditionalModuleStorageV5(shared::module_storage_using::<E>(&module, budget)?);
        Ok((module, storage))
    })
}

#[cfg(test)]
#[path = "kernel_ir_codegen_conditional_descriptor_v5_tests.rs"]
mod tests;
