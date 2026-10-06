//! Concrete V5 continuation on Cargo's retained native account. Not selected by
//! the production wrapper until configuration, authority and host gates migrate.
#![allow(
    clippy::result_large_err,
    reason = "terminal recovery error retains the transaction lock"
)]

use super::{
    BuildAttempt, CapabilityError, Carriage, Failure, HandoffError,
    ParentCompilerExecutionReadinessCustodyV3 as Readiness, ProducerIdentity, Resource, Subject,
    SubjectError, validate_receipt,
};
use crate::build_config::native::PreparedNativeProductionBuildConfig;
use crate::protected_compiler_handoff_v3::ParentRustcInvocationCustody as Invocation;
use fe2o3_artifact_transaction::{
    ArtifactLockRetirementBarrierV1 as Barrier, CompilerModuleHandoffAdmissionErrorV5,
    CompilerModuleHandoffReceiptV5 as Receipt,
    consume_compiler_module_handoff_in_original_account_v5 as consume_original,
    consume_compiler_module_handoff_with_currentness_v5 as consume,
};
use fe2o3_compiler_closure_capability::{
    RetainedCompilerRuntimeErrorV1 as ApprovalError, RetainedCompilerRuntimeV1 as Approval,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5 as Handoff;
pub(crate) use fe2o3_hsaco_finalize::ConditionalWorkerRecoveryPolicyV5 as ConditionalRecoveryPolicy;
use fe2o3_hsaco_finalize::{
    ConditionalWorkerCompactFinalizerReplayV5 as Transcript,
    ConditionalWorkerHsacoPublicationErrorV5 as PublicationError,
    ConditionalWorkerOutputErrorV5 as OutputError, NativeFirstBuildWorkerErrorV1,
    NativeWorkerCompactReplayErrorV1 as TranscriptError, NativeWorkerFinalizationErrorV1,
    PreparedConditionalWorkerHsacoPublicationV5 as Publication,
    PreparedFinalizedConditionalWorkerHsacoV5 as Artifact,
    PublishedConditionalWorkerHsacoV5 as PublishedOutput,
    RecoveredConditionalWorkerHsacoPublicationV5 as DurablePublication,
    execute_preflighted_conditional_reproducible_first_build_worker_v2 as execute,
    finalize_conditional_worker_hsaco_v5 as finalize,
    persist_prepared_conditional_worker_hsaco_publication_in_original_account_v5 as persist_original,
    persist_prepared_conditional_worker_hsaco_publication_v5 as persist_publication,
    preflight_conditional_reproducible_first_build_worker_v2 as preflight,
    preflight_conditional_worker_in_original_account_v2 as preflight_original,
    prepare_conditional_worker_compact_finalizer_replay_v5 as prepare_transcript,
    prepare_conditional_worker_hsaco_publication_in_original_account_v5 as prepare_original,
    prepare_conditional_worker_hsaco_publication_v5 as prepare_publication,
    publish_recovered_conditional_worker_hsaco_in_original_account_v5 as publish_original,
};
use fe2o3_verifier::{
    CompilerConditionalNativeSemanticHandoffErrorV5 as RecoveryError,
    InertNativeConditionalPolicyRosterV1 as PolicyRoster,
    NativeConditionalPolicyReconstructionErrorV1 as PolicyRosterError,
    reconstruct_inert_native_conditional_policy_roster_in_original_account_v1 as reconstruct_original,
    reconstruct_inert_native_conditional_policy_roster_v1 as reconstruct_policies,
    recover_compiler_conditional_native_semantic_handoff_in_original_account_v5 as recover_original,
    recover_compiler_conditional_native_semantic_handoff_v5 as recover,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    path::Path,
};

/// Move-only structural result. Exact parent/readiness custody and its exclusive
/// budget borrow survive through this owner; there is no detached parts escape.
/// Publication and the sealed verifier/host authority gate remain separate.
pub(crate) struct ParentPreparedConditionalArtifact<'a, 'b, 'w> {
    publication: Publication,
    custody: ParentArtifactCustody<'a, 'b, 'w>,
}

