//! Staged genuine-U consumer of the shared nominal V3 transport, not a default.
#![allow(
    clippy::result_large_err,
    reason = "Keep original typed child causes inline."
)]
#![allow(
    clippy::drop_non_drop,
    reason = "End borrowed witness lifetimes before refunds."
)]

use super::{
    E as NominalError, NominalFinalNativeCustodyV3,
    NominalLoopUnrollNativeProductionCompilationV3 as Input, Unrolled,
};
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

#[cfg(test)]
#[path = "production_nominal_native_transport_resource_v3_tests.rs"]
pub(crate) mod nominal_native_transport_resource_v3;

#[derive(Debug)]
pub(crate) enum NominalNativeTransportErrorV3 {
    Resource(Resource),
    Nominal(NominalError),
    Module(module::NominalModuleErrorV3),
    Source(CompilerDescriptorSourceErrorV3<Resource>),
    Catalog(SourcePipelineCatalogCallbackErrorV1<NativeError>),
    Mismatch(&'static str),
    Panicked,
}
type E = NominalNativeTransportErrorV3;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nominal native transport V3: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Nominal(e) => Some(e),
            Self::Module(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Catalog(e) => Some(e),
            _ => None,
        }
    }
}

/// Only the addition to the already reserved consumed nominal-U owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalNativeTransportStorageV3(usize);
impl NominalNativeTransportStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// A genuine staged U custodian plus common V3 descriptor/module transport.
/// This does not select a production policy or grant signed/native authority.
/// A later expanded-history final-subject adapter reuses the same Source,
/// Module and independent model checker before its first LLVM emission; it must
/// not relabel this particular U custodian as an expanded final subject.
pub(crate) struct NominalLoopUnrollDescriptorTransportV3 {
    custody: NominalFinalNativeCustodyV3,
    source: Source,
    module: Module,
    module_storage: usize,
    input_floor: usize,
    retained_floor: usize,
}
type Output = NominalLoopUnrollDescriptorTransportV3;

const SCOPE: usize = 2 * size_of::<usize>()
    + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<[Option<Box<dyn Any + Send>>; 2]>();
const MAX_SOURCE_CAPACITY: usize = 2 * MAX_DESCRIPTOR_TABLE_BYTES;

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
        Ok(r) => r,
        Err(p) => {
            payloads[0] = Some(p);
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
        .ok_or(Resource::Accounting.into())
}
fn retained_addition(module_storage: usize) -> R<usize> {
    header_addition()?
        .checked_add(
            module_storage
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Accounting)?,
        )
        .ok_or(Resource::Arithmetic.into())
}

impl Input {
    /// Consume the only real U owner and transfer its existing Vec backing.
    /// The incoming owner receipt remains reserved throughout and transfers on
    /// success; reserve only the returned addition. On any consuming failure,
    /// retire that original receipt exactly once after this call returns.
    pub(crate) fn into_nominal_descriptor_transport_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(Output, NominalNativeTransportStorageV3)> {
        let incoming = budget.storage();
        scoped(budget, move |budget| {
            budget.charge_work(4)?;
            if incoming < self.retained_floor || self.wire.capacity() > MAX_SOURCE_CAPACITY {
                return Err(E::Mismatch("paid bounded original descriptor backing"));
            }
            self.verify_equivalence(budget).map_err(E::Nominal)?;
            // H_new-H_old includes Source replacing Vec and the embedded Module
            // header. Let the module builder pay that one header later.
            let before_module = header_addition()?
                .checked_sub(size_of::<Module>())
                .ok_or(Resource::Accounting)?;
            budget.reserve_storage(before_module)?;
            let Input { custody, wire, .. } = self;
            let validation = compiler_descriptor_source_validation_storage_v3(wire.capacity())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)?;
            let source = Source::from_owned_canonical_bytes(wire, validation, &mut |w| {
                budget.charge_work(w)
            })
            .map_err(E::Source)?;
            budget.release_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)?;
            let (module, module_receipt) = module::retain_nominal_compiler_module_text_v3(
                custody.native.output(),
                custody.native.llvm_ir(),
                &source,
                budget,
            )
            .map_err(E::Module)?;
            budget.reserve_storage(module_receipt.retained_storage())?;
            let additional = retained_addition(module_receipt.retained_storage())?;
            if before_module.checked_add(module_receipt.retained_storage()) != Some(additional) {
                return Err(Resource::Accounting.into());
            }
            let retained_floor = incoming
                .checked_add(additional)
                .ok_or(Resource::Arithmetic)?;
            let value = Output {
                custody,
                source,
                module,
                module_storage: module_receipt.retained_storage(),
                input_floor: incoming,
                retained_floor,
            };
            value.verify_equivalence(budget)?;
            Ok((value, NominalNativeTransportStorageV3(additional)))
        })
    }
}

