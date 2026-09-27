//! Conditional V5 source custody through the existing reproducible worker engine.
use crate::{
    LinkOptionV1, MultiInputLinkPlanV1, NativeFirstBuildWorkerErrorV1 as Error, PinnedWorkerV1,
    WorkerExecutionLimitsV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerResponseV2,
    first_build_worker_conditional_binding::{
        ProtectedCompilerConditionalHandoffBindingV2 as Binding, require_storage_limit,
    },
    first_build_worker_engine::ReproducibleFirstBuildEnginePreflight as Engine,
    first_build_worker_native::failure,
    first_build_worker_native_resources::NativeWorkerResourceQuote as Quote,
    native_worker_engine::{execute_native_engine, prepare_native_engine},
    worker_executor::InertWorkerExecutionV2,
};
use fe2o3_artifact_transaction::{
    CompilerModuleHandoffConsumptionTokenV5 as Token, CompilerModuleHandoffErrorV5 as HandoffError,
    CompilerModuleHandoffReceiptV5 as Receipt, ConsumedCompilerModuleHandoffV5 as Consumed,
};
use fe2o3_build_authority::CompilerClosureV2 as Closure;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::RecoveredCompilerConditionalNativeSemanticHandoffV5 as Source;
use std::mem::size_of;
const ENTRY_WORK: usize = 8192;
const FRAME: usize = 4 * size_of::<Binding>() + size_of::<Quote>() + 4096;

/// Additional unreserved storage. Keep source and preflight reservations paid
/// until the returned evidence drops; this is logical Rust storage, not RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalFirstBuildWorkerStorageV2(usize);
impl ConditionalFirstBuildWorkerStorageV2 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
use self::ConditionalFirstBuildWorkerStorageV2 as Storage;

