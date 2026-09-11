//! Admission closure and cooperative observation drain. No native release authority.

use super::*;
use crate::RuntimeSubmissionIdV1;
use std::collections::VecDeque;

pub const MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncDrainErrorV1 {
    InvalidTickBudget,
    AdmissionClosed,
    ReentrantCall,
}

impl fmt::Display for RuntimeAsyncDrainErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime async drain: {self:?}")
    }
}
impl Error for RuntimeAsyncDrainErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncDrainOutcomeV1 {
    /// All accepted commands and observers finished; retained submissions are quiescent.
    /// This includes failed results and is not a native cleanup receipt.
    Quiescent,
    /// The cooperative tick budget expired. Unresolved custody is quarantined.
    BudgetExhausted,
}

/// Historical observations of retained context submissions, not all operations
/// ever accepted. Already-retired graph nodes and rejected launches are absent.
pub type RuntimeAsyncDrainCountsV1 = crate::RuntimeStreamObservationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAsyncDrainReportV1 {
    pub outcome: RuntimeAsyncDrainOutcomeV1,
    pub ticks: usize,
    pub retained_submissions: RuntimeAsyncDrainCountsV1,
    pub queued_commands_exhausted: bool,
    /// Drivers still requiring progress. Never-adopted parked preparations are
    /// excluded; their storage remains retained until discard or owned shutdown.
    pub operations_remaining: usize,
    pub graph_active: bool,
}

struct AdmissionState {
    closed: bool,
    drain: Option<DrainRequest>,
}

pub(super) struct AdmissionV1(Mutex<AdmissionState>);

impl AdmissionV1 {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(AdmissionState {
            closed: false,
            drain: None,
        })))
    }

    pub(super) fn try_send<B: RuntimeBackendV1>(
        &self,
        sender: &SyncSender<RuntimeAsyncEngineCommandV1<B>>,
        command: RuntimeAsyncEngineCommandV1<B>,
    ) -> Result<(), TrySendError<RuntimeAsyncEngineCommandV1<B>>> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let result = if state.closed {
            Err(TrySendError::Disconnected(command))
        } else {
            sender.try_send(command)
        };
        // In particular, rejected payload destructors may reenter admission.
        drop(state);
        result
    }

    pub(super) fn close(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn install_capture(
        &self,
        max_ticks: usize,
        capture: drain_capture::PendingCaptureV1,
    ) -> Result<(), drain_capture::PendingCaptureV1> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            drop(state);
            return Err(capture);
        }
        let capture = capture.retain();
        state.drain = Some(DrainRequest {
            reply: DrainReplyV1::Capture(capture),
            remaining: max_ticks,
            ticks: 0,
            native: None,
            quiescent: false,
        });
        state.closed = true;
        Ok(())
    }

    fn take_drain(&self) -> Option<DrainRequest> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain
            .take()
    }

    pub(super) fn close_and_take(&self) -> Option<DrainRequest> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        state.drain.take()
    }

    fn stop(&self) {
        let pending = self.close_and_take();
        drop(pending);
    }
}

