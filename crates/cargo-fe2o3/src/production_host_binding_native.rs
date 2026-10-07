//! Native-only committed source namespace projection, never runtime authority.
use super::*;
use crate::application_handoff::native::Pinned;
use fe2o3_compiler_closure_capability::{
    CompilerExecutionClientProfileCapabilityV3 as Capability,
    ProductionCompilerExecutionDeploymentV3 as Deployment,
};
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    InertSemanticCompilerModuleHandoffV5 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v5 as decode_work,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime_protocol::InertConditionalWorkerReadinessWireV5 as Wire;

pub(crate) struct CommittedNativeHostBindingProjection<'source, 'work> {
    pinned: Pinned<'source, 'work>,
    deployment: Deployment<'work>,
    closure: &'source AuthorizedKernelClosureV1,
    outer: Handoff,
    source: RetainedSource,
    sealed: SealedProjection,
}

impl<'source, 'work> CommittedNativeHostBindingProjection<'source, 'work> {
    pub(crate) fn prepare(
        context: &'source crate::BackendRunContext,
        admission: &crate::authority_release::ProtectedReleaseAdmission,
        capability: &Capability,
        budget: &mut Budget<'work>,
    ) -> Result<Self, String> {
        let closure = context
            .authorized_closure
            .as_ref()
            .ok_or("native host binding requires the original authorized source closure")?;
        closure.revalidate()?;
        let (deployment, charge) = Deployment::open(budget).map_err(|e| e.to_string())?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(|e| e.to_string())?;
        if capability.profile().canonical_bytes() != deployment.profile().canonical_bytes() {
            return Err(
                "native host binding installed profile differs from protected release".into(),
            );
        }
        let pinned = Pinned::discover(context.generation.artifact_dir(), budget)?;
        let wire = Wire::decode(pinned.exact_readiness(), pinned.exact_readiness().len())
            .map_err(|e| e.to_string())?;
        let outer_bytes = wire.outer_handoff_bytes();
        budget
            .charge_work(
                decode_work(outer_bytes.len())
                    .map_err(|e| format!("native V5 decode bound: {e:?}"))?,
            )
            .map_err(|e| e.to_string())?;
        budget
            .reserve_storage(
                outer_bytes
                    .len()
                    .checked_add(METADATA)
                    .ok_or("native host decode storage overflow")?,
            )
            .map_err(|e| e.to_string())?;
        let outer = Handoff::decode_owned(outer_bytes.to_vec())
            .map_err(|e| format!("native V5 handoff: {e:?}"))?;
        validate_native_source_binding(&pinned, &outer, &deployment)?;
        // Retained strings, parsed invocation fields and the sealed projection
        // are paid alongside the exact original V5 backing, without a new account.
        budget
            .reserve_storage(
                outer
                    .backing_capacity()
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(128 * 1024))
                    .ok_or("native host projection storage overflow")?,
            )
            .map_err(|e| e.to_string())?;
        budget
            .charge_work(
                outer
                    .backing_capacity()
                    .checked_mul(16)
                    .ok_or("native host projection work overflow")?,
            )
            .map_err(|e| e.to_string())?;
        let (source, sealed) =
            prepare_source_projection(context, admission, closure, outer.capsule().invocation())?;
        let value = Self {
            pinned,
            deployment,
            closure,
            outer,
            source,
            sealed,
        };
        value.revalidate(budget)?;
        Ok(value)
    }

    pub(crate) fn revalidate(&self, budget: &mut Budget<'work>) -> Result<(), String> {
        self.deployment
            .revalidate(budget)
            .map_err(|e| e.to_string())?;
        self.pinned.revalidate(budget)?;
        validate_native_source_binding(&self.pinned, &self.outer, &self.deployment)?;
        self.closure.revalidate()?;
        self.source.revalidate()
    }
}

fn validate_native_source_binding(
    pinned: &Pinned<'_, '_>,
    outer: &Handoff,
    deployment: &Deployment<'_>,
) -> Result<(), String> {
    let subject = pinned.carriage().request().subject();
    if pinned.carriage().policy().canonical_bytes() != deployment.policy().canonical_bytes()
        || subject.outer_handoff().sha256() != outer.identity().sha256()
        || subject.outer_handoff().byte_len() != outer.identity().byte_len()
        || subject.rustc_invocation_sha256() != outer.capsule().invocation_digest().as_bytes()
        || subject.compiler_closure() != *outer.capsule().invocation().compiler_closure()
    {
        return Err("native committed source and authenticated V3 carriage differ".into());
    }
    Ok(())
}

impl HostBindingProjection for CommittedNativeHostBindingProjection<'_, '_> {
    fn configure_child(
        &self,
        command: &mut Command,
        wrapper: &crate::pinned_executable::PinnedExecutable,
    ) -> Result<(), String> {
        configure_host_wrapper(command, wrapper, &self.sealed)
    }
}
