//! One-shot orchestration; no new verifier, carrier, or native execution authority.

use std::{future::Future, pin::Pin, time::Instant};

use fe2o3_runtime::*;

use crate::{
    AqlDispatchGeometryV1, AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    CompilerGeneratedRuntimeArguments, GeneratedRuntimeArgumentLimitsV1,
    GeneratedRuntimeCompletedBundleV1, GeneratedRuntimeResultBudgetV1,
    GeneratedRuntimeTypedBindErrorV1, GeneratedRuntimeTypedCompletionErrorV1,
    GeneratedRuntimeTypedOutputBundleV1, GeneratedWorkerV3ContextInvocationErrorV1,
    GeneratedWorkerV3RuntimeInvocationErrorV1, WorkerV3ProtectedVerifierBackendV1,
    WorkerV3RefiningProtectedVerifierAdapterV1, WorkerV3RefiningProtectedVerifierErrorV1,
    WorkerV3SemanticMachineRefinementBackendV1,
};

use super::{
    ProductionWorkerV3AuthenticationErrorV1, authenticate_inherited_worker_v3_application_v1,
};

#[cfg(test)]
mod tests;

/// Bounds for one invocation on the calling thread. The deadline is cooperative:
/// it cannot interrupt the argument constructor, a backend call, verifier,
/// decoder, or shutdown.
pub struct ProductionWorkerV3CurrentThreadConfigV1 {
    pub device_unique_id: u64,
    pub geometry: AqlDispatchGeometryV1,
    pub dynamic_group_segment_bytes: u32,
    pub timeout_milliseconds: u32,
    pub argument_limits: GeneratedRuntimeArgumentLimitsV1,
    pub engine: RuntimeAsyncEngineConfigV1,
    pub progress: RuntimeAsyncProgressConfigV1,
    pub journal_allocation_capacity: usize,
    pub journal_writer_capacity: usize,
    pub drain_tick_budget: usize,
    pub deadline: Instant,
}