/// The original parent/readiness is retained after independent journal recovery.
/// This is not a constructor from a standalone recovered record, fresh compiler
/// consumption, or sealed verifier/load/launch authority.
pub(crate) struct ParentDurableConditionalArtifact<'a, 'b, 'w> {
    publication: DurablePublication,
    custody: ParentArtifactCustody<'a, 'b, 'w>,
}

/// Exact physically published V5 output plus the original Root parent. This
/// cannot be detached into a V89 load envelope or promoted to GPU authority.
pub(crate) struct ParentPublishedConditionalArtifact<'a, 'b, 'w> {
    publication: PublishedOutput,
    custody: ParentArtifactCustody<'a, 'b, 'w>,
}

struct ParentArtifactCustody<'a, 'b, 'w> {
    approval: Approval,
    compiler_execution: Carriage,
    policy_roster: PolicyRoster,
    readiness: Readiness<'b, 'w>,
    invocation: &'a Invocation,
    configuration_storage: usize,
    policy_origin: PolicyOrigin,
}

// These are independent caller limits/target, never values read from a handoff.
// Only successful original-root admission below records the nominated variant.
#[derive(Clone, Copy)]
struct RecoveryParameters {
    history_limits: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1,
    target: fe2o3_amd_target::ProductionAmdTargetProfileV1,
}
#[derive(Clone, Copy)]
enum PolicyOrigin {
    Independent,
    OriginalRoot(RecoveryParameters),
}
impl PolicyOrigin {
    fn original_parameters(self) -> Result<RecoveryParameters> {
        match self {
            Self::OriginalRoot(parameters) => Ok(parameters),
            Self::Independent => {
                Err(Failure::Mismatch("publication has no original-root policy admission").into())
            }
        }
    }
}
enum SelectedPolicy<'a, 'r> {
    Independent(&'a ConditionalRecoveryPolicy<'r>),
    OriginalRoot(RecoveryParameters),
}
impl ParentPreparedConditionalArtifact<'_, '_, '_> {
    const HEADER: usize = size_of::<Self>()
        - size_of::<Publication>()
        - size_of::<Carriage>()
        - size_of::<PolicyRoster>()
        - size_of::<Approval>()
        - size_of::<Readiness<'static, 'static>>();

    pub(crate) fn artifact(&self) -> &Artifact {
        self.publication.finalized()
    }
    pub(crate) fn compiler_execution(&self) -> &Carriage {
        &self.custody.compiler_execution
    }
    pub(crate) fn transcript(&self) -> &Transcript {
        self.publication.transcript()
    }
    pub(crate) fn revalidate(&mut self) -> Result<()> {
        self.custody.revalidate(
            self.publication.finalized(),
            self.publication.transcript(),
            self.publication.required_retained_storage(),
            Self::HEADER,
        )
    }
}

