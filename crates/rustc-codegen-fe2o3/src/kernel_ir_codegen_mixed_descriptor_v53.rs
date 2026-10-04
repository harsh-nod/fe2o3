//! Exact final V18 LLVM plus mandatory mixed descriptor, without authority.
use super::{nominal_v3 as shared, *};
use fe2o3_kernel_descriptor::{
    MIXED_DESCRIPTOR_READER_STORAGE_V53, MixedDescriptorErrorV53, MixedDescriptorTableV53,
    decode_mixed_descriptor_v53,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use sha2::{Digest, Sha256};
use std::{error::Error, mem::size_of};

const PREFIX: &str =
    "\nmodule asm \".section .fe2o3.kd.v53,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
#[derive(Debug)]
pub(crate) enum MixedModuleErrorV53 {
    Resource(Resource),
    Construction(CompilerModuleConstructionError),
    Descriptor(MixedDescriptorErrorV53<Resource>),
    Binding(&'static str),
    Panicked,
}
type E = MixedModuleErrorV53;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mixed module: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Construction(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            _ => None,
        }
    }
}
impl shared::TextError for E {
    fn construction(e: CompilerModuleConstructionError) -> Self {
        Self::Construction(e)
    }
    fn metadata(rule: &'static str) -> Self {
        Self::Binding(rule)
    }
    fn panicked() -> Self {
        Self::Panicked
    }
}

fn identity(bytes: &[u8], budget: &mut Budget<'_>) -> R<[u8; 32]> {
    budget.charge_work(bytes.len().checked_add(64).ok_or(Resource::Arithmetic)?)?;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-MIXED-DESCRIPTOR/V53\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    Ok(hash.finalize().into())
}

fn check_suffix(text: &str, wire: &[u8], budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        text.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(wire.len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    let suffix = shared::suffix_length_using::<E>(wire.len(), PREFIX)?;
    let start = text
        .len()
        .checked_sub(suffix)
        .filter(|n| *n != 0)
        .ok_or(E::Binding("mixed suffix extent"))?;
    let prefix = text
        .get(..start)
        .ok_or(E::Binding("mixed suffix boundary"))?;
    if prefix.contains(".fe2o3.kd.") || text.len() > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
    {
        return Err(E::Binding("complete mixed module"));
    }
    fn take(rest: &mut &[u8], value: &[u8]) -> R<()> {
        *rest = rest
            .strip_prefix(value)
            .ok_or(E::Binding("exact mixed suffix"))?;
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
        return Err(E::Binding("trailing mixed suffix"));
    }
    Ok(())
}

pub(crate) fn check_metadata(
    owner: &Owner,
    module: &InertCompilerModuleTextV1,
    wire: &MixedDescriptorTableV53<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    let retained = shared::module_storage_using::<E>(module, budget)?;
    if budget.storage() < retained {
        return Err(Resource::Accounting.into());
    }
    shared::scoped_using::<_, E>(budget, |budget| {
        let graph = shared::VerifiedTextGraphV53::V18(owner);
        shared::preflight_graph::<E>(graph, budget)?;
        budget.reserve_storage(
            MIXED_DESCRIPTOR_READER_STORAGE_V53
                + shared::DESCRIPTOR_NAMES_HEADER_STORAGE
                + size_of::<Sha256>(),
        )?;
        let decoded =
            decode_mixed_descriptor_v53(wire.canonical_bytes(), &mut |n| budget.charge_work(n))
                .map_err(E::Descriptor)?;
        if module.descriptor_source_identity
            != Some(DescriptorSourceIdentity::Mixed53(identity(
                decoded.canonical_bytes(),
                budget,
            )?))
        {
            return Err(E::Binding("mixed source identity"));
        }
        shared::check_symbols_graph::<E>(graph, module, budget)?;
        shared::check_descriptor_names::<E>(
            module,
            decoded.kernel_count(),
            budget,
            |ordinal, budget| {
                let kernel = decoded
                    .kernel(ordinal, &mut |n| budget.charge_work(n))
                    .map_err(E::Descriptor)?;
                Ok(shared::DescriptorName {
                    entry: kernel.entry_name(),
                    symbol: kernel.descriptor_symbol(),
                })
            },
        )?;
        check_suffix(module.llvm_ir(), wire.canonical_bytes(), budget)
    })
}

/// Retain the exact already-checked final LLVM, with its mandatory contract wire.
/// Caller reserves returned full-capacity storage before any further query.
pub(crate) fn retain_text(
    owner: &Owner,
    llvm: &str,
    wire: &MixedDescriptorTableV53<'_>,
    budget: &mut Budget<'_>,
) -> R<(InertCompilerModuleTextV1, usize)> {
    shared::scoped_using::<_, E>(budget, |budget| {
        let graph = shared::VerifiedTextGraphV53::V18(owner);
        shared::preflight_graph::<E>(graph, budget)?;
        budget.reserve_storage(size_of::<InertCompilerModuleTextV1>() + size_of::<Sha256>())?;
        let symbols = shared::symbols_graph::<E>(graph, budget)?;
        let llvm_ir =
            shared::embedded_text_using::<E>(llvm, wire.canonical_bytes(), PREFIX, budget)?;
        let module = InertCompilerModuleTextV1 {
            llvm_ir,
            kernel_entries: symbols.kernel_entries,
            device_definitions: symbols.device_definitions,
            internal_helpers: symbols.internal_helpers,
            device_ffi_exports: symbols.device_ffi_exports,
            external_declarations: symbols.external_declarations,
            descriptor_source_identity: Some(DescriptorSourceIdentity::Mixed53(identity(
                wire.canonical_bytes(),
                budget,
            )?)),
        };
        check_metadata(owner, &module, wire, budget)?;
        let retained = shared::module_storage_using::<E>(&module, budget)?;
        Ok((module, retained))
    })
}

#[cfg(test)]
#[path = "kernel_ir_codegen_mixed_layout_v60_tests.rs"]
mod layout_tests;
