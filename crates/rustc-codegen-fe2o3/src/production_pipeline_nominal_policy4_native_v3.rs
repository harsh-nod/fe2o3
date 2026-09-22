//! Consume the existing nominal P4 O custodian; no optimizer or worker activation.
#![allow(clippy::result_large_err, clippy::drop_non_drop)]

use super::{Output as Admitted, PreparedNominalPolicy4AbiV3 as Input};
use crate::compiler_descriptor::nominal_v3::NominalDescriptorErrorV3;
use crate::kernel_ir_codegen::{InertCompilerModuleTextV1 as Module, nominal_v3 as module};
use crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1;
use crate::production_pipeline::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1;
use dialect_amdgcn::{
    NativeV12TextDescriptorReplayErrorV3 as NativeError,
    check_native_v12_text_descriptor_relation_v3,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3, CompilerDescriptorSourceErrorV3,
    CompilerDescriptorSourceV3,
};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, SourcePipelineCatalogCallbackErrorV1,
    with_checked_source_pipeline_catalog_v1,
};
use std::{
    any::Any,
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub(crate) enum NominalPolicy4NativeErrorV3 {
    Resource(Resource),
    Nominal(NominalDescriptorErrorV3),
    Module(module::NominalModuleErrorV3),
    Source(CompilerDescriptorSourceErrorV3<Resource>),
    Native(NativeError),
    Catalog(SourcePipelineCatalogCallbackErrorV1<NativeError>),
    SourceProof(NativeSourceLineageErrorV1),
    SourceBinding(SourcePipelineCatalogCallbackErrorV1<NativeOutputHandoffErrorV1>),
    Mismatch(&'static str),
    Panicked,
}
type E = NominalPolicy4NativeErrorV3;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(message) => write!(f, "nominal P4 native transport: {message}"),
            _ => write!(f, "nominal P4 native transport: {self:?}"),
        }
    }
}

#[path = "production_pipeline_nominal_policy4_source_proof_v3.rs"]
mod source_proof;
#[cfg(test)]
#[path = "production_pipeline_nominal_policy4_native_v3_tests.rs"]
mod tests;
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Nominal(e) => Some(e),
            Self::Module(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Native(e) => Some(e),
            Self::Catalog(e) => Some(e),
            Self::SourceProof(e) => Some(e),
            Self::SourceBinding(e) => Some(e),
            Self::Mismatch(_) | Self::Panicked => None,
        }
    }
}

/// Only the addition to the consumed, separately prepaid input owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalPolicy4NativeStorageV3(usize);
impl NominalPolicy4NativeStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedNominalPolicy4NativeV3 {
    input: Input,
    module: Module,
    module_storage: usize,
    input_floor: usize,
    retained_floor: usize,
}
type Output = PreparedNominalPolicy4NativeV3;

const SCOPE: usize = 2 * size_of::<usize>()
    + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<[Option<Box<dyn Any + Send>>; 2]>();

fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let paid = SCOPE
        .checked_add(size_of::<R<T>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(E::Panicked)
        }
    };
    let same =
        budget.work_ledger_identity_v1() == ledger && slot == budget as *const Budget<'w> as usize;
    if !same
        || floor
            .checked_add(paid)
            .is_none_or(|minimum| budget.storage() < minimum)
    {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if same && budget.storage() >= floor {
        if let Err(error) = budget.release_storage(budget.storage() - floor) {
            let rejected = std::mem::replace(&mut result, Err(error.into()));
            payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
        }
    }
    drop(payloads);
    result
}

fn header_addition() -> R<usize> {
    size_of::<Output>()
        .checked_sub(size_of::<Input>())
        .and_then(|n| n.checked_sub(size_of::<Module>()))
        .ok_or(Resource::Accounting.into())
}
fn retained_addition(module_storage: usize) -> R<usize> {
    if module_storage < size_of::<Module>() {
        return Err(Resource::Accounting.into());
    }
    header_addition()?
        .checked_add(module_storage)
        .ok_or(Resource::Arithmetic.into())
}

