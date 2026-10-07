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

#[path = "native_application_sources.rs"]
mod application_sources;

impl InheritedCompilerExecutionDeploymentV3 {
    /// Opens the closed native application source roster from the actual installed
    /// root tree, then performs the same image, record, seed, lifecycle and anchor
    /// preparation admission as the inherited route. Configuration matching alone
    /// cannot create the returned signing or anchor owners.
    ///
    /// The caller reserves the FULL returned charge on this original account.
    /// This binds only the separate application listener; it does not launch a
    /// supervisor, issuer or application. The compiler listener is unchanged.
    /// Root-controlled installation paths must remain administratively stable;
    /// these finite observations do not defend against a concurrent malicious root.
    pub(crate) fn admit_native_application<'work>(
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        b: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        Self::admit_fixed_native_sources(installation, root::ListenerRole::NativeApplication, b)
    }

    pub(crate) fn admit_fixed_native_compiler<'work>(
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        b: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        Self::admit_fixed_native_sources(installation, root::ListenerRole::Compiler, b)
    }

    fn admit_fixed_native_sources<'work>(
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        role: root::ListenerRole,
        b: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        let floor = installation.retained_storage()?;
        b.with_prepaid_scope(
            floor,
            8,
            application_sources::WORK,
            application_sources::SCRATCH,
            |b| {
                installation.revalidate(b)?;
                let (value, charge) = Self::admit_sources(
                    role,
                    b,
                    |b| {
                        let files = application_sources::open(&Self::SOURCE_LIMITS, b)?;
                        if source::read_record::<SUPERVISOR_BYTES>(&files[8], b)?.as_slice()
                            != installation.supervisor().canonical_bytes()
                            || source::read_record::<POLICY_BYTES>(&files[9], b)?.as_slice()
                                != installation.policy().canonical_bytes()
                            || source::read_record::<ANCHOR_BYTES>(&files[10], b)?.as_slice()
                                != installation.anchor().canonical_bytes()
                        {
                            return Err(root::invalid(
                                "native application",
                                "fixed source records differ from root installation",
                            ));
                        }
                        Ok(files)
                    },
                    |value, b| {
                        installation.revalidate(b)?;
                        if value.trust.policy().policy().canonical_bytes()
                            != installation.policy().canonical_bytes()
                            || value.trust.deployment().deployment().canonical_bytes()
                                != installation.supervisor().canonical_bytes()
                        {
                            return Err(root::invalid(
                                "native application",
                                "actual source admission differs from root installation",
                            ));
                        }
                        Ok(())
                    },
                )?;
                b.reserve_storage(charge.additional_storage())?;
                Ok((value, charge))
            },
        )
    }
}

impl InheritedCompilerExecutionDeploymentV3 {
    pub(crate) fn fixed_compiler_startup_quota(
        turns: usize,
        cleanup_turns: usize,
    ) -> Result<root::CompilerExecutionStartupQuotaV2> {
        use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Installation;
        let mut quota = Self::original_root_startup_quota(turns, cleanup_turns)?;
        quota.request_work = root::sum(&[
            quota.request_work,
            2 * application_sources::WORK,
            2 * Installation::IO_WORK,
            root::LOCAL_WORK,
        ])?;
        quota.request_storage = root::sum(&[
            quota.request_storage,
            2 * application_sources::SCRATCH,
            2 * Installation::IO_STORAGE,
        ])?;
        // Unlike permanent shutdown, the first nonterminal checkpoint is not
        // prepaid at pool admission; retries retain the existing scan allowance.
        quota.cleanup_work = root::sum(&[quota.cleanup_work, Cleanup::quiescent_phase_work()])?;
        Ok(quota)
    }