/// The actual conditional source family cannot be replaced by an ordinary one.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedConditionalFirstBuildWorkerV2 as C,
///     PreparedNativeFirstBuildWorkerV1 as N};
/// fn downgrade(c: C) -> N { c.into() }
/// ```
pub struct PreparedConditionalFirstBuildWorkerV2 {
    binding: Binding,
    worker: WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    quote: Quote,
    engine: Engine,
    storage: Storage,
}
type Prepared = PreparedConditionalFirstBuildWorkerV2;
impl Prepared {
    pub const fn binding(&self) -> Binding {
        self.binding
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
}

/// Retains the actual consumed conditional source, final graph and catalog with
/// both measured worker exchanges. No source replay or ordinary projection is
/// substituted. Reproducibility grants no protected origin, machine refinement,
/// publication, load or launch authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::InertConditionalFirstBuildWorkerEvidenceV2 as E;
/// fn duplicate(e: E) { let _ = e.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::InertConditionalFirstBuildWorkerEvidenceV2 as E;
/// fn manufacture() -> E { E::default() }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{InertConditionalFirstBuildWorkerEvidenceV2 as C,
///     InertNativeFirstBuildWorkerEvidenceV1 as N};
/// fn downgrade(c: C) -> N { c.into() }
/// ```
pub struct InertConditionalFirstBuildWorkerEvidenceV2 {
    source: Consumed<Source>,
    binding: Binding,
    identity: [u8; 32],
    worker: WorkerMeasurementV1,
    limits: WorkerExecutionLimitsV1,
    plan: MultiInputLinkPlanV1,
    bootstrap_request: Vec<u8>,
    bootstrap: InertWorkerExecutionV2,
    replay_request: Vec<u8>,
    replay: InertWorkerExecutionV2,
    storage: Storage,
    retained_storage: usize,
}
type Evidence = InertConditionalFirstBuildWorkerEvidenceV2;
impl Evidence {
    pub(crate) fn revalidate_for_artifact(&self, b: &mut Budget<'_>) -> Result<(), Error> {
        b.with_prepaid_scope(self.retained_storage, 8, ENTRY_WORK, FRAME, |b| {
            if self.source.receipt() != self.binding.receipt()
                || Binding::from_handoff(
                    self.source.content(),
                    self.source.receipt(),
                    self.binding.compiler_closure(),
                    b,
                )? != self.binding
            {
                return Err(Error::PreflightMismatch("conditional artifact source"));
            }
            Ok(())
        })
    }
    pub(crate) fn artifact_lineage(
        &self,
    ) -> crate::worker_hsaco_lineage::WorkerArtifactLineage<'_> {
        use crate::worker_hsaco_lineage::{WorkerArtifactExchange, WorkerArtifactLineage};
        WorkerArtifactLineage {
            module: self.recovered_handoff().handoff().module_handoff(),
            plan: &self.plan,
            measurement: &self.worker,
            exchanges: [
                WorkerArtifactExchange {
                    request: &self.bootstrap_request,
                    response: self.bootstrap.response(),
                    executable: self.bootstrap.worker_executable(),
                },
                WorkerArtifactExchange {
                    request: &self.replay_request,
                    response: self.replay.response(),
                    executable: self.replay.worker_executable(),
                },
            ],
            output: self.output_bytes(),
        }
    }
    pub const fn recovered_handoff(&self) -> &Source {
        self.source.content()
    }
    pub const fn binding(&self) -> Binding {
        self.binding
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker
    }
    pub const fn execution_limits(&self) -> WorkerExecutionLimitsV1 {
        self.limits
    }
    pub const fn plan(&self) -> &MultiInputLinkPlanV1 {
        &self.plan
    }
    pub fn bootstrap_request_bytes(&self) -> &[u8] {
        &self.bootstrap_request
    }
    pub fn replay_request_bytes(&self) -> &[u8] {
        &self.replay_request
    }
    pub fn bootstrap_response(&self) -> &WorkerResponseV2 {
        self.bootstrap.response()
    }
    pub fn replay_response(&self) -> &WorkerResponseV2 {
        self.replay.response()
    }
    pub fn output_bytes(&self) -> &[u8] {
        self.replay
            .response()
            .output()
            .expect("shared engine requires output")
            .bytes()
    }
    pub fn output_identity(&self) -> crate::ContentIdentityV1 {
        self.replay
            .response()
            .output()
            .expect("shared engine requires output")
            .identity()
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
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

/// All configuration-dependent checks run on the locked V5 token before consume.
/// The caller prepays the full recovered token and reserves the returned charge.
/// Worker process/LLVM limits and caller spare capacities remain the existing
/// engine's separate accounting domains, as for native V4 staging.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV5 as Token;
/// use fe2o3_verifier::RecoveredCompilerConditionalNativeSemanticHandoffV5 as Source;
/// fn bypass_recovery(raw: Token) -> Token<Source> { raw }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn preflight_conditional_reproducible_first_build_worker_v2(
    token: &Token<Source>,
    receipt: Receipt,
    closure: Closure,
    worker: &PinnedWorkerV1,
    providers: Vec<WorkerInputV1>,
    options: Vec<LinkOptionV1>,
    output: WorkerOutputConstraintsV1,
    limits: WorkerExecutionLimitsV1,
    b: &mut Budget<'_>,
) -> Result<(Prepared, Storage), Error> {
    b.with_prepaid_scope(
        token.storage().retained_storage(),
        16,
        ENTRY_WORK,
        FRAME,
        |b| {
            require_storage_limit(b)?;
            if token.receipt() != receipt {
                return Err(Error::PreflightMismatch("locked V5 receipt"));
            }
            token
                .revalidate_locked_currentness(b)
                .map_err(currentness_error)?;
            let binding = Binding::from_handoff(token.content(), receipt, closure, b)?;
            let handoff = token.content().handoff();
            let (engine, quote) = prepare_native_engine(
                (&binding).into(),
                handoff.canonical_bytes().len(),
                handoff.module_handoff(),
                worker,
                providers,
                options,
                output,
                limits,
                size_of::<Prepared>(),
                b,
            )?;
            let storage = Storage(
                quote
                    .preflight_storage
                    .checked_add(size_of::<Prepared>())
                    .ok_or(Resource::Arithmetic)?,
            );
            b.reserve_storage(storage.0)?;
            token
                .revalidate_locked_currentness(b)
                .map_err(currentness_error)?;
            Ok((
                Prepared {
                    binding,
                    worker: worker.measurement().clone(),
                    limits,
                    quote,
                    engine,
                    storage,
                },
                storage,
            ))
        },
    )
}

/// Rechecks the exact consumed occurrence and pinned worker before any process
/// starts. Successful output keeps the original V5 source allocation, not just
/// a digest. The returned charge is additional to source and preflight storage.
pub fn execute_preflighted_conditional_reproducible_first_build_worker_v2(
    consumed: Consumed<Source>,
    preflight: Prepared,
    worker: &PinnedWorkerV1,
    b: &mut Budget<'_>,
) -> Result<(Evidence, Storage), Error> {
    let floor = consumed
        .storage()
        .retained_storage()
        .checked_add(preflight.storage.0)
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(floor, 8, ENTRY_WORK, FRAME, |b| {
        require_storage_limit(b)?;
        if consumed.receipt() != preflight.binding.receipt() {
            return Err(Error::PreflightMismatch("consumed V5 receipt"));
        }
        let binding = Binding::from_handoff(
            consumed.content(),
            consumed.receipt(),
            preflight.binding.compiler_closure(),
            b,
        )?;
        if binding != preflight.binding {
            return Err(Error::PreflightMismatch("conditional source/F binding"));
        }
        if worker.measurement() != &preflight.worker {
            return Err(Error::PreflightMismatch("measured worker"));
        }
        let Prepared {
            worker: measurement,
            limits,
            quote,
            engine,
            ..
        } = preflight;
        let storage = Storage(
            quote
                .returned_retained_storage()
                .checked_add(size_of::<Evidence>())
                .ok_or(Resource::Arithmetic)?,
        );
        let scratch = quote
            .execution_storage
            .checked_add(size_of::<Evidence>())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 0, quote.execution_work, scratch, |_| {
            let (result, identity) =
                execute_native_engine((&binding).into(), engine, &measurement, limits, worker)?;
            Ok((
                Evidence {
                    source: consumed,
                    binding,
                    identity,
                    worker: measurement,
                    limits,
                    plan: result.plan,
                    bootstrap_request: result.candidate_request_bytes,
                    bootstrap: result.candidate,
                    replay_request: result.authorized_request_bytes,
                    replay: result.authorized,
                    storage,
                    retained_storage: floor.checked_add(storage.0).ok_or(Resource::Arithmetic)?,
                },
                storage,
            ))
        })
    })
}

fn currentness_error(error: HandoffError) -> Error {
    match error {
        HandoffError::Resource(e) => e.into(),
        other => failure("V5 currentness", other),
    }
}