impl Output {
    pub(crate) fn output(&self) -> &Graph {
        self.custody.native.output()
    }
    pub(crate) fn forwarding_output(&self) -> &Graph {
        self.custody.native.forwarding_output()
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

    #[cfg(test)]
    pub(crate) fn source_test_mutations_v3(&mut self, budget: &mut Budget<'_>) -> R<()> {
        let floor = budget.storage();
        self.module_storage += 1;
        let receipt_result = self.verify_equivalence(budget);
        self.module_storage -= 1;
        if !matches!(
            receipt_result,
            Err(E::Mismatch("exact cumulative native transport receipt"))
        ) {
            return Err(E::Mismatch("transport receipt donor refusal"));
        }
        let old = self.module.replace_first_ascii_byte_for_test_v3(b'!');
        let native_result = self.verify_equivalence(budget);
        self.module.replace_first_ascii_byte_for_test_v3(old);
        if !matches!(
            native_result,
            Err(E::Catalog(SourcePipelineCatalogCallbackErrorV1::Callback(
                NativeError::Invalid(_)
            )))
        ) {
            return Err(E::Mismatch("independent embedded native donor refusal"));
        }
        if budget.storage() != floor {
            return Err(Resource::Accounting.into());
        }
        self.verify_equivalence(budget)
    }

    fn source_anchor(&self) -> Anchor<'_> {
        match &self.custody.native.owner {
            Unrolled::Direct(v) => Anchor::Direct(
                v.prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .source_semantic_kir(),
            ),
            Unrolled::Erased(v) => Anchor::Erased(
                v.prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .prefix()
                    .erased_source(),
            ),
        }
    }

    /// Fresh genuine source/F/U/target/ranked and nominal ABI replay precedes
    /// independent stored-tag/all-five-vector/physical/requirements/native checks.
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
                return Err(E::Mismatch("bounded transferred V3 capacity"));
            }
            let actual = module::module_storage(&self.module, budget).map_err(E::Module)?;
            if actual != self.module_storage
                || self.input_floor.checked_add(retained_addition(actual)?)
                    != Some(self.retained_floor)
            {
                return Err(E::Mismatch("exact cumulative native transport receipt"));
            }
            self.custody
                .with_checked_table(
                    self.source.canonical_bytes(),
                    self.retained_floor,
                    budget,
                    |_, _| Ok(()),
                )
                .map_err(E::Nominal)?;
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
                        self.custody.native.profile,
                        &table,
                        self.module.llvm_ir(),
                        budget,
                    )?;
                    budget
                        .reserve_storage(relation.storage().retained_storage())
                        .map_err(NativeError::Resource)?;
                    let prefix = relation.pre_descriptor_llvm();
                    let original = self.custody.native.llvm_ir();
                    budget
                        .charge_work(
                            prefix
                                .len()
                                .checked_add(original.len())
                                .ok_or(Resource::Arithmetic)?,
                        )
                        .map_err(NativeError::Resource)?;
                    if prefix != original {
                        return Err(NativeError::Invalid("retained actual U native prefix"));
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
pub(crate) mod test_support {
    use super::*;
    pub(crate) const fn scope_header() -> usize {
        SCOPE
    }
    pub(crate) fn addition(module_storage: usize) -> R<usize> {
        retained_addition(module_storage)
    }
    pub(crate) fn scope<'w, T>(
        budget: &mut Budget<'w>,
        run: impl FnOnce(&mut Budget<'w>) -> R<T>,
    ) -> R<T> {
        scoped(budget, run)
    }
}