    /// Consumes fixed application-source admission through the actual root anchor
    /// launch and native supervisor preparation. Keeps original signing, lifecycle
    /// and anchor custody; neither installed records nor a readiness record replace
    /// these owners. The original compiler listener cannot enter this route.
    ///
    /// Keep the consumed input reservation and reserve the returned growth. The
    /// same original cleanup account must remain alive through all nonterminal
    /// children, including anchor startup failures. This does not launch a supervisor.
    pub(crate) fn prepare_native_application_root<'work>(
        self,
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(Prepared, Storage)> {
        self.prepare_native_application_with_guard(installation, timeout, cleanup, true, b)
    }

    pub(crate) fn prepare_native_application_after_compiler<'work>(
        self,
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        b: &mut Budget<'work>,
    ) -> Result<(Prepared, Storage)> {
        self.prepare_native_application_with_guard(installation, timeout, cleanup, false, b)
    }

    fn prepare_native_application_with_guard<'work>(
        self,
        installation: &fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3<'work>,
        timeout: Duration,
        cleanup: &mut Cleanup,
        install_guard: bool,
        b: &mut Budget<'work>,
    ) -> Result<(Prepared, Storage)> {
        let floor = root::sum(&[self.retained, installation.retained_storage()?])?;
        b.with_prepaid_scope(floor, 8, application_sources::WORK, Self::FRAME, |b| {
            installation.revalidate(b)?;
            self.inputs.validate_native_application(b)?;
            if self.trust.policy().policy().canonical_bytes()
                != installation.policy().canonical_bytes()
                || self.trust.deployment().deployment().canonical_bytes()
                    != installation.supervisor().canonical_bytes()
            {
                return Err(root::invalid(
                    "native application",
                    "preparation installation mismatch",
                ));
            }
            self.prepare_original_root_with_guard(timeout, cleanup, install_guard, b)
        })
    }

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
        let cancellation = RootCompilerRequest::cancellation_quota()?;
        let runtime = RootCompilerRequest::runtime_turn_quota()?;
        let issuer = RootCompilerRequest::runtime_startup_quota()?;
        let completion = RootCompilerRequest::completion_quota()?;
        let (runtime_cleanup_work, runtime_cleanup_storage) =
            RootCompilerRequest::runtime_cleanup_growth(turns, cleanup_turns)?;
        quota.request_work = root::sum(&[
            quota.request_work,
            root::LOCAL_WORK,
            Inputs::WORK,
            request.work(),
            launch.work(),
            issuer.work(),
            // Original completion is sent only after aggregate cleanup, on the
            // same request account and deadline. No cleanup credit is borrowed.
            completion.work(),
            // One immediate foreground retirement attempt, then at most one
            // per original cleanup turn. No new account or renewed deadline.
            root::repeated(root::sum(&[cleanup_turns, 1])?, cancellation.work())?,
            root::repeated(
                turns,
                root::sum(&[
                    Receiver::TURN_WORK,
                    continuity.work(),
                    refusal.work(),
                    runtime.work(),
                ])?,
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
            cancellation.scratch(),
            runtime.scratch(),
            issuer.scratch(),
            completion.scratch(),
        ])?;
        quota.cleanup_work = root::sum(&[quota.cleanup_work, cleanup_work, runtime_cleanup_work])?;
        quota.cleanup_storage = root::sum(&[
            quota.cleanup_storage,
            cleanup_storage,
            runtime_cleanup_storage,
        ])?;
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
        self.prepare_original_root_with_guard(timeout, cleanup, true, b)
    }

    fn prepare_original_root_with_guard(
        self,
        timeout: Duration,
        cleanup: &mut Cleanup,
        install_guard: bool,
        b: &mut Budget<'_>,
    ) -> Result<(Prepared, Storage)> {
        let input = self.retained;
        b.with_prepaid_scope(input, 8, root::LOCAL_WORK, Self::FRAME, |b| {
            let prepared = self.prepare_inner_with_guard(timeout, cleanup, install_guard, b)?;
            let growth = prepared.retained_storage().saturating_sub(input);
            Ok((prepared, Storage(growth)))
        })
    }
}
