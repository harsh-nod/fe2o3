//! One pre-admitted owned host capture at the owner's private drain boundary.

use super::*;
use crate::{RuntimeHostCaptureErrorV1, RuntimeHostCaptureSourceV1};
use drain_capture_storage::{CaptureReservationV1, RuntimeAsyncCapturedBytesV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncDrainCaptureAdmissionErrorV1 {
    Disabled,
    InvalidTickBudget,
    ReentrantCall,
    ForeignContext,
    InvalidDestination,
    ReplyCapacity,
    StorageCapacity,
    AdmissionClosed,
}

impl fmt::Display for RuntimeAsyncDrainCaptureAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime drain capture admission: {self:?}")
    }
}

impl Error for RuntimeAsyncDrainCaptureAdmissionErrorV1 {}

/// Rejected admission returns the exact source and caller-owned destination.
pub struct RuntimeAsyncDrainCaptureAdmissionFailureV1 {
    error: RuntimeAsyncDrainCaptureAdmissionErrorV1,
    source: RuntimeHostCaptureSourceV1,
    destination: Box<[u8]>,
}

impl RuntimeAsyncDrainCaptureAdmissionFailureV1 {
    pub const fn error(&self) -> RuntimeAsyncDrainCaptureAdmissionErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        RuntimeAsyncDrainCaptureAdmissionErrorV1,
        RuntimeHostCaptureSourceV1,
        Box<[u8]>,
    ) {
        (self.error, self.source, self.destination)
    }
}

impl fmt::Debug for RuntimeAsyncDrainCaptureAdmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAsyncDrainCaptureAdmissionFailureV1")
            .field("error", &self.error)
            .field("byte_len", &self.destination.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for RuntimeAsyncDrainCaptureAdmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for RuntimeAsyncDrainCaptureAdmissionFailureV1 {}

/// Drain classification and one owned capture; neither grants execution authority.
#[derive(Debug)]
pub struct RuntimeAsyncDrainCaptureReportV1 {
    pub drain: RuntimeAsyncDrainReportV1,
    pub capture: Result<RuntimeAsyncCapturedBytesV1, RuntimeHostCaptureErrorV1>,
}

pub(super) struct PendingCaptureV1 {
    source: RuntimeHostCaptureSourceV1,
    destination: CaptureReservationV1,
    reply: owned::Reply<RuntimeAsyncDrainCaptureReportV1>,
}

impl PendingCaptureV1 {
    pub(super) fn retain(self) -> CaptureRequestV1 {
        CaptureRequestV1 {
            source: self.source,
            destination: Some(self.destination.retain()),
            reply: self.reply,
        }
    }

    fn reject(self) -> RuntimeAsyncDrainCaptureAdmissionFailureV1 {
        RuntimeAsyncDrainCaptureAdmissionFailureV1 {
            error: RuntimeAsyncDrainCaptureAdmissionErrorV1::AdmissionClosed,
            source: self.source,
            destination: self.destination.into_unadmitted_destination(),
        }
    }
}

pub(super) struct CaptureRequestV1 {
    source: RuntimeHostCaptureSourceV1,
    destination: Option<RuntimeAsyncCapturedBytesV1>,
    reply: owned::Reply<RuntimeAsyncDrainCaptureReportV1>,
}

impl CaptureRequestV1 {
    pub(super) fn complete<B: RuntimeBackendV1>(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        drain: RuntimeAsyncDrainReportV1,
        quiescence: drain::DrainQuiescenceV1,
    ) {
        let Some(mut destination) = self.destination.take() else {
            return;
        };
        let outcome =
            context.capture_host_drain_v1(&self.source, destination.as_bytes_mut(), quiescence);
        if context.is_terminal() {
            drop(destination);
            self.reply
                .complete(Err(RuntimeAsyncEngineCallErrorV1::CaptureFailed(
                    outcome
                        .err()
                        .unwrap_or(RuntimeHostCaptureErrorV1::ContextTerminal),
                )));
            return;
        }
        let capture = outcome.map(|()| destination);
        self.reply
            .complete(Ok(RuntimeAsyncDrainCaptureReportV1 { drain, capture }));
    }

    pub(super) fn incomplete(&mut self, drain: RuntimeAsyncDrainReportV1) {
        drop(self.destination.take());
        self.reply.complete(Ok(RuntimeAsyncDrainCaptureReportV1 {
            drain,
            capture: Err(RuntimeHostCaptureErrorV1::CaptureIncomplete),
        }));
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    /// Closes admission and captures one pre-registered host range after drain.
    ///
    /// The destination is already caller allocated. Admission reserves its exact
    /// slice-byte and reply-cell capacity and preallocates result metadata before cutoff.
    /// Rejection returns the original storage without closing admission. Dropping
    /// an accepted future does not withdraw the drain or its retained custody.
    #[allow(clippy::result_large_err)]
    pub fn begin_drain_with_capture(
        &self,
        max_ticks: usize,
        source: RuntimeHostCaptureSourceV1,
        destination: Box<[u8]>,
    ) -> Result<
        RuntimeAsyncCommandFutureV1<RuntimeAsyncDrainCaptureReportV1>,
        RuntimeAsyncDrainCaptureAdmissionFailureV1,
    > {
        use RuntimeAsyncDrainCaptureAdmissionErrorV1 as AdmissionError;
        let invalid = if self.observer.is_worker_thread() {
            Some(AdmissionError::ReentrantCall)
        } else if max_ticks == 0 || max_ticks > MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1 {
            Some(AdmissionError::InvalidTickBudget)
        } else if self.observer.capture_budget.is_none() {
            Some(AdmissionError::Disabled)
        } else if !source.belongs_to_context(self.observer.context_generation) {
            Some(AdmissionError::ForeignContext)
        } else if destination.is_empty() || destination.len() != source.byte_len() {
            Some(AdmissionError::InvalidDestination)
        } else {
            None
        };
        if let Some(error) = invalid {
            return Err(RuntimeAsyncDrainCaptureAdmissionFailureV1 {
                error,
                source,
                destination,
            });
        }
        let budget = self
            .observer
            .capture_budget
            .as_ref()
            .expect("validated capture budget");
        let destination = match budget.reserve(destination) {
            Ok(destination) => destination,
            Err((_, destination)) => {
                return Err(RuntimeAsyncDrainCaptureAdmissionFailureV1 {
                    error: AdmissionError::StorageCapacity,
                    source,
                    destination,
                });
            }
        };
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(_) => {
                return Err(RuntimeAsyncDrainCaptureAdmissionFailureV1 {
                    error: AdmissionError::ReplyCapacity,
                    source,
                    destination: destination.into_unadmitted_destination(),
                });
            }
        };
        let pending = PendingCaptureV1 {
            source,
            destination,
            reply,
        };
        self.observer
            .admission
            .install_capture(max_ticks, pending)
            .map_err(PendingCaptureV1::reject)?;
        Ok(future)
    }
}
