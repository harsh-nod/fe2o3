//! One account borrow from native recipe admission through child readiness.
//! Root-policy custody precedes channel setup and survives artifact persistence.
//! Runtime enforcement and production broker selection remain separate gates.
use super::{
    Budget, ParentCompilerExecutionReadinessCustodyV3 as Readiness, Policy, Profile, Resource,
    Result,
    pipeline::{ConditionalRecoveryPolicy, ContinuationError, ParentDurableConditionalArtifact},
    validate_configuration,
};
use crate::{
    build_config::native::PreparedNativeProductionBuildConfig as Recipe,
    protected_compiler_handoff_v3::ParentRustcInvocationCustody as Invocation,
};
use fe2o3_artifact_transaction::{BuildAttempt, ProducerIdentity};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::ApprovedCompilerPolicyV1 as Approval;
use fe2o3_compiler_execution_client::PendingCompilerExecutionChildChannelV1 as Pending;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as PolicyRecord;
use std::{mem::size_of, path::Path, process::Command, time::Instant};

// Fixed Rust owner/channel/hook state. Command arguments, environment, pinned
// executable custody, spawn/OS retries and process supervision remain separately
// prepaid caller domains; this is neither allocator-capacity nor RSS accounting.
const FRAME: usize = 64 * 1024;
const LOCAL_WORK: usize = 128 * 1024;

pub(super) struct PreparedCompilerExecutionTransportV3<'b, 'w> {
    profile: Profile,
    policy: Policy,
    channel: Pending,
    recipe: Recipe,
    budget: &'b mut Budget<'w>,
}

pub(super) struct ReadyCompilerExecutionTransportV3<'b, 'w> {
    readiness: Readiness<'b, 'w>,
    recipe: Recipe,
}

impl<'b, 'w> PreparedCompilerExecutionTransportV3<'b, 'w> {
    /// Transport/configuration only. This alone has no finalization transition.
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
    ) -> Result<ReadyCompilerExecutionTransportV3<'b, 'w>> {
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
        Ok(ReadyCompilerExecutionTransportV3 { readiness, recipe })
    }
}

pub(crate) struct PreparedCompilerExecutionBoundaryV3<'b, 'w> {
    transport: PreparedCompilerExecutionTransportV3<'b, 'w>,
    approval: Approval,
}

/// No detached approval, recipe or account escape. This retains policy approval,
/// not a completed compiler-runtime guard or artifact/launch authority.
pub(crate) struct ReadyCompilerExecutionAttemptV3<'b, 'w> {
    transport: ReadyCompilerExecutionTransportV3<'b, 'w>,
    approval: Approval,
}

impl<'b, 'w> PreparedCompilerExecutionBoundaryV3<'b, 'w> {
    pub(crate) fn prepare(
        approval: Approval,
        closure: CompilerClosureV2,
        recipe: Recipe,
        command: &mut Command,
        budget: &'b mut Budget<'w>,
    ) -> Result<Self> {
        super::require_runtime_enforcement(budget)?;
        recipe.check_account(budget)?;
        let floor = approval
            .required_retained_storage()
            .checked_add(recipe.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        super::pipeline::check_account_floor(budget, floor)?;
        approval.require_compiler(closure, budget)?;
        // The child channel needs inert transport; only this enclosing owner
        // retains the independently loaded approval that permits continuation.
        let (file, storage) = approval.profile().try_clone_for_transfer(budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (profile, storage) = Profile::from_file(file, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let transport =
            PreparedCompilerExecutionTransportV3::prepare(profile, recipe, command, budget)?;
        Ok(Self {
            transport,
            approval,
        })
    }

    pub(crate) fn finish(
        self,
        child_pid: u32,
        deadline: Instant,
    ) -> Result<ReadyCompilerExecutionAttemptV3<'b, 'w>> {
        let Self {
            transport,
            approval,
        } = self;
        let transport = transport.finish(child_pid, deadline)?;
        approval.revalidate(transport.readiness.budget)?;
        Ok(ReadyCompilerExecutionAttemptV3 {
            transport,
            approval,
        })
    }
}

impl<'b, 'w> ReadyCompilerExecutionAttemptV3<'b, 'w> {
    /// Checks successful completion of the exact retained compiler child before
    /// acquiring any publication. Output/path, exact parent invocation and
    /// independently admitted recovery-policy inputs remain separately prepaid. Finalization
    /// and independent durable recovery use the same policy view and retain this
    /// readiness through both stages. Failure/unwind is terminal, without refund.
    #[allow(clippy::too_many_arguments, clippy::result_large_err)]
    pub(crate) fn finalize_after_compiler_success<'a>(
        self,
        child: &mut std::process::Child,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        policy: ConditionalRecoveryPolicy<'_>,
    ) -> std::result::Result<ParentDurableConditionalArtifact<'a, 'b, 'w>, ContinuationError> {
        let Self {
            mut transport,
            approval,
        } = self;
        approval.revalidate(transport.readiness.budget)?;
        transport.readiness.require_compiler_success(child)?;
        let prepared = transport.recipe.finalize_conditional_current(
            transport.readiness,
            approval,
            output_dir,
            producer,
            attempt,
            invocation,
            &policy,
        )?;
        prepared.persist(output_dir, producer, policy)
    }
}

const _: () = assert!(FRAME >= 2 * size_of::<PreparedCompilerExecutionBoundaryV3<'static, 'static>>()
    + 2 * size_of::<ReadyCompilerExecutionAttemptV3<'static, 'static>>()
    + Readiness::OWNER_STORAGE
    + fe2o3_compiler_execution_client::CompilerExecutionSupervisorReadinessV3::CHILD_LAUNCH_STORAGE);