impl<'a, 'b, 'w> ParentPreparedConditionalArtifact<'a, 'b, 'w> {
    /// Consumes this fresh owner even on failure. The caller keeps independent
    /// policy/path/producer backing prepaid. Never enclose this transition in a
    /// refundable scope; a later refusal cannot roll back an inert journal commit.
    pub(crate) fn persist(
        mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        policy: ConditionalRecoveryPolicy<'_>,
    ) -> Result<ParentDurableConditionalArtifact<'a, 'b, 'w>> {
        self.revalidate()?;
        self.custody
            .readiness
            .origin
            .require_output(output_dir, self.custody.readiness.budget)?;
        self.custody
            .policy_roster
            .require_expected_policies(policy.roots, self.custody.readiness.budget)?;
        let retired = self
            .publication
            .required_retained_storage()
            .checked_add(Self::HEADER)
            .ok_or(Resource::Arithmetic)?;
        let replacement = replacement::Replacement::begin(retired, self.custody.readiness.budget)?;
        let Self {
            publication,
            mut custody,
        } = self;
        // Concrete terminal source recovery is deliberately outside refund scopes.
        let persist = if matches!(custody.readiness.origin, super::Origin::Root(_)) {
            persist_original
        } else {
            persist_publication
        };
        let (publication, storage) = persist(
            output_dir,
            producer,
            publication,
            policy,
            custody.readiness.budget,
        )?;
        let charge = storage.retained_storage();
        if charge != publication.required_retained_storage() {
            return Err(Resource::Accounting.into());
        }
        replacement.reserve(charge, custody.readiness.budget)?;
        custody.revalidate(
            publication.finalized(),
            publication.transcript(),
            charge,
            ParentDurableConditionalArtifact::HEADER,
        )?;
        // The old publication was dropped by successful exact fresh/recovered
        // comparison. Retire only its known charge and our superseded header.
        replacement.finish(
            charge,
            ParentDurableConditionalArtifact::HEADER,
            custody.readiness.budget,
        )?;
        Ok(ParentDurableConditionalArtifact {
            publication,
            custody,
        })
    }

    /// Only the original-root path can nominate its retained policies. Approval,
    /// original completion, exact invocation and signed carriage are revalidated
    /// before lending those values and again after concrete durable recovery.
    /// No policy, artifact or account borrow is synthesized by this transition.
    pub(crate) fn persist_original_root(
        mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> Result<ParentDurableConditionalArtifact<'a, 'b, 'w>> {
        self.revalidate()?;
        self.custody.readiness.origin.require_original_root()?;
        let parameters = self.custody.policy_origin.original_parameters()?;
        self.custody
            .readiness
            .origin
            .require_output(output_dir, self.custody.readiness.budget)?;
        let retired = self
            .publication
            .required_retained_storage()
            .checked_add(Self::HEADER)
            .ok_or(Resource::Arithmetic)?;
        let replacement = replacement::Replacement::begin(retired, self.custody.readiness.budget)?;
        let Self {
            publication,
            mut custody,
        } = self;
        let (publication, storage) = custody.policy_roster.with_root_policies(
            custody.readiness.budget,
            |roots, budget| {
                let (publication, storage) = persist_original(
                    output_dir,
                    producer,
                    publication,
                    ConditionalRecoveryPolicy {
                        roots,
                        history_limits: parameters.history_limits,
                        target: parameters.target,
                    },
                    budget,
                )?;
                budget.reserve_storage(storage.retained_storage())?;
                Ok::<_, ContinuationError>((publication, storage))
            },
        )??;
        let charge = storage.retained_storage();
        if charge != publication.required_retained_storage() {
            return Err(Resource::Accounting.into());
        }
        // The policy-view callback reserved this owner before dropping its view
        // scratch. finish() later checks the exact original replacement balance.
        custody.revalidate(
            publication.finalized(),
            publication.transcript(),
            charge,
            ParentDurableConditionalArtifact::HEADER,
        )?;
        replacement.finish(
            charge,
            ParentDurableConditionalArtifact::HEADER,
            custody.readiness.budget,
        )?;
        Ok(ParentDurableConditionalArtifact {
            publication,
            custody,
        })
    }
}

impl ParentDurableConditionalArtifact<'_, '_, '_> {
    const HEADER: usize = size_of::<Self>()
        - size_of::<DurablePublication>()
        - size_of::<Carriage>()
        - size_of::<PolicyRoster>()
        - size_of::<Approval>()
        - size_of::<Readiness<'static, 'static>>();

    pub(crate) fn publication(&self) -> &DurablePublication {
        &self.publication
    }
    pub(crate) fn compiler_execution(&self) -> &Carriage {
        &self.custody.compiler_execution
    }
    pub(crate) fn revalidate(&mut self) -> Result<()> {
        self.custody.revalidate(
            self.publication.finalized(),
            self.publication.transcript(),
            self.publication.required_retained_storage(),
            Self::HEADER,
        )
    }
}

