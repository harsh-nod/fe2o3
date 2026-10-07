//! Original controller image/profile, bounded Verus lease and pinned analyzer.
use super::*;
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1 as Limits,
    AuthenticatedPhysicalMachineEffectWorkerV1 as Analyzer,
};
use fe2o3_protected_service_profile::{ProofControllerProcessProfileV1 as Process, observations};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Image,
};
use fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1 as Runtime;
use std::time::Duration;

const PROFILE_WORK: usize = 2
    * (observations::PROCESS_CURRENT_WORK
        + observations::NAMESPACE_CAPTURE_WORK
        + observations::NAMESPACE_SELF_WORK);
const PROFILE_SCRATCH: usize = 128 * 1024
    + observations::PROCESS_CURRENT_SCRATCH
    + observations::NAMESPACE_CAPTURE_SCRATCH
    + observations::NAMESPACE_SELF_SCRATCH;

pub(crate) struct Tools {
    pub(crate) analyzer: Analyzer,
    pub(crate) runtime: Runtime,
    profile: Process,
    image: Image,
    runtime_charge: usize,
    ledger: Ledger,
    account: Option<Account>,
}
impl Tools {
    /// Analyzer opening/child capture keeps its existing independent subprocess
    /// resource policy. Its opaque internal heap is not claimed as canonical
    /// account/RSS accounting. Image, profile and retained Verus backing are paid
    /// on the original controller ledger; neither proof policy is broadened.
    pub(crate) fn open(config: &Config, budget: &mut Budget<'_>) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(PROFILE_WORK).map_err(other)?;
        budget.reserve_storage(PROFILE_SCRATCH).map_err(other)?;
        let profile = Process::capture(config.credentials()?).map_err(other)?;
        budget
            .reserve_storage(size_of::<Process>())
            .map_err(other)?;
        budget.release_storage(PROFILE_SCRATCH).map_err(other)?;
        let (image, charge) = Image::admit_running(
            config.controller_measurement()?,
            Owner::new(0, 0).map_err(other)?,
            "native application proof controller",
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (runtime, charge) = Runtime::open_closed_conditional_fill_in_original_account_v1(
            crate::deployment::RUNTIME_PATH,
            budget,
        )
        .map_err(other)?;
        let runtime_charge = charge.retained_storage();
        budget.reserve_storage(runtime_charge).map_err(other)?;
        require(
            runtime.identity().as_bytes() == config.verus_identity(),
            "native Verus runtime identity differs",
        )?;
        let analyzer = Analyzer::open(
            crate::deployment::WORKER_PATH,
            config.analyzer_policy()?,
            Self::analysis_limits()?,
        )
        .map_err(other)?;
        let value = Self {
            analyzer,
            runtime,
            profile,
            image,
            runtime_charge,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
        };
        // Header and the analyzer's fixed descriptor path are retained separately
        // from the subordinate analyzer's bounded transient capture domain.
        budget
            .reserve_storage(size_of::<Self>() + 4096)
            .map_err(other)?;
        value.revalidate(budget)?;
        let storage = value.retained_storage();
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, storage))
    }
    pub(crate) fn analysis_limits() -> io::Result<Limits> {
        Limits::new(Duration::from_secs(60), 1024 * 1024, 16 * 1024).map_err(other)
    }
    pub(crate) fn retained_storage(&self) -> usize {
        size_of::<Self>()
            + 4096
            + self.image.retained_storage()
            + self.runtime_charge
            + size_of::<usize>()
    }
    pub(crate) fn revalidate(&self, budget: &mut Budget<'_>) -> io::Result<()> {
        require(
            self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1()
                && budget.storage() >= self.retained_storage(),
            "native proof tools account differs",
        )?;
        budget.charge_work(PROFILE_WORK).map_err(other)?;
        budget.reserve_storage(PROFILE_SCRATCH).map_err(other)?;
        self.profile.revalidate_current().map_err(other)?;
        budget.release_storage(PROFILE_SCRATCH).map_err(other)?;
        self.image.revalidate(budget).map_err(other)?;
        self.runtime
            .revalidate_closed_conditional_fill_in_original_account_v1(budget)
            .map_err(other)
    }
}
