//! One account borrow from native recipe admission through child readiness.
//! The outer broker still owns profile/configuration provenance and selection.
use super::{
    Budget, ParentCompilerExecutionReadinessCustodyV3 as Readiness, Policy, Profile, Resource,
    Result,
    pipeline::{ConditionalRecoveryPolicy, ContinuationError, ParentPreparedConditionalArtifact},
    validate_configuration,
};
use crate::{
    build_config::native::PreparedNativeProductionBuildConfig as Recipe,
    protected_compiler_handoff_v3::ParentRustcInvocationCustody as Invocation,
};
use fe2o3_artifact_transaction::{BuildAttempt, ProducerIdentity};
use fe2o3_compiler_execution_client::PendingCompilerExecutionChildChannelV1 as Pending;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as PolicyRecord;
use std::{mem::size_of, path::Path, process::Command, time::Instant};

// Fixed Rust owner/channel/hook state. Command arguments, environment, pinned
// executable custody, spawn/OS retries and process supervision remain separately
// prepaid caller domains; this is neither allocator-capacity nor RSS accounting.
const FRAME: usize = 64 * 1024;
const LOCAL_WORK: usize = 128 * 1024;

pub(crate) struct PreparedCompilerExecutionBoundaryV3<'b, 'w> {
    profile: Profile,
    policy: Policy,
    channel: Pending,
    recipe: Recipe,
    budget: &'b mut Budget<'w>,
}

/// No detached recipe or budget escape: the readiness and recipe move together.
pub(crate) struct ReadyCompilerExecutionAttemptV3<'b, 'w> {
    readiness: Readiness<'b, 'w>,
    recipe: Recipe,
}

impl<'b, 'w> PreparedCompilerExecutionBoundaryV3<'b, 'w> {
    /// Consume the prepaid, independently admitted profile and native recipe.
    /// Retain this original budget borrow until failure or the final artifact.
    /// Command is one-use after preparation; drop it on refusal or after spawn
    /// before retiring its policy-alias/hook charge. Failure/unwind is terminal:
    /// no enclosing refundable scope may retire retained input reservations.
    pub(crate) fn prepare(
        profile: Profile,
        recipe: Recipe,
        command: &mut Command,
        budget: &'b mut Budget<'w>,
    ) -> Result<Self> {
        recipe.check_account(budget)?;
        budget.charge_work(LOCAL_WORK)?;
        let floor = recipe
            .retained_storage()
            .checked_add(profile.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(FRAME + Policy::FILE_STORAGE)?;
        profile.revalidate(budget)?;
        Pending::preflight_with_issuer_policy()?;
        let (record, storage) =
            PolicyRecord::decode(profile.profile().policy().canonical_bytes(), budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (policy, storage) = Policy::create(record, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        validate_configuration(&profile, &policy, budget)?;
        policy.inherit_for_child(command, budget)?;
        let channel = Pending::prepare(command)?;
        Ok(Self {
            profile,
            policy,
            channel,
            recipe,
            budget,
        })
    }

    /// Call after the single selected child is spawned. The existing child
    /// endpoint and supervisor exchange both use this absolute deadline; this
    /// method neither constructs synthetic readiness nor accepts a supplied PID
    /// as proof of a channel's peer identity.
    pub(crate) fn finish(
        self,
        child_pid: u32,
        deadline: Instant,
    ) -> Result<ReadyCompilerExecutionAttemptV3<'b, 'w>> {
        let Self {
            profile,
            policy,
            channel,
            recipe,
            budget,
        } = self;
        recipe.check_account(budget)?;
        budget.charge_work(LOCAL_WORK)?;
        validate_configuration(&profile, &policy, budget)?;
        let launch = channel.finish_until(child_pid, deadline)?;
        let readiness = Readiness::finish(profile, policy, launch, child_pid, deadline, budget)?;
        Ok(ReadyCompilerExecutionAttemptV3 { readiness, recipe })
    }
}

impl<'b, 'w> ReadyCompilerExecutionAttemptV3<'b, 'w> {
    /// The enclosing supervisor must establish successful compiler completion
    /// before calling. Output/path, exact parent invocation and independently
    /// admitted recovery-policy inputs remain separately prepaid.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(crate) fn finalize_after_compiler_success<'a>(
        self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        policy: ConditionalRecoveryPolicy<'_>,
    ) -> std::result::Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>, ContinuationError> {
        self.recipe.finalize_conditional_current(
            self.readiness,
            output_dir,
            producer,
            attempt,
            invocation,
            policy,
        )
    }
}

const _: () = assert!(FRAME >= 2 * size_of::<PreparedCompilerExecutionBoundaryV3<'static, 'static>>()
    + 2 * size_of::<ReadyCompilerExecutionAttemptV3<'static, 'static>>()
    + Readiness::OWNER_STORAGE
    + fe2o3_compiler_execution_client::CompilerExecutionSupervisorReadinessV3::CHILD_LAUNCH_STORAGE);