pub(super) struct AdmissionWorkerGuardV1(pub(super) Arc<AdmissionV1>);
impl Drop for AdmissionWorkerGuardV1 {
    fn drop(&mut self) {
        self.0.stop();
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncEngineHandleV1<B> {
    pub(super) fn try_send_command(
        &self,
        command: RuntimeAsyncEngineCommandV1<B>,
    ) -> Result<(), TrySendError<RuntimeAsyncEngineCommandV1<B>>> {
        self.admission.try_send(&self.sender, command)
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    /// Permanently closes admission across every cloned handle, then observes
    /// the finite accepted workload without cancelling it. The lifecycle slot
    /// is independent of command-channel capacity. Further commands cannot be
    /// admitted; local validation/capacity errors may precede `EngineStopped`.
    /// Dropping this future does not withdraw the drain.
    ///
    /// A tick budget is not a wall-clock deadline. Backend calls, callbacks and
    /// executor wakers must return for the owner to check it. Expiry quarantines
    /// unresolved custody, never fabricates completion. Owner interruption or
    /// panic resolves as `EngineStopped`, not as a quiescent drain report.
    ///
    /// After awaiting this future, join the owner with `shutdown` (or recover
    /// a transferable context) and inspect cleanup separately. Existing Stop
    /// APIs still interrupt an unfinished drain.
    pub fn begin_drain(
        &self,
        max_ticks: usize,
    ) -> Result<RuntimeAsyncCommandFutureV1<RuntimeAsyncDrainReportV1>, RuntimeAsyncDrainErrorV1>
    {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncDrainErrorV1::ReentrantCall);
        }
        if max_ticks == 0 || max_ticks > MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1 {
            return Err(RuntimeAsyncDrainErrorV1::InvalidTickBudget);
        }
        let (reply, future) = owned::Reply::pair();
        let mut state = self
            .observer
            .admission
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.closed {
            drop(state);
            return Err(RuntimeAsyncDrainErrorV1::AdmissionClosed);
        }
        state.closed = true;
        state.drain = Some(DrainRequest {
            reply: DrainReplyV1::Report(reply),
            remaining: max_ticks,
            ticks: 0,
            native: None,
            quiescent: false,
        });
        Ok(future)
    }
}

struct NativeDrain {
    pending: VecDeque<RuntimeSubmissionIdV1>,
    streams: Vec<RuntimeStreamIdV1>,
    next_stream: usize,
}

/// Minted only by the owner's complete accepted-prefix quiescence decision.
pub(crate) struct DrainQuiescenceV1 {
    _private: (),
}

enum DrainReplyV1 {
    Report(owned::Reply<RuntimeAsyncDrainReportV1>),
    Capture(drain_capture::CaptureRequestV1),
}

pub(super) struct DrainRequest {
    reply: DrainReplyV1,
    remaining: usize,
    ticks: usize,
    native: Option<NativeDrain>,
    quiescent: bool,
}

impl DrainRequest {
    pub(super) fn is_quiescent(&self) -> bool {
        self.quiescent
    }
    pub(super) fn take(admission: &AdmissionV1) -> Option<Self> {
        admission.take_drain()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick<B: RuntimeBackendV1 + 'static>(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        queue_exhausted: bool,
        operations: usize,
        graph_active: bool,
        waiters_empty: bool,
        config: RuntimeAsyncEngineConfigV1,
        progress: &RuntimeAsyncProgressModeV1<B>,
    ) -> bool {
        self.ticks += 1;
        self.remaining -= 1;
        if queue_exhausted && !graph_active {
            if self.native.is_none() {
                match context.snapshot_async_drain_v1() {
                    Ok((pending, streams)) => {
                        self.native = Some(NativeDrain {
                            pending,
                            streams,
                            next_stream: 0,
                        });
                    }
                    Err(_) => self.remaining = 0,
                }
            }
            if let Some(native) = self.native.as_mut() {
                for _ in 0..config.polls_per_tick.min(native.pending.len()) {
                    let id = native.pending.pop_front().expect("bounded drain roster");
                    let observed = match context.poll_async_drain_v1(id) {
                        Ok(status) => status.is_terminal(),
                        Err(RuntimeErrorV1::BackendQuiescent(_)) => true,
                        Err(_) => false,
                    };
                    if !observed {
                        native.pending.push_back(id);
                    }
                    if context.is_terminal() {
                        return true;
                    }
                }
                let flushes = if native.pending.is_empty() {
                    0
                } else {
                    progress.config.flushes_per_tick.min(native.streams.len())
                };
                for _ in 0..flushes {
                    let stream = native.streams[native.next_stream];
                    native.next_stream = (native.next_stream + 1) % native.streams.len();
                    let _ = (progress.flush_stream)(context, stream);
                    if context.is_terminal() {
                        return true;
                    }
                }
                if native.pending.is_empty() && operations == 0 && waiters_empty {
                    let counts = context.async_drain_counts_v1();
                    if counts.pending == 0 {
                        let report = RuntimeAsyncDrainReportV1 {
                            outcome: RuntimeAsyncDrainOutcomeV1::Quiescent,
                            ticks: self.ticks,
                            retained_submissions: counts,
                            queued_commands_exhausted: true,
                            operations_remaining: 0,
                            graph_active: false,
                        };
                        match &mut self.reply {
                            DrainReplyV1::Report(reply) => reply.complete(Ok(report)),
                            DrainReplyV1::Capture(capture) => capture.complete(
                                context,
                                report,
                                DrainQuiescenceV1 { _private: () },
                            ),
                        }
                        self.quiescent = !context.is_terminal();
                        return true;
                    }
                    // Standalone operations can create submissions after the initial snapshot.
                    self.native = None;
                }
            }
        }
        if self.remaining == 0 {
            let counts = context.async_drain_counts_v1();
            context.quarantine_after_async_command_panic_v1();
            self.finish(
                RuntimeAsyncDrainOutcomeV1::BudgetExhausted,
                counts,
                queue_exhausted,
                operations,
                graph_active,
            );
            return true;
        }
        false
    }

    fn finish(
        &mut self,
        outcome: RuntimeAsyncDrainOutcomeV1,
        retained_submissions: RuntimeAsyncDrainCountsV1,
        queued_commands_exhausted: bool,
        operations_remaining: usize,
        graph_active: bool,
    ) {
        self.quiescent = outcome == RuntimeAsyncDrainOutcomeV1::Quiescent;
        let report = RuntimeAsyncDrainReportV1 {
            outcome,
            ticks: self.ticks,
            retained_submissions,
            queued_commands_exhausted,
            operations_remaining,
            graph_active,
        };
        match &mut self.reply {
            DrainReplyV1::Report(reply) => reply.complete(Ok(report)),
            DrainReplyV1::Capture(capture) => capture.incomplete(report),
        }
    }
}