// Existing native/layout engines retain their own internal bounds. Prepay their
// visible output extents, then keep each actual String capacity reserved.
fn engine_text(
    maximum: usize,
    budget: &mut Budget<'_>,
    run: impl FnOnce() -> Result<String, NativeError>,
) -> R<(String, usize)> {
    budget.charge_work(1)?;
    let requested = maximum
        .checked_add(size_of::<String>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let text = run().map_err(E::Native)?;
    let actual = text
        .capacity()
        .checked_add(size_of::<String>())
        .ok_or(Resource::Arithmetic)?;
    if actual >= requested {
        budget.reserve_storage(actual - requested)?;
    } else {
        budget.release_storage(requested - actual)?;
    }
    if text.len() > maximum {
        return Err(E::Mismatch("native text bound"));
    }
    Ok((text, actual))
}

fn emit_module(input: &Input, budget: &mut Budget<'_>) -> R<(Module, usize)> {
    scoped(budget, |budget| {
        let (dialect, dialect_storage) = engine_text(
            dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
            budget,
            || {
                match input.bindings.rustc_target.profile() {
                    Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(input.output()),
                    Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(input.output()),
                }.map_err(NativeError::Lowering)
            },
        )?;
        let (prefix, prefix_storage) = engine_text(
            dialect_amdgcn::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1,
            budget,
            || {
                dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&dialect)
                    .map_err(NativeError::Layout)
            },
        )?;
        drop(dialect);
        budget.release_storage(dialect_storage)?;
        let (module, receipt) = module::retain_nominal_compiler_module_text_v3(
            input.output(),
            &prefix,
            &input.descriptor,
            budget,
        )
        .map_err(E::Module)?;
        budget.reserve_storage(receipt.retained_storage())?;
        drop(prefix);
        budget.release_storage(prefix_storage)?;
        Ok((module, receipt.retained_storage()))
    })
}

impl Input {
    /// The original input receipt stays paid and transfers on success. Reserve
    /// only the returned addition. On failure the consumed input is dropped;
    /// its caller must retire the original reservation exactly once afterward.
    pub(crate) fn into_nominal_native_transport_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(Output, NominalPolicy4NativeStorageV3)> {
        let incoming = budget.storage();
        if incoming < self.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, move |budget| {
            budget.charge_work(4)?;
            self.verify_equivalence(budget).map_err(E::Nominal)?;
            budget.reserve_storage(header_addition()?)?;
            let (module, module_storage) = emit_module(&self, budget)?;
            budget.reserve_storage(module_storage)?;
            let additional = retained_addition(module_storage)?;
            let value = Output {
                input: self,
                module,
                module_storage,
                input_floor: incoming,
                retained_floor: incoming
                    .checked_add(additional)
                    .ok_or(Resource::Arithmetic)?,
            };
            #[cfg(test)]
            tests::factory_checkpoint()?;
            value.verify_equivalence(budget)?;
            Ok((value, NominalPolicy4NativeStorageV3(additional)))
        })
    }
}

impl Output {
    pub(crate) fn output(&self) -> &Graph {
        self.input.output()
    }
    pub(crate) fn module(&self) -> &Module {
        &self.module
    }
    pub(crate) fn descriptor_source(&self) -> &CompilerDescriptorSourceV3 {
        &self.input.descriptor
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
    fn source_anchor(&self) -> Anchor<'_> {
        match &self.input.admitted {
            Admitted::Direct(owner) => Anchor::Direct(owner.source_semantic_kir()),
            Admitted::Erased(owner) => Anchor::Erased(owner.erased_source()),
        }
    }

    /// Replays original source-to-O before independent catalog, physical ABI,
    /// capability, V3 tag, complete symbol roster and exact native-text checks.
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            let actual = module::module_storage(&self.module, budget).map_err(E::Module)?;
            if actual != self.module_storage
                || self.input_floor < self.input.retained_storage_floor_v1()
                || self.input_floor.checked_add(retained_addition(actual)?)
                    != Some(self.retained_floor)
            {
                return Err(E::Mismatch("exact cumulative P4 native receipt"));
            }
            self.input.verify_equivalence(budget).map_err(E::Nominal)?;
            module::check_nominal_compiler_module_metadata_v3(
                self.output(),
                &self.module,
                &self.input.descriptor,
                budget,
            )
            .map_err(E::Module)?;
            budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)?;
            let extent = self
                .input
                .descriptor
                .storage()
                .retained_storage()
                .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
                .ok_or(Resource::Arithmetic)?;
            let table = self
                .input
                .descriptor
                .table(extent, &mut |w| budget.charge_work(w))
                .map_err(E::Source)?;
            budget.release_storage(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)?;
            with_checked_source_pipeline_catalog_v1(
                self.source_anchor(),
                budget,
                |view, budget| {
                    let catalog = view.catalog(budget).map_err(NativeError::Resource)?;
                    let relation = check_native_v12_text_descriptor_relation_v3(
                        self.output(),
                        catalog,
                        self.output().canonical().canonical_bytes(),
                        self.input.bindings.rustc_target.profile(),
                        &table,
                        self.module.llvm_ir(),
                        budget,
                    )?;
                    budget
                        .reserve_storage(relation.storage().retained_storage())
                        .map_err(NativeError::Resource)?;
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