impl<'a, 'b, 'w> ParentDurableConditionalArtifact<'a, 'b, 'w> {
    /// Physical transaction only. Success still requires the distinct V5 durable
    /// readiness envelope before a managed build can report completion.
    pub(crate) fn publish_original_root(
        mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
    ) -> Result<ParentPublishedConditionalArtifact<'a, 'b, 'w>> {
        self.revalidate()?;
        self.custody.readiness.origin.require_original_root()?;
        self.custody.policy_origin.original_parameters()?;
        self.custody
            .readiness
            .origin
            .require_output(output_dir, self.custody.readiness.budget)?;
        let retired = self
            .publication
            .required_retained_storage()
            .checked_add(Self::HEADER)
            .ok_or(Resource::Arithmetic)?;
        let replacement = replacement::Replacement::begin(retired, self.custody.readiness.budget)?;
        let Self {
            publication,
            mut custody,
        } = self;
        let (publication, extra) =
            publish_original(output_dir, producer, publication, custody.readiness.budget)?;
        let charge = publication.required_retained_storage();
        if charge
            != retired
                .checked_sub(Self::HEADER)
                .and_then(|n| n.checked_add(extra.retained_storage()))
                .ok_or(Resource::Arithmetic)?
        {
            return Err(Resource::Accounting.into());
        }
        // The actual old replay owner moved into this publication. Charge the
        // complete successor before retiring its superseded reservation.
        replacement.reserve(charge, custody.readiness.budget)?;
        let retained = publication.recovered_evidence();
        custody.revalidate(
            retained.finalized(),
            retained.transcript(),
            charge,
            ParentPublishedConditionalArtifact::HEADER,
        )?;
        publication.revalidate(producer, custody.readiness.budget)?;
        replacement.finish(
            charge,
            ParentPublishedConditionalArtifact::HEADER,
            custody.readiness.budget,
        )?;
        Ok(ParentPublishedConditionalArtifact {
            publication,
            custody,
        })
    }
}

impl ParentPublishedConditionalArtifact<'_, '_, '_> {
    const HEADER: usize = size_of::<Self>()
        - size_of::<PublishedOutput>()
        - size_of::<Carriage>()
        - size_of::<PolicyRoster>()
        - size_of::<Approval>()
        - size_of::<Readiness<'static, 'static>>();

    pub(crate) fn publication(&self) -> &PublishedOutput {
        &self.publication
    }
    pub(crate) fn compiler_execution(&self) -> &Carriage {
        &self.custody.compiler_execution
    }
    pub(crate) fn revalidate(&mut self, producer: &ProducerIdentity) -> Result<()> {
        let retained = self.publication.recovered_evidence();
        self.custody.revalidate(
            retained.finalized(),
            retained.transcript(),
            self.publication.required_retained_storage(),
            Self::HEADER,
        )?;
        self.publication
            .revalidate(producer, self.custody.readiness.budget)?;
        Ok(())
    }
}

impl ParentArtifactCustody<'_, '_, '_> {
    fn revalidate(
        &mut self,
        artifact: &Artifact,
        transcript: &Transcript,
        publication_storage: usize,
        header: usize,
    ) -> Result<()> {
        let parent_storage = self.invocation.native_retained_storage()?;
        let floor = publication_storage
            .checked_add(self.compiler_execution.retained_storage())
            .and_then(|n| n.checked_add(self.policy_roster.required_retained_storage()))
            .and_then(|n| n.checked_add(self.approval.required_retained_storage()))
            .and_then(|n| n.checked_add(self.readiness.retained_storage()))
            .and_then(|n| n.checked_add(header))
            .and_then(|n| n.checked_add(parent_storage))
            .and_then(|n| n.checked_add(self.configuration_storage))
            .ok_or(Resource::Arithmetic)?;
        check_account_floor(self.readiness.budget, floor)?;
        // Fixed comparisons of the occurrence, source and finalization coordinates.
        self.readiness.budget.charge_work(1024)?;
        transcript.verify_finalized_coordinates(artifact)?;
        self.readiness.revalidate()?;
        let closure = *artifact
            .source()
            .recovered_handoff()
            .handoff()
            .capsule()
            .invocation()
            .compiler_closure();
        self.approval
            .require_compiler(closure, self.readiness.budget)?;
        require_approved_profile(&self.approval, &mut self.readiness)?;
        check_pair(
            &mut self.readiness,
            self.invocation,
            artifact.source().binding().receipt(),
            artifact.source().recovered_handoff().handoff(),
            &self.compiler_execution,
        )
    }
}