impl ProductionWorkerV3CurrentThreadConfigV1 {
    fn preflight<VE>(&self) -> Result<(), ProductionWorkerV3CurrentThreadErrorV1<VE>> {
        let bounds = 1..=fe2o3_runtime_model::CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
        if !bounds.contains(&self.journal_allocation_capacity)
            || !bounds.contains(&self.journal_writer_capacity)
            || !(1..=MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1).contains(&self.drain_tick_budget)
            // Reservation needs its finite reply and the original completion cell.
            || self.engine.reply_capacity() < 2
        {
            return Err(ProductionWorkerV3CurrentThreadErrorV1::InvalidBounds);
        }
        if Instant::now() >= self.deadline {
            return Err(ProductionWorkerV3CurrentThreadErrorV1::Drive(
                ProductionWorkerV3ApplicationStageV1::Initialize,
                RuntimeAsyncDriveErrorV1::DeadlineExceeded,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionWorkerV3ApplicationStageV1 {
    Initialize,
    CreateStream,
    Prepare,
    Reserve,
    Activate,
    Complete,
    DestroyStream,
    Drain,
}

/// Failure before an owned Context exists. Context construction can already
/// have called the backend, so that failure retains the backend until process
/// exit rather than invoking its destructor as though initialization were pure.
/// Context initialization unwind also retains the backend, but is reported by
/// the outer owner's `InitializerPanicked`, not by this returned-error enum.
#[derive(Debug)]
pub enum ProductionWorkerV3RuntimeInitializationErrorV1 {
    Backend(KfdRuntimeBackendErrorV1),
    ContextRetainedUntilProcessExit(RuntimeErrorV1<KfdRuntimeBackendErrorV1>),
}

/// The primary failure, separate from drain and owned shutdown disposition.
/// This one-shot API consumes argument/output observers and offers no retry or
/// ticket recovery. Stop/shutdown, not observer destruction, settles custody.
#[derive(Debug)]
pub enum ProductionWorkerV3CurrentThreadErrorV1<VE> {
    Authentication(ProductionWorkerV3AuthenticationErrorV1<VE>),
    InvalidBounds,
    Initialization(
        RuntimeAsyncCurrentThreadInitErrorV1<ProductionWorkerV3RuntimeInitializationErrorV1>,
    ),
    Drive(
        ProductionWorkerV3ApplicationStageV1,
        RuntimeAsyncDriveErrorV1,
    ),
    Engine(
        ProductionWorkerV3ApplicationStageV1,
        RuntimeAsyncEngineCallErrorV1,
    ),
    Context(
        ProductionWorkerV3ApplicationStageV1,
        RuntimeErrorV1<KfdRuntimeBackendErrorV1>,
    ),
    Invocation(GeneratedWorkerV3RuntimeInvocationErrorV1),
    Preparation(GeneratedWorkerV3ContextInvocationErrorV1),
    Reservation(RuntimeGfx942GeneratedReservationErrorV1),
    Activation(RuntimeAsyncGeneratedActivationErrorV1<KfdRuntimeBackendErrorV1>),
    Binding(GeneratedRuntimeTypedBindErrorV1),
    Completion(GeneratedRuntimeTypedCompletionErrorV1),
    Drain(RuntimeAsyncDrainErrorV1),
    DrainIncomplete,
}

/// Completed results remain available even when subsequent drain/shutdown fails.
/// `error == None` alone is not success: inspect `is_success()` and the exact
/// reports. A handoff ACK is never represented as a completion receipt here.
#[must_use = "inspect execution, drain, and shutdown disposition"]
pub struct ProductionWorkerV3CurrentThreadReportV1<R, VE> {
    pub completed: Option<GeneratedRuntimeCompletedBundleV1<R>>,
    pub error: Option<ProductionWorkerV3CurrentThreadErrorV1<VE>>,
    pub drain: Option<RuntimeAsyncDrainReportV1>,
    /// None means that no complete runtime owner was constructed.
    pub shutdown: Option<RuntimeAsyncOwnedShutdownV1<KfdRuntimeBackendErrorV1>>,
}

impl<R, VE> ProductionWorkerV3CurrentThreadReportV1<R, VE> {
    pub fn is_success(&self) -> bool {
        self.completed.is_some()
            && self.error.is_none()
            && self
                .drain
                .as_ref()
                .is_some_and(|report| report.outcome == RuntimeAsyncDrainOutcomeV1::Quiescent)
            && self.shutdown.as_ref().is_some_and(|report| {
                report.disposition == RuntimeAsyncOwnedDispositionV1::Released
                    && !report.worker_panicked
                    && report.native_failure.is_none()
            })
    }

    fn failed(error: ProductionWorkerV3CurrentThreadErrorV1<VE>) -> Self {
        Self {
            completed: None,
            error: Some(error),
            drain: None,
            shutdown: None,
        }
    }
}

/// Authenticates Cargo's inherited handoff, runs one generated invocation without
/// spawning a thread, collects the selected charged outputs, drains, and shuts
/// down the generated-only KFD owner. Requires the refining adapter, not a
/// verifier that supplies only a protected acceptance decision. No concrete
/// production verifier/refinement provider is supplied by this helper.
///
/// The argument constructor runs once after authentication/evidence/bounds
/// validation and before native construction; it receives no runtime authority.
/// Return `()` to observe only completion, `(output,)` for one output, or a sealed
/// tuple of up to 64 outputs. Result accounting and decoding use the original
/// generated storage and result budget even when no outputs are selected.
///
/// Any stage failure or cooperative deadline expiry immediately selects owned
/// Stop/shutdown. It is not cancellation, rollback, or a resumable timeout.
/// Parked tickets stay owned by that runtime until shutdown; failed native
/// cleanup retains custody until process exit. The report preserves the primary
/// failure and exact shutdown disposition. Panics in caller-owned destructors
/// follow the lower APIs' unwind contract and do not produce a normal report.
/// Context initialization unwind retains its backend until process exit and
/// reports `InitializerPanicked`; no owned-shutdown report exists at that stage.
///
/// ```no_run
/// use fe2o3_host::*;
/// fn application<K, P, R, A, O>(
///     verifier: &mut WorkerV3RefiningProtectedVerifierAdapterV1<P, R>,
///     arguments: impl FnOnce() -> (A, O),
///     budget: &GeneratedRuntimeResultBudgetV1,
///     config: ProductionWorkerV3CurrentThreadConfigV1,
/// ) where
///     K: CompilerGeneratedKernelExpectationV1 + 'static,
///     P: WorkerV3ProtectedVerifierBackendV1<K>,
///     R: WorkerV3SemanticMachineRefinementBackendV1<K>,
///     A: CompilerGeneratedRuntimeArguments<K>,
///     O: GeneratedRuntimeTypedOutputBundleV1,
/// {
///     // SAFETY: called at cooperative process startup before unrelated code.
///     let report = unsafe { run_inherited_worker_v3_current_thread_v1::<K, P, R, A, O>(
///         verifier, arguments, budget, config,
///     ) };
///     assert!(report.is_success(), "inspect error, drain, and shutdown on failure");
/// }
/// ```
///
/// A protected-only verifier cannot enter this runner:
/// ```compile_fail,E0308
/// use fe2o3_host::*;
/// fn lacks_refinement<K, P, R, A>(
///     verifier: &mut WorkerV3ProtectedVerifierAdapterV1<P>,
///     arguments: impl FnOnce() -> (A, ()),
///     budget: &GeneratedRuntimeResultBudgetV1,
///     config: ProductionWorkerV3CurrentThreadConfigV1,
/// ) where
///     K: CompilerGeneratedKernelExpectationV1 + 'static,
///     P: WorkerV3ProtectedVerifierBackendV1<K>,
///     R: WorkerV3SemanticMachineRefinementBackendV1<K>,
///     A: CompilerGeneratedRuntimeArguments<K>,
/// {
///     unsafe { run_inherited_worker_v3_current_thread_v1::<K, P, R, A, ()>(
///         verifier, arguments, budget, config,
///     ); }
/// }
/// ```
///
/// # Safety
/// Must be called before creating threads, environment/descriptor-accessing
/// signal handlers, descendants, or unrelated descriptor/environment mutation,
/// as required by `authenticate_inherited_worker_v3_application_v1`.
pub unsafe fn run_inherited_worker_v3_current_thread_v1<K, P, R, A, O>(
    verifier: &mut WorkerV3RefiningProtectedVerifierAdapterV1<P, R>,
    make_arguments: impl FnOnce() -> (A, O),
    result_budget: &GeneratedRuntimeResultBudgetV1,
    config: ProductionWorkerV3CurrentThreadConfigV1,
) -> ProductionWorkerV3CurrentThreadReportV1<
    O::Results,
    WorkerV3RefiningProtectedVerifierErrorV1<P::Error, R::Error>,
>
where
    K: CompilerGeneratedKernelExpectationV1 + 'static,
    P: WorkerV3ProtectedVerifierBackendV1<K>,
    R: WorkerV3SemanticMachineRefinementBackendV1<K>,
    A: CompilerGeneratedRuntimeArguments<K>,
    O: GeneratedRuntimeTypedOutputBundleV1,
{
    // SAFETY: the caller upholds the inherited handoff's cooperative startup contract.
    let executable =
        match unsafe { authenticate_inherited_worker_v3_application_v1::<K, _>(verifier) } {
            Ok(executable) => executable,
            Err(error) => {
                return ProductionWorkerV3CurrentThreadReportV1::failed(
                    ProductionWorkerV3CurrentThreadErrorV1::Authentication(error),
                );
            }
        };
    run_authenticated(executable, make_arguments, result_budget, config)
}

fn run_authenticated<K, A, O, VE>(
    executable: AuthenticatedWorkerV3ExecutableV1<K>,
    make_arguments: impl FnOnce() -> (A, O),
    result_budget: &GeneratedRuntimeResultBudgetV1,
    config: ProductionWorkerV3CurrentThreadConfigV1,
) -> ProductionWorkerV3CurrentThreadReportV1<O::Results, VE>
where
    K: CompilerGeneratedKernelExpectationV1 + 'static,
    A: CompilerGeneratedRuntimeArguments<K>,
    O: GeneratedRuntimeTypedOutputBundleV1,
{
    // Reject absent compiler/refinement evidence before opening KFD, not merely
    // when the later preparation callback would discover it.
    if let Err(error) = executable.require_runtime_evidence() {
        return ProductionWorkerV3CurrentThreadReportV1::failed(
            ProductionWorkerV3CurrentThreadErrorV1::Invocation(error),
        );
    }
    if let Err(error) = config.preflight() {
        return ProductionWorkerV3CurrentThreadReportV1::failed(error);
    }
    let (arguments, outputs) = make_arguments();
    if let Err(error) = config.preflight() {
        return ProductionWorkerV3CurrentThreadReportV1::failed(error);
    }
    let initialized = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
        || {
            let backend =
                KfdRuntimeBackendV1::open_worker_v3_generated_only_v1(config.device_unique_id)
                    .map_err(ProductionWorkerV3RuntimeInitializationErrorV1::Backend)?;
            RuntimeContextV1::open_with_version_journal_v1(
                backend,
                config.journal_allocation_capacity,
                config.journal_writer_capacity,
            )
            .map_err(|failure| {
                let (backend, error) = failure.into_parts();
                core::mem::forget(backend);
                ProductionWorkerV3RuntimeInitializationErrorV1::ContextRetainedUntilProcessExit(
                    error,
                )
            })
        },
        config.engine,
        config.progress,
    );
    let (mut engine, handle) = match initialized {
        Ok(owner) => owner,
        Err(error) => {
            return ProductionWorkerV3CurrentThreadReportV1::failed(
                ProductionWorkerV3CurrentThreadErrorV1::Initialization(error),
            );
        }
    };
    let mut report = ProductionWorkerV3CurrentThreadReportV1 {
        completed: None,
        error: None,
        drain: None,
        shutdown: None,
    };
    report.error = execute(
        &mut engine,
        &handle,
        executable,
        arguments,
        outputs,
        result_budget,
        &config,
        &mut report.completed,
        &mut report.drain,
    )
    .err();
    report.shutdown = Some(engine.shutdown());
    report
}

fn drive<B, F, VE>(
    engine: &mut RuntimeAsyncCurrentThreadOwnedEngineV1<B>,
    mut future: F,
    stage: ProductionWorkerV3ApplicationStageV1,
    deadline: Instant,
) -> Result<F::Output, ProductionWorkerV3CurrentThreadErrorV1<VE>>
where
    B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static,
    F: Future + Unpin,
{
    // Taking the one-shot future by value releases its reply permit before the
    // next stage. A borrowed Ready future would still hold that bounded cell.
    engine
        .drive_until_ready(Pin::new(&mut future), deadline)
        .map_err(|error| ProductionWorkerV3CurrentThreadErrorV1::Drive(stage, error))
}

#[allow(clippy::too_many_arguments)]
fn execute<K, A, O, VE>(
    engine: &mut RuntimeAsyncCurrentThreadOwnedEngineV1<KfdRuntimeBackendV1>,
    handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
    executable: AuthenticatedWorkerV3ExecutableV1<K>,
    arguments: A,
    outputs: O,
    result_budget: &GeneratedRuntimeResultBudgetV1,
    config: &ProductionWorkerV3CurrentThreadConfigV1,
    completed: &mut Option<GeneratedRuntimeCompletedBundleV1<O::Results>>,
    drain: &mut Option<RuntimeAsyncDrainReportV1>,
) -> Result<(), ProductionWorkerV3CurrentThreadErrorV1<VE>>
where
    K: CompilerGeneratedKernelExpectationV1 + 'static,
    A: CompilerGeneratedRuntimeArguments<K>,
    O: GeneratedRuntimeTypedOutputBundleV1,
{
    use ProductionWorkerV3ApplicationStageV1 as Stage;
    use ProductionWorkerV3CurrentThreadErrorV1 as Failure;
    let create = handle
        .observer()
        .enqueue_with_context(|context| {
            let [device] = context.devices() else {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
            };
            let device = device.id();
            context.create_stream(device).map(|stream| (device, stream))
        })
        .map_err(|error| Failure::Engine(Stage::CreateStream, error))?;
    let (device, stream) = drive(engine, create, Stage::CreateStream, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::CreateStream, error))?
        .map_err(|error| Failure::Context(Stage::CreateStream, error))?;
    let prepare = executable
        .prepare_generated_context_invocation_async(
            arguments,
            handle,
            device,
            config.geometry,
            config.dynamic_group_segment_bytes,
            config.timeout_milliseconds,
            config.argument_limits,
            result_budget,
        )
        .map_err(Failure::Invocation)?;
    let prepared = drive(engine, prepare, Stage::Prepare, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::Prepare, error))?
        .map_err(Failure::Preparation)?;
    // On rejection, the registry still owns the exact parked ticket's storage.
    // This one-shot owner stops immediately; it never retries or replays it.
    let reserve = handle
        .try_reserve_prepared_v1(prepared)
        .map_err(|failure| Failure::Reservation(failure.error))?;
    let reserved = drive(engine, reserve, Stage::Reserve, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::Reserve, error))?
        .map_err(|failure| Failure::Reservation(failure.error))?;
    let activate = handle
        .try_activate_generated_v1(reserved, stream)
        .map_err(|failure| Failure::Activation(failure.error))?;
    let completion = drive(engine, activate, Stage::Activate, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::Activate, error))?
        .map_err(|failure| Failure::Activation(failure.error))?;
    let typed = outputs
        .bind_completion_bundle_v1(completion)
        .map_err(|failure| Failure::Binding(failure.error))?;
    *completed = Some(
        drive(engine, typed, Stage::Complete, config.deadline)?
            .map_err(|failure| Failure::Completion(failure.error))?,
    );
    let destroy = handle
        .observer()
        .enqueue_with_context(move |context| context.destroy_stream(stream))
        .map_err(|error| Failure::Engine(Stage::DestroyStream, error))?;
    drive(engine, destroy, Stage::DestroyStream, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::DestroyStream, error))?
        .map_err(|error| Failure::Context(Stage::DestroyStream, error))?;
    // Drain permanently closes command admission, so stream destruction is first.
    let draining = handle
        .begin_drain(config.drain_tick_budget)
        .map_err(Failure::Drain)?;
    let report = drive(engine, draining, Stage::Drain, config.deadline)?
        .map_err(|error| Failure::Engine(Stage::Drain, error))?;
    let quiescent = report.outcome == RuntimeAsyncDrainOutcomeV1::Quiescent;
    *drain = Some(report);
    if quiescent {
        Ok(())
    } else {
        Err(Failure::DrainIncomplete)
    }
}
