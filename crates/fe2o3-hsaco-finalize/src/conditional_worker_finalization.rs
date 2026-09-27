//! Actual conditional source custody through the shared V5 artifact finalizer.
use crate::{
    ContentIdentityV1, FinalizedNominalHsacoV5,
    InertConditionalFirstBuildWorkerEvidenceV2 as Evidence,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5 as SCRATCH, NativeWorkerFinalizationErrorV1 as Error,
    NominalFinalizationErrorV5, finalize_unfinalized_nominal_hsaco_v5,
    inspect_unfinalized_nominal_hsaco_v5,
    native_worker_finalization::{failure, finalization_identity},
    nominal_worker_common::common_launch,
    request_construction::decode_link_options,
    worker_v3_hsaco_admission::{
        SharedWorkerV3HsacoInspectionV1 as Inspection, WorkerV3LaunchContractV1 as Launch,
        inspect_worker_v3_hsaco_preimage_v1, strict_kernel_launch_contract,
    },
};
use fe2o3_kernel_descriptor::{
    ConditionalInvocationWireErrorV1, DescriptorWireErrorV3, DescriptorWireErrorV5,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;
const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-WORKER-CANONICAL-FINALIZATION/V5\0";
const FRAME: usize = 2 * size_of::<PreparedFinalizedConditionalWorkerHsacoV5>() + 4096;

/// Actual consumed or recovered V5 source/worker owner plus exact V5 artifact bytes. This is
/// structural evidence, not protected origin, machine refinement or authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedConditionalWorkerHsacoV5 as C,
///     PreparedFinalizedNativeWorkerHsacoV4 as N};
/// fn downgrade(c: C) -> N { c.into() }
/// ```
pub struct PreparedFinalizedConditionalWorkerHsacoV5 {
    source: Evidence,
    finalized: FinalizedNominalHsacoV5,
    inspection: Inspection,
    identity: [u8; 32],
    retained_storage: usize,
}
impl PreparedFinalizedConditionalWorkerHsacoV5 {
    pub const fn source(&self) -> &Evidence {
        &self.source
    }
    pub const fn finalized(&self) -> &FinalizedNominalHsacoV5 {
        &self.finalized
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub fn policy(&self) -> &crate::WorkerV3HsacoPolicyV1 {
        &self.inspection.policy
    }
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Additional unreserved owner headers. Artifact parsing and payloads keep the
/// existing finalizer's separately bounded domain, not aggregate Rust/RSS credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerFinalizationStorageV5(usize);
impl ConditionalWorkerFinalizationStorageV5 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Finalizes only the exact descriptor carried by the retained conditional
/// source. The caller prepays source custody and reserves the returned header
/// charge before retaining the result. Worker-wire decoding and artifact
/// inspection retain their existing separately bounded work/storage domain;
/// descriptor traversal and final identity hashing use this original account.
/// This is not an aggregate artifact work, Rust allocation, or RSS bound.
pub fn finalize_conditional_worker_hsaco_v5(
    source: Evidence,
    b: &mut Budget<'_>,
) -> Result<
    (
        PreparedFinalizedConditionalWorkerHsacoV5,
        ConditionalWorkerFinalizationStorageV5,
    ),
    Error,
> {
    let floor = source.required_retained_storage();
    b.with_prepaid_scope(floor, 8, 4096, FRAME, |b| {
        source.revalidate_for_artifact(b)?;
        source
            .artifact_lineage()
            .validate()
            .map_err(|e| failure("Worker lineage", e))?;
        let handoff = source.recovered_handoff().handoff();
        let module = handoff.module_handoff();
        let abi = handoff.capsule().descriptor_bytes();
        let raw = source.output_bytes();
        let launch = derive_launch(raw, b)?;
        let (code_object, _) =
            decode_link_options(source.plan().options()).map_err(|e| failure("link options", e))?;
        let inspection = inspect_worker_v3_hsaco_preimage_v1(
            source.plan().target(),
            code_object,
            module.symbol_manifest().clone(),
            module.envelope().identity(),
            source.output_identity(),
            raw,
            launch,
        )
        .map_err(|e| failure("raw inspection", e))?;
        let finalized = finalize_artifact(raw, abi, b)?;
        b.charge_work(
            finalized
                .as_bytes()
                .len()
                .checked_add(finalized.descriptor_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let identity = finalization_identity(
            DOMAIN,
            source.identity(),
            source.binding().identity(),
            &inspection,
            ContentIdentityV1::calculate(finalized.as_bytes()),
            ContentIdentityV1::calculate(finalized.descriptor_bytes()),
            finalized.digest(),
        );
        let storage = ConditionalWorkerFinalizationStorageV5(size_of::<
            PreparedFinalizedConditionalWorkerHsacoV5,
        >());
        Ok((
            PreparedFinalizedConditionalWorkerHsacoV5 {
                source,
                finalized,
                inspection,
                identity,
                retained_storage: floor.checked_add(storage.0).ok_or(Resource::Arithmetic)?,
            },
            storage,
        ))
    })
}

fn derive_launch(raw: &[u8], b: &mut Budget<'_>) -> Result<Launch, Error> {
    b.with_prepaid_scope(b.storage(), 0, 0, SCRATCH, |b| {
        let inspected =
            inspect_unfinalized_nominal_hsaco_v5(raw, SCRATCH, &mut |w| b.charge_work(w))
                .map_err(|e| descriptor_error("V5 descriptor", e))?;
        let table = inspected.descriptor_table();
        common_launch(table.kernel_count(), |index| {
            let kernel = table
                .kernel(index, &mut |w| b.charge_work(w))
                .map_err(|e| descriptor_error("V5 kernel", NominalFinalizationErrorV5::Wire(e)))?;
            strict_kernel_launch_contract(kernel.launch()).map_err(|e| failure("launch", e))
        })
    })
}
fn finalize_artifact(
    raw: &[u8],
    abi: &[u8],
    b: &mut Budget<'_>,
) -> Result<FinalizedNominalHsacoV5, Error> {
    b.with_prepaid_scope(b.storage(), 0, 0, SCRATCH, |b| {
        finalize_unfinalized_nominal_hsaco_v5(raw, abi, SCRATCH, &mut |w| b.charge_work(w))
            .map_err(|e| descriptor_error("V5 finalization", e))
    })
}
pub(crate) fn reconstruct_artifact(finalized: &[u8], b: &mut Budget<'_>) -> Result<Vec<u8>, Error> {
    b.with_prepaid_scope(b.storage(), 0, 0, SCRATCH, |b| {
        crate::derive_unfinalized_nominal_hsaco_v5(finalized, SCRATCH, &mut |w| b.charge_work(w))
            .map_err(|e| descriptor_error("V5 raw reconstruction", e))
    })
}
fn descriptor_error(phase: &'static str, e: NominalFinalizationErrorV5<Resource>) -> Error {
    match e {
        NominalFinalizationErrorV5::Work(e)
        | NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Nominal(
            DescriptorWireErrorV3::Work(e),
        ))
        | NominalFinalizationErrorV5::Wire(DescriptorWireErrorV5::Contract(
            ConditionalInvocationWireErrorV1::Work(e),
        )) => Error::Resource(e),
        other => failure(phase, other),
    }
}

#[cfg(test)]
#[path = "conditional_worker_finalization_tests.rs"]
mod tests;
