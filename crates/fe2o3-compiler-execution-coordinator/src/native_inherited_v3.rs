use crate::{
    CompilerExecutionSupervisorTrustV3 as Trust, PreparedCompilerExecutionSupervisorV3 as Prepared,
    RootManagedCompilerExecutionServiceV3 as Managed,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV3 as AnchorCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV3 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as AnchorKey,
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV3 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV3 as SupervisorCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3 as ANCHOR_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as ANCHOR_SCRATCH,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as ANCHOR_WORK,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3 as PROVISIONING_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3 as PROVISIONING_SCRATCH,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3 as PROVISIONING_WORK,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as POLICY_BYTES,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_SCRATCH,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3 as SUPERVISOR_BYTES,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as SUPERVISOR_SCRATCH,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
    CompilerExecutionExternalAnchorDeploymentV3 as AnchorDeployment,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3 as MAX_ANCHOR,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3 as MAX_HELPER,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3 as MAX_SUPERVISOR,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as MAX_LAUNCHER,
};
use fe2o3_external_anchor_coordinator::{
    PreparedExternalAnchorOccurrenceV3 as Anchor, RootManagedExternalAnchorV3 as ManagedAnchor,
};
crate::native_inherited_adapter::inherited!(InheritedCompilerExecutionDeploymentV3, "3", "2");

impl InheritedCompilerExecutionDeploymentV3 {
    /// Closed executable schedule. Preserve the legacy startup/cleanup allowance
    /// and explicitly add every original-root operation; no presumed offset from
    /// the now-unused indirect launch or Managed continuity is spent twice.
    pub(crate) fn original_root_startup_quota(
        turns: usize,
        cleanup_turns: usize,
    ) -> Result<root::CompilerExecutionStartupQuotaV2> {
        use crate::native_v3::root_intake::{Receiver, RootCompilerRequest};
        let mut quota = Self::startup_quota(turns, cleanup_turns)?;
        let continuity = Prepared::maximum_revalidation_quota()?;
        let request = RootCompilerRequest::preparation_quota()?;
        let launch = RootCompilerRequest::launch_quota()?;
        let (cleanup_work, cleanup_storage) = RootCompilerRequest::cleanup_growth()?;
        let refusal = RootCompilerRequest::refusal_quota()?;
        quota.request_work = root::sum(&[
            quota.request_work,
            root::LOCAL_WORK,
            Inputs::WORK,
            request.work(),
            launch.work(),
            root::repeated(
                turns,
                root::sum(&[Receiver::TURN_WORK, continuity.work(), refusal.work()])?,
            )?,
        ])?;
        quota.request_storage = root::sum(&[
            quota.request_storage,
            // The complete original admission reservation remains even when
            // Prepared is smaller; growth and transcript overlap that floor.
            Self::maximum_retained_storage()?,
            Prepared::maximum_retained_storage()?,
            Receiver::STORAGE,
            Receiver::SCRATCH,
            RootCompilerRequest::ENVELOPE,
            request.scratch(),
            launch.scratch(),
            refusal.scratch(),
            continuity.scratch(),
        ])?;
        quota.cleanup_work = root::sum(&[quota.cleanup_work, cleanup_work])?;
        quota.cleanup_storage = root::sum(&[quota.cleanup_storage, cleanup_storage])?;
        Ok(quota)
    }

    /// Keeps genuine preparation at the original root without the indirect
    /// supervisor launch. The sole listener remains bound. The caller keeps the
    /// full consumed reservation and adds returned monotone growth, including
    /// any conservative excess above Prepared's own charge, until final Drop.
    /// Store the returned owner in outer cancellation custody before reserving
    /// growth. On refusal the original pool still owns any nonterminal anchor
    /// child/domain and its canonical guard, not a recreated cleanup controller.
    pub(crate) fn prepare_original_root(
        self,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'_>,
    ) -> Result<(Prepared, Storage)> {
        let input = self.retained;
        b.with_prepaid_scope(input, 8, root::LOCAL_WORK, Self::FRAME, |b| {
            let prepared = self.prepare_inner(timeout, cleanup, b)?;
            let growth = prepared.retained_storage().saturating_sub(input);
            Ok((prepared, Storage(growth)))
        })
    }
}
