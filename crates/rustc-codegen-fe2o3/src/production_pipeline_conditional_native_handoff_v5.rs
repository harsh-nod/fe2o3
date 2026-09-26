//! Consuming protected continuation; serialization grants no finalizer authority.
use super::super::ConditionalPrefixForFV1 as Prefix;
use super::{Budget, Resource};
use crate::production_pipeline::{ProductionCompilerCustody, ProductionPipelineError};
use fe2o3_compiler_ffi::{
    CompilerModuleHandoffV2 as Module,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as DECODE_STORAGE,
    InertSemanticCompilerModuleHandoffErrorV5 as HandoffError,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5,
};
use fe2o3_compiler_lineage::*;
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;
use std::{fmt, mem::size_of};

#[path = "production_pipeline_conditional_native_module_v5.rs"]
mod module;
#[path = "production_pipeline_conditional_native_pack_v5.rs"]
mod pack;

/// Error chains deliberately stop here: opaque failures retain their charges.
#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Output(NativeConditionalOutputErrorV1<Resource>),
    Carrier(NativeConditionalCarrierErrorV1<Resource>),
    Metadata(NativeConditionalMetadataErrorV1<Resource>),
    Capsule(InertProductionSemanticCapsuleErrorV5<Resource>),
    Handoff(HandoffError),
    Seal(HandoffError<Resource>),
    Invocation(fe2o3_rustc_invocation::ValidationError),
    Lineage(ProductionTargetLineageErrorV3),
    Subject(NativeNeutralSubjectErrorV1),
    Commitment(fe2o3_compiler_ffi::FinalCompilerModuleCommitmentErrorV3),
    Worker(crate::production_worker_handoff::ProductionWorkerHandoffError),
    Manifest(fe2o3_compiler_ffi::CompilerModuleSymbolManifestErrorV1),
    Module(fe2o3_compiler_ffi::CompilerModuleHandoffErrorV2),
    Roles(crate::compiler_module_contract::CompilerModuleRoleError),
    Mismatch(&'static str),
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
type R<T> = Result<T, Error>;

pub(in crate::production_pipeline) struct Prepared {
    prefix: Prefix,
    handoff: Handoff,
    retained_floor: usize,
}
impl Prepared {
    pub(in crate::production_pipeline) fn into_finalizer_error(
        self,
        budget: &mut Budget<'_>,
    ) -> ProductionPipelineError {
        if budget.storage() < self.retained_floor {
            return super::super::resource(Resource::Accounting);
        }
        let Self {
            prefix, handoff, ..
        } = self;
        let error = prefix.into_finalizer_error_v1(budget);
        drop(handoff);
        error
    }
}

fn invocation(custody: &ProductionCompilerCustody) -> R<&RustcInvocationDescriptorV3> {
    match custody {
        ProductionCompilerCustody::ProtectedV3 { invocation, .. } => Ok(invocation.descriptor()),
        ProductionCompilerCustody::ExtractionOnly => Err(Error::Mismatch(
            "conditional native handoff requires original protected compiler custody",
        )),
    }
}

pub(in crate::production_pipeline) fn prepare(
    mut prefix: Prefix,
    budget: &mut Budget<'_>,
) -> R<Prepared> {
    if budget.storage() < prefix.retained_floor {
        return Err(Resource::Accounting.into());
    }
    invocation(&prefix.preparation.bindings.transaction.compiler_custody)?;
    prefix.chain.check_owned(budget)?;
    budget.reserve_storage(size_of::<Prepared>() - size_of::<Prefix>() - size_of::<Handoff>())?;
    let floor = budget.storage();
    let module = module::prepare(&mut prefix, budget)?;
    let backing = pack::prepare(&prefix, &module, budget)?;
    drop(module);
    // Both original and newly serialized owners were paid while coexisting.
    // Only the returned backing survives this construction scratch scope.
    let scratch = budget
        .storage()
        .checked_sub(floor)
        .and_then(|n| n.checked_sub(backing.capacity()))
        .ok_or(Resource::Accounting)?;
    budget.release_storage(scratch)?;
    budget.reserve_storage(DECODE_STORAGE)?;
    budget.charge_work(
        inert_semantic_compiler_module_handoff_decode_work_v5(backing.len())
            .map_err(Error::Handoff)?,
    )?;
    let handoff = Handoff::decode_owned(backing).map_err(Error::Handoff)?;
    Ok(Prepared {
        prefix,
        handoff,
        retained_floor: budget.storage(),
    })
}

fn coordinate(digest: [u8; 32], length: u64) -> R<TargetLineageIdentityV3> {
    TargetLineageIdentityV3::new(digest, length).map_err(Error::Lineage)
}

// Fixed bounded codecs are prepaid with a logical allocation/copy allowance,
// not a claim about exact allocator overhead or instruction counts.
fn codec<T>(bytes: usize, budget: &mut Budget<'_>) -> R<()> {
    budget.reserve_storage(
        bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(
        bytes
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(())
}

fn grow(bytes: &mut Vec<u8>, len: usize, budget: &mut Budget<'_>) -> R<()> {
    let old_len = bytes.len();
    let additional = len.checked_sub(old_len).ok_or(Resource::Arithmetic)?;
    if budget.storage() < bytes.capacity() {
        return Err(Resource::Accounting.into());
    }
    let requested = bytes.capacity().max(len);
    budget.reserve_storage(requested - bytes.capacity())?;
    budget.charge_work(1)?;
    bytes
        .try_reserve_exact(additional)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        bytes
            .capacity()
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    budget.charge_work(additional)?;
    bytes.resize(len, 0);
    Ok(())
}

#[cfg(test)]
#[path = "production_pipeline_conditional_native_handoff_v5_tests.rs"]
mod tests;

fn copy(destination: &mut [u8], source: &[u8], budget: &mut Budget<'_>) -> R<()> {
    if destination.len() != source.len() {
        return Err(Error::Mismatch("conditional native field extent"));
    }
    budget.charge_work(source.len())?;
    destination.copy_from_slice(source);
    Ok(())
}
