//! Common nominal V3 module transport with genuine expanded-final custody.
#![allow(clippy::result_large_err, reason = "Keep typed child causes inline.")]
#![allow(clippy::drop_non_drop, reason = "End borrowed views before refunds.")]
use super::descriptor;
use super::{ExpandedFinalNativeCustodyV3, ExpandedNativeProductionCompilationV3 as Input};
use crate::kernel_ir_codegen::{InertCompilerModuleTextV1 as Module, nominal_v3 as module};
use dialect_amdgcn::{
    NativeV12TextDescriptorReplayErrorV3 as NativeError,
    check_native_v12_text_descriptor_relation_v3,
};
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
    COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3, CompilerDescriptorSourceErrorV3,
    CompilerDescriptorSourceV3 as Source, compiler_descriptor_source_validation_storage_v3,
};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    MAX_DESCRIPTOR_TABLE_BYTES,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    SourcePipelineCatalogCallbackErrorV1, with_checked_source_pipeline_catalog_v1,
};
use std::{error::Error, fmt, mem::size_of};

#[derive(Debug)]
pub(crate) enum ExpandedNativeTransportErrorV3 {
    Resource(Resource),
    Expanded(descriptor::E),
    Module(module::NominalModuleErrorV3),
    Source(CompilerDescriptorSourceErrorV3<Resource>),
    Catalog(SourcePipelineCatalogCallbackErrorV1<NativeError>),
    Mismatch(&'static str),
    Panicked,
}
type E = ExpandedNativeTransportErrorV3;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expanded-final native transport: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Expanded(e) => Some(e),
            Self::Module(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Catalog(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpandedNativeTransportStorageV3(usize);
impl ExpandedNativeTransportStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only custody plus exact final Source/Module bytes. This is not a
/// publication, producer-signature, loader or execution receipt.
pub(crate) struct ExpandedNativeTransportV3 {
    custody: ExpandedFinalNativeCustodyV3,
    source: Source,
    module: Module,
    module_storage: usize,
    input_floor: usize,
    retained_floor: usize,
}
type Output = ExpandedNativeTransportV3;
const MAX_SOURCE_CAPACITY: usize = 2 * MAX_DESCRIPTOR_TABLE_BYTES;
fn header_addition() -> R<usize> {
    size_of::<Output>()
        .checked_sub(size_of::<Input>())
        .ok_or(Resource::Accounting.into())
}
fn retained_addition(storage: usize) -> R<usize> {
    header_addition()?
        .checked_add(
            storage
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Accounting)?,
        )
        .ok_or(Resource::Arithmetic.into())
}
fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    descriptor::scoped(budget, || E::Panicked, run)
}

impl Input {
    /// Moves the only final owner and original canonical Vec backing. The old
    /// receipt remains paid on success and failure; reserve only the addition
    /// on success, and retire the old receipt once after consuming failure.
    pub(crate) fn into_expanded_descriptor_transport_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(Output, ExpandedNativeTransportStorageV3)> {
        let incoming = budget.storage();
        scoped(budget, move |budget| {
            budget.charge_work(4)?;
            if incoming < self.retained_floor || self.wire.capacity() > MAX_SOURCE_CAPACITY {
                return Err(E::Mismatch("paid bounded expanded descriptor backing"));
            }
            self.verify_equivalence(budget).map_err(E::Expanded)?;
            let before_module = header_addition()?
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Accounting)?;
            budget.reserve_storage(before_module)?;
            let Input { custody, wire, .. } = self;
            let validation = compiler_descriptor_source_validation_storage_v3(wire.capacity())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)?;
            let source = Source::from_owned_canonical_bytes(wire, validation, &mut |n| {
                budget.charge_work(n)
            })
            .map_err(E::Source)?;
            budget.release_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)?;
            let (module, receipt) = module::retain_nominal_compiler_module_text_v3(
                custody.owner.output(),
                &custody.llvm,
                &source,
                budget,
            )
            .map_err(E::Module)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let additional = retained_addition(receipt.retained_storage())?;
            if before_module.checked_add(receipt.retained_storage()) != Some(additional) {
                return Err(Resource::Accounting.into());
            }
            let value = Output {
                custody,
                source,
                module,
                module_storage: receipt.retained_storage(),
                input_floor: incoming,
                retained_floor: incoming
                    .checked_add(additional)
                    .ok_or(Resource::Arithmetic)?,
            };
            value.verify_equivalence(budget)?;
            Ok((value, ExpandedNativeTransportStorageV3(additional)))
        })
    }
}

impl Output {
    pub(crate) fn output(&self) -> &Graph {
        self.custody.owner.output()
    }
    pub(crate) fn module(&self) -> &Module {
        &self.module
    }
    pub(crate) fn descriptor_source(&self) -> &Source {
        &self.source
    }
    pub(crate) const fn retained_storage_floor_v3(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }

    /// Replay original source, every prefix/scalar pair and fresh final ABI,
    /// then independently check actual stored tags, symbols, physical layout,
    /// target requirements and native bytes against that same final graph.
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            let capacity = self
                .source
                .storage()
                .retained_storage()
                .checked_sub(COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3)
                .ok_or(Resource::Accounting)?;
            if capacity > MAX_SOURCE_CAPACITY {
                return Err(E::Mismatch("bounded final descriptor capacity"));
            }
            let actual = module::module_storage(&self.module, budget).map_err(E::Module)?;
            if actual != self.module_storage
                || self.input_floor.checked_add(retained_addition(actual)?)
                    != Some(self.retained_floor)
            {
                return Err(E::Mismatch("exact cumulative expanded native receipt"));
            }
            self.custody
                .verify(self.source.canonical_bytes(), self.retained_floor, budget)
                .map_err(E::Expanded)?;
            module::check_nominal_compiler_module_metadata_v3(
                self.output(),
                &self.module,
                &self.source,
                budget,
            )
            .map_err(E::Module)?;
            budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)?;
            let extent = self
                .source
                .storage()
                .retained_storage()
                .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
                .ok_or(Resource::Arithmetic)?;
            let table = self
                .source
                .table(extent, &mut |n| budget.charge_work(n))
                .map_err(E::Source)?;
            budget.release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)?;
            with_checked_source_pipeline_catalog_v1(
                self.custody.owner.source_anchor(),
                budget,
                |view, budget| {
                    let catalog = view.catalog(budget).map_err(NativeError::Resource)?;
                    let relation = check_native_v12_text_descriptor_relation_v3(
                        self.output(),
                        catalog,
                        self.output().canonical().canonical_bytes(),
                        self.custody.profile,
                        &table,
                        self.module.llvm_ir(),
                        budget,
                    )?;
                    budget
                        .reserve_storage(relation.storage().retained_storage())
                        .map_err(NativeError::Resource)?;
                    let prefix = relation.pre_descriptor_llvm();
                    budget
                        .charge_work(
                            prefix
                                .len()
                                .checked_add(self.custody.llvm.len())
                                .ok_or(Resource::Arithmetic)?,
                        )
                        .map_err(NativeError::Resource)?;
                    if prefix != self.custody.llvm {
                        return Err(NativeError::Invalid(
                            "retained actual expanded-final native prefix",
                        ));
                    }
                    drop(relation);
                    Ok::<_, NativeError>(())
                },
            )
            .map_err(E::Catalog)?;
            drop(table);
            budget.release_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3)?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "production_expanded_native_transport_v3_tests.rs"]
mod tests;

#[path = "production_expanded_history_serializer_v1.rs"]
pub(crate) mod serialized_history;