const FRAME: usize = 4 * size_of::<ContinuationError>()
    + size_of::<RetirementPanic>()
    + size_of::<super::artifact::CurrentPublication>()
    + 2 * size_of::<ParentPreparedConditionalArtifact<'static, 'static, 'static>>()
    + 2 * size_of::<ParentDurableConditionalArtifact<'static, 'static, 'static>>()
    + 2 * size_of::<ParentPublishedConditionalArtifact<'static, 'static, 'static>>()
    + size_of::<replacement::Replacement>()
    + 8192;
type Result<T> = std::result::Result<T, ContinuationError>;

#[path = "compiler_execution_boundary_native_replacement.rs"]
mod replacement;

impl<'b, 'w> Readiness<'b, 'w> {
    /// Requires observed successful child completion. The enclosing attempt keeps
    /// path/producer, parent capture and policy inputs prepaid. The recipe must
    /// have been prepared on this same account and retains its original floor.
    /// This consumes the recipe's owned vectors without cloning caller payloads.
    ///
    /// This is a terminal operation on refusal/unwind. In particular, no scope
    /// refunds reservations around V5 recovery or consumes a failed admission.
    /// Readiness is consumed even on failure, preventing reuse of this owner.
    /// The admission error itself retains its lock until destroyed. Successful
    /// returned owners are already charged on this custody's original account.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finalize_current_publication<'a>(
        self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        policy: &ConditionalRecoveryPolicy<'_>,
        recipe: PreparedNativeProductionBuildConfig,
        approval: Approval,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>> {
        self.finalize_current_using(
            output_dir,
            producer,
            attempt,
            invocation,
            SelectedPolicy::Independent(policy),
            recipe,
            approval,
        )
    }

