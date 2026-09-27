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
    CompilerModuleHandoffAdmissionErrorV5, CompilerModuleHandoffReceiptV5 as Receipt,
    consume_compiler_module_handoff_with_currentness_v5 as consume,
};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5 as Handoff;
pub(crate) use fe2o3_hsaco_finalize::ConditionalWorkerRecoveryPolicyV5 as ConditionalRecoveryPolicy;
use fe2o3_hsaco_finalize::{
    ConditionalWorkerCompactFinalizerReplayV5 as Transcript,
    ConditionalWorkerHsacoPublicationErrorV5 as PublicationError, NativeFirstBuildWorkerErrorV1,
    NativeWorkerCompactReplayErrorV1 as TranscriptError, NativeWorkerFinalizationErrorV1,
    PreparedConditionalWorkerHsacoPublicationV5 as Publication,
    PreparedFinalizedConditionalWorkerHsacoV5 as Artifact,
    RecoveredConditionalWorkerHsacoPublicationV5 as DurablePublication,
    execute_preflighted_conditional_reproducible_first_build_worker_v2 as execute,
    finalize_conditional_worker_hsaco_v5 as finalize,
    persist_prepared_conditional_worker_hsaco_publication_v5 as persist_publication,
    preflight_conditional_reproducible_first_build_worker_v2 as preflight,
    prepare_conditional_worker_compact_finalizer_replay_v5 as prepare_transcript,
    prepare_conditional_worker_hsaco_publication_v5 as prepare_publication,
};
use fe2o3_verifier::{
    CompilerConditionalNativeSemanticHandoffErrorV5 as RecoveryError,
    recover_compiler_conditional_native_semantic_handoff_v5 as recover,
};
use std::{fmt, mem::size_of, path::Path};

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

struct ParentArtifactCustody<'a, 'b, 'w> {
    compiler_execution: Carriage,
    readiness: Readiness<'b, 'w>,
    invocation: &'a Invocation,
    configuration_storage: usize,
}
impl ParentPreparedConditionalArtifact<'_, '_, '_> {
    const HEADER: usize = size_of::<Self>()
        - size_of::<Publication>()
        - size_of::<Carriage>()
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
        let (publication, storage) = persist_publication(
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
}

impl ParentDurableConditionalArtifact<'_, '_, '_> {
    const HEADER: usize = size_of::<Self>()
        - size_of::<DurablePublication>()
        - size_of::<Carriage>()
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
    + 2 * size_of::<ParentPreparedConditionalArtifact<'static, 'static, 'static>>()
    + 2 * size_of::<ParentDurableConditionalArtifact<'static, 'static, 'static>>()
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
        mut self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        invocation: &'a Invocation,
        policy: &ConditionalRecoveryPolicy<'_>,
        recipe: PreparedNativeProductionBuildConfig,
    ) -> Result<ParentPreparedConditionalArtifact<'a, 'b, 'w>> {
        self.require_completion()?;
        let (worker, providers, options, output, limits, configuration_storage) =
            recipe.into_worker_parts(self.budget)?;
        let floor = self
            .retained_storage()
            .checked_add(invocation.native_retained_storage()?)
            .and_then(|n| n.checked_add(configuration_storage))
            .ok_or(Resource::Arithmetic)?;
        check_account_floor(self.budget, floor)?;
        self.budget.reserve_storage(FRAME)?;
        self.revalidate()?;
        invocation.revalidate_native(self.budget)?;
        let (lease, token) = self.acquire_current_publication(output_dir, producer, attempt)?;
        let closure = invocation
            .match_native_invocation(token.handoff().capsule().invocation(), self.budget)?;
        let compiler_execution = self.admit_current_receipt(&lease, &token)?;

        // No blanket-refund scope may enclose this concrete terminal recovery.
        let (token, storage) = token.try_map_handoff(self.budget, |handoff, b| {
            recover(
                handoff,
                policy.roots,
                policy.history_limits,
                policy.target,
                b,
            )
            .map(|(source, storage)| (source, storage.retained_storage()))
        })?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let receipt = token.receipt();
        let (prepared, storage) = preflight(
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
        let consumed = consume(&lease, token, self.budget)?;
        check_pair(
            &mut self,
            invocation,
            consumed.receipt(),
            consumed.content().handoff(),
            &compiler_execution,
        )?;
        let (evidence, storage) = execute(consumed, prepared, &worker, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let (artifact, storage) = finalize(evidence, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let (transcript, storage) = prepare_transcript(&artifact, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;
        let (publication, storage) =
            prepare_publication(producer, artifact, transcript, self.budget)?;
        self.budget.reserve_storage(storage.retained_storage())?;

        // Only known success-only scratch is released. Source/preflight/worker
        // reservations remain with the actual retained artifact and receipt.
        let lease_storage = lease.storage().retained_storage();
        drop(lease);
        self.budget.release_storage(lease_storage)?;
        self.budget
            .release_storage(FRAME - ParentPreparedConditionalArtifact::HEADER)?;
        let mut prepared = ParentPreparedConditionalArtifact {
            publication,
            custody: ParentArtifactCustody {
                compiler_execution,
                readiness: self,
                invocation,
                configuration_storage,
            },
        };
        prepared.revalidate()?;
        Ok(prepared)
    }
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
            let (subject, storage) = Subject::from_publication(receipt, handoff, b)?;
            b.reserve_storage(storage.retained_storage())?;
            validate_receipt(&readiness.profile, &subject, carriage, b)?;
            Ok(())
        })
}

fn check_account_floor(
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
}

/// Opaque terminal error: nested resource errors must not invite blanket refunds
/// around an operation that can already have consumed source/proof custody.
#[derive(Debug)]
pub(crate) struct ContinuationError(Cause);
#[derive(Debug)]
enum Cause {
    Resource(Resource),
    Readiness(Failure),
    Invocation(CapabilityError),
    Recovery(CompilerModuleHandoffAdmissionErrorV5<RecoveryError>),
    Transaction(HandoffError),
    Subject(SubjectError),
    Worker(NativeFirstBuildWorkerErrorV1),
    Finalizer(NativeWorkerFinalizationErrorV1),
    Transcript(TranscriptError),
    Publication(PublicationError),
}
macro_rules! causes {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for ContinuationError {
            fn from(error: $ty) -> Self { Self(Cause::$variant(error)) }
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
    CompilerModuleHandoffAdmissionErrorV5<RecoveryError> => Recovery,
    HandoffError => Transaction, SubjectError => Subject,
    NativeFirstBuildWorkerErrorV1 => Worker, NativeWorkerFinalizationErrorV1 => Finalizer,
    TranscriptError => Transcript, PublicationError => Publication);