    /// Trust compiler-nominated proof keys only after the original root runtime
    /// completion, fixed-root approved images/profile, exact invocation and
    /// current signed publication all join. This is not a packet-only provider.
    /// Independent history limits and target still come from the managed build.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finalize_original_root_publication<'a>(
        self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        history_limits: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1,
        target: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        recipe: PreparedNativeProductionBuildConfig,
        approval: Approval,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>> {
        self.origin.require_original_root()?;
        self.finalize_current_using(
            output_dir,
            producer,
            attempt,
            invocation,
            SelectedPolicy::OriginalRoot(RecoveryParameters {
                history_limits,
                target,
            }),
            recipe,
            approval,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finalize_current_using<'a>(
        mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        policy: SelectedPolicy<'_, '_>,
        recipe: PreparedNativeProductionBuildConfig,
        approval: Approval,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>> {
        self.require_runtime_enforcement(invocation)?;
        self.require_completion()?;
        let (worker, providers, options, output, limits, configuration_storage) =
            recipe.into_worker_parts(self.budget)?;
        let floor = self
            .retained_storage()
            .checked_add(invocation.native_retained_storage()?)
            .and_then(|n| n.checked_add(configuration_storage))
            .and_then(|n| n.checked_add(approval.required_retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        check_account_floor(self.budget, floor)?;
        self.budget.reserve_storage(FRAME)?;
        self.revalidate()?;
        invocation.revalidate_native(self.budget)?;
        let super::artifact::CurrentPublication {
            lease,
            token,
            retirement,
        } = self.acquire_current_publication(output_dir, producer, attempt)?;
        let original = retirement.is_some();
        let (consumed, prepared, compiler_execution, policy_roster, policy_origin) =
            with_retirement(retirement, || {
                let closure = invocation
                    .match_native_invocation(token.handoff().capsule().invocation(), self.budget)?;
                approval.require_compiler(closure, self.budget)?;
                require_approved_profile(&approval, &mut self)?;
                let compiler_execution = self.admit_current_receipt(&lease, &token)?;

                // No policy value is lent before ALL original occurrence, approval and
                // signed-current-publication checks above succeed. The independent path
                // additionally requires exact equality with its supplied policy backing.
                let reconstruct = if original {
                    reconstruct_original
                } else {
                    reconstruct_policies
                };
                let (policy_roster, storage) = reconstruct(
                    token.handoff().capsule().policy_roster_bytes(),
                    token.handoff().capsule().source_packet_bytes(),
                    self.budget,
                )?;
                self.budget.reserve_storage(storage.retained_storage())?;
                if let SelectedPolicy::Independent(policy) = &policy {
                    policy_roster.require_expected_policies(policy.roots, self.budget)?;
                }
                if original {
                    token.revalidate_locked_currentness_in_original_account_v5(self.budget)?;
                } else {
                    token.revalidate_locked_currentness(self.budget)?;
                }

                // No blanket-refund scope may enclose this concrete terminal recovery.
                let (token, policy_origin) = match policy {
                    SelectedPolicy::Independent(policy) => {
                        let recover_using = if original { recover_original } else { recover };
                        let admit = |handoff, b: &mut super::Budget<'_>| {
                            recover_using(
                                handoff,
                                policy.roots,
                                policy.history_limits,
                                policy.target,
                                b,
                            )
                            .map(|(source, storage)| (source, storage.retained_storage()))
                        };
                        let (token, storage) = if original {
                            token.try_map_handoff_in_original_account_v5(self.budget, admit)
                        } else {
                            token.try_map_handoff(self.budget, admit)
                        }?;
                        self.budget.reserve_storage(storage.retained_storage())?;
                        (token, PolicyOrigin::Independent)
                    }
                    SelectedPolicy::OriginalRoot(parameters) => {
                        self.origin.require_original_root()?;
                        approval.require_compiler(closure, self.budget)?;
                        require_approved_profile(&approval, &mut self)?;
                        self.revalidate()?;
                        check_pair(
                            &mut self,
                            invocation,
                            token.receipt(),
                            token.handoff(),
                            &compiler_execution,
                        )?;
                        let token =
                            policy_roster.with_root_policies(self.budget, |roots, b| {
                                let (token, storage) = token
                                    .try_map_handoff_in_original_account_v5(b, |handoff, b| {
                                        recover_original(
                                            handoff,
                                            roots,
                                            parameters.history_limits,
                                            parameters.target,
                                            b,
                                        )
                                        .map(
                                            |(source, storage)| {
                                                (source, storage.retained_storage())
                                            },
                                        )
                                    })?;
                                b.reserve_storage(storage.retained_storage())?;
                                Ok::<_, ContinuationError>(token)
                            })??;
                        (token, PolicyOrigin::OriginalRoot(parameters))
                    }
                };
                // Each branch retains the mapped owner before any lent policy view dies.
                let receipt = token.receipt();
                let (prepared, storage) = (if original {
                    preflight_original
                } else {
                    preflight
                })(
                    &token,
                    receipt,
                    closure,
                    &worker,
                    providers,
                    options,
                    output,
                    limits,
                    self.budget,
                )?;
                self.budget.reserve_storage(storage.retained_storage())?;
                self.revalidate()?;
                invocation.match_native_invocation(
                    token.content().handoff().capsule().invocation(),
                    self.budget,
                )?;
                let consumed = if original {
                    consume_original(&lease, token, self.budget)
                } else {
                    consume(&lease, token, self.budget)
                }?;
                check_pair(
                    &mut self,
                    invocation,
                    consumed.receipt(),
                    consumed.content().handoff(),
                    &compiler_execution,
                )?;
                let lease_storage = lease.storage().retained_storage();
                drop(lease);
                self.budget.release_storage(lease_storage)?;
                Ok((
                    consumed,
                    prepared,
                    compiler_execution,
                    policy_roster,
                    policy_origin,
                ))
            })?;
        // The actual original retirement barrier has now dropped. Worker spawn
        // cannot start while either current-publication lock remains held.
        let (evidence, storage) = execute(consumed, prepared, &worker, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let (artifact, storage) = finalize(evidence, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let (transcript, storage) = prepare_transcript(&artifact, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let prepare = if original {
            prepare_original
        } else {
            prepare_publication
        };
        let (publication, storage) = prepare(producer, artifact, transcript, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;

        // Only known success-only scratch is released. Source/preflight/worker
        // reservations remain with the actual retained artifact and receipt.
        self.budget
            .release_storage(FRAME - ParentPreparedConditionalArtifact::HEADER)?;
        let mut prepared = ParentPreparedConditionalArtifact {
            publication,
            custody: ParentArtifactCustody {
                approval,
                compiler_execution,
                policy_roster,
                readiness: self,
                invocation,
                configuration_storage,
                policy_origin,
            },
        };
        prepared.revalidate()?;
        Ok(prepared)
    }
}

fn require_approved_profile(approval: &Approval, readiness: &mut Readiness<'_, '_>) -> Result<()> {
    let floor = approval
        .required_retained_storage()
        .checked_add(readiness.retained_storage())
        .ok_or(Resource::Arithmetic)?;
    check_account_floor(readiness.budget, floor)?;
    readiness.budget.charge_work(
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3,
    )?;
    if approval.profile().profile().canonical_bytes()
        != readiness.profile.profile().canonical_bytes()
    {
        return Err(
            ApprovalError::Mismatch("readiness differs from root-approved V3 profile").into(),
        );
    }
    Ok(())
}

fn check_pair(
    readiness: &mut Readiness<'_, '_>,
    invocation: &Invocation,
    receipt: Receipt,
    handoff: &Handoff,
    carriage: &Carriage,
) -> Result<()> {
    invocation.match_native_invocation(handoff.capsule().invocation(), readiness.budget)?;
    let floor = readiness.budget.storage();
    check_account_floor(readiness.budget, floor)?;
    readiness
        .budget
        .with_prepaid_scope(floor, 0, 0, FRAME, |b| {
            let (subject, storage) = if matches!(readiness.origin, super::Origin::Root(_)) {
                Subject::from_publication_in_original_account_v3(receipt, handoff, b)
            } else {
                Subject::from_publication(receipt, handoff, b)
            }?;
            b.reserve_storage(storage.retained_storage())?;
            readiness.origin.require_subject(&subject, b)?;
            validate_receipt(&readiness.profile, &subject, carriage, b)?;
            Ok(())
        })
}

pub(super) fn check_account_floor(
    b: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    floor: usize,
) -> std::result::Result<(), Resource> {
    // The total scope work includes, rather than adds to, its entry work.
    b.with_prepaid_scope(floor, 8, 8, 0, |_| Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    fn isolated_retirement_test(name: &str) -> bool {
        const ENV: &str = "FE2O3_ORIGINAL_PARENT_RETIREMENT_TEST";
        if std::env::var_os(ENV).is_some() {
            return false;
        }
        let full = format!("{}::{name}", module_path!());
        let (_, test) = full.split_once("::").unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", test, "--nocapture"]).env(ENV, "1");
        let mut child =
            fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "original parent retirement test: {status}"
                );
                return true;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                child.wait().unwrap();
                panic!("original parent retirement test timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn actual_parent_retirement_survives_error_and_unwind_payload_destruction() {
        if isolated_retirement_test(
            "actual_parent_retirement_survives_error_and_unwind_payload_destruction",
        ) {
            return;
        }
        use fe2o3_artifact_transaction::try_acquire_artifact_lock_retirement_barrier_v1 as acquire;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Payload(Arc<AtomicUsize>);
        impl Drop for Payload {
            fn drop(&mut self) {
                assert!(
                    acquire().is_err(),
                    "payload outlived its actual retirement barrier"
                );
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let barrier = acquire().unwrap();
        let error =
            with_retirement::<()>(Some(barrier), || Err(Resource::Accounting.into())).unwrap_err();
        assert!(acquire().is_err());
        drop(error);
        drop(acquire().unwrap());

        let drops = Arc::new(AtomicUsize::new(0));
        let barrier = acquire().unwrap();
        let caught = catch_unwind(AssertUnwindSafe(|| {
            with_retirement::<()>(Some(barrier), || {
                std::panic::panic_any(Payload(Arc::clone(&drops)))
            })
        }));
        assert!(caught.is_err());
        assert!(acquire().is_err());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(caught);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        drop(acquire().unwrap());
    }

    #[test]
    fn actual_parent_retirement_releases_before_successful_worker_continuation() {
        if isolated_retirement_test(
            "actual_parent_retirement_releases_before_successful_worker_continuation",
        ) {
            return;
        }
        use fe2o3_artifact_transaction::try_acquire_artifact_lock_retirement_barrier_v1 as acquire;
        let barrier = acquire().unwrap();
        assert_eq!(
            with_retirement(Some(barrier), || {
                assert!(acquire().is_err());
                Ok(17)
            })
            .unwrap(),
            17
        );
        drop(acquire().unwrap());
        assert!(
            FRAME
                >= size_of::<RetirementPanic>()
                    + size_of::<super::super::artifact::CurrentPublication>()
        );
    }

    #[test]
    fn continuation_account_floor_accepts_exact_and_rejects_short_resources() {
        for (quota, storage) in [(8, 96), (7, 96), (8, 95)] {
            let mut work = Work::new(quota);
            let mut b = Budget::new(&mut work, 96);
            b.reserve_storage(storage).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = check_account_floor(&mut b, 96);
            if quota == 7 {
                assert!(matches!(result, Err(Resource::Work(_))));
            } else if storage == 95 {
                assert_eq!(result, Err(Resource::Accounting));
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(b.work(), 8);
            }
            assert_eq!(b.storage(), storage);
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }

    #[test]
    fn independent_policy_origin_cannot_use_nominated_persistence() {
        assert!(PolicyOrigin::Independent.original_parameters().is_err());
        for target in [
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let parameters = RecoveryParameters {
                history_limits: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1 {
                    refinement: Default::default(),
                    forwarding: Default::default(),
                },
                target,
            };
            // This private scalar test constructs no approval, runtime, root
            // completion or artifact. It only checks the selected policy mode.
            let retained = PolicyOrigin::OriginalRoot(parameters)
                .original_parameters()
                .unwrap();
            assert_eq!(retained.history_limits, parameters.history_limits);
            assert_eq!(retained.target, target);
        }
    }
}

/// Opaque terminal error: nested resource errors must not invite blanket refunds
/// around an operation that can already have consumed source/proof custody.
#[derive(Debug)]
pub(crate) struct ContinuationError(Cause, Option<Barrier>);

// The inner payload can own a terminal error and its output lock. Its destructor
// must finish while the original barrier still excludes process creation.
struct RetirementPanic {
    _payload: Box<dyn std::any::Any + Send>,
    _barrier: Option<Barrier>,
}

fn with_retirement<T>(barrier: Option<Barrier>, run: impl FnOnce() -> Result<T>) -> Result<T> {
    match catch_unwind(AssertUnwindSafe(run)) {
        Ok(Ok(value)) => {
            drop(barrier);
            Ok(value)
        }
        Ok(Err(mut error)) => {
            // This private scope is entered once, immediately after acquisition.
            assert!(error.1.is_none());
            error.1 = barrier;
            Err(error)
        }
        Err(payload) => resume_unwind(Box::new(RetirementPanic {
            _payload: payload,
            _barrier: barrier,
        })),
    }
}
#[derive(Debug)]
enum Cause {
    Approval(ApprovalError),
    Resource(Resource),
    Readiness(Failure),
    Invocation(CapabilityError),
    PolicyRoster(PolicyRosterError),
    Recovery(CompilerModuleHandoffAdmissionErrorV5<RecoveryError>),
    Transaction(HandoffError),
    Subject(SubjectError),
    Worker(NativeFirstBuildWorkerErrorV1),
    Finalizer(NativeWorkerFinalizationErrorV1),
    Transcript(TranscriptError),
    Publication(PublicationError),
    Output(OutputError),
}
macro_rules! causes {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for ContinuationError {
            fn from(error: $ty) -> Self { Self(Cause::$variant(error), None) }
        })+
        impl fmt::Display for ContinuationError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match &self.0 { $(Cause::$variant(error) => error.fmt(f),)+ }
            }
        }
        impl std::error::Error for ContinuationError {}
    };
}
causes!(Resource => Resource, Failure => Readiness, CapabilityError => Invocation,
    ApprovalError => Approval,
    PolicyRosterError => PolicyRoster,
    CompilerModuleHandoffAdmissionErrorV5<RecoveryError> => Recovery,
    HandoffError => Transaction, SubjectError => Subject,
    NativeFirstBuildWorkerErrorV1 => Worker, NativeWorkerFinalizationErrorV1 => Finalizer,
    TranscriptError => Transcript, PublicationError => Publication, OutputError => Output);
