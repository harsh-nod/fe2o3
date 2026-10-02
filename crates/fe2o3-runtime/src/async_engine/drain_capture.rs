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
    InvalidSources,
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

/// Rejected group admission returns the exact ordered roster and destination.
pub struct RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {
    error: RuntimeAsyncDrainCaptureAdmissionErrorV1,
    sources: Box<[RuntimeHostCaptureSourceV1]>,
    destination: Box<[u8]>,
}

impl RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {
    pub const fn error(&self) -> RuntimeAsyncDrainCaptureAdmissionErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        RuntimeAsyncDrainCaptureAdmissionErrorV1,
        Box<[RuntimeHostCaptureSourceV1]>,
        Box<[u8]>,
    ) {
        (self.error, self.sources, self.destination)
    }
}

impl fmt::Debug for RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAsyncDrainCaptureGroupAdmissionFailureV1")
            .field("error", &self.error)
            .field("range_count", &self.sources.len())
            .field("byte_len", &self.destination.len())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {}

/// Drain classification and one owned capture; neither grants execution authority.
#[derive(Debug)]
pub struct RuntimeAsyncDrainCaptureReportV1 {
    pub drain: RuntimeAsyncDrainReportV1,
    pub capture: Result<RuntimeAsyncCapturedBytesV1, RuntimeHostCaptureErrorV1>,
}

pub(super) struct PendingCaptureV1 {
    sources: CaptureSourcesV1,
    destination: CaptureReservationV1,
    reply: owned::Reply<RuntimeAsyncDrainCaptureReportV1>,
}

impl PendingCaptureV1 {
    pub(super) fn retain(self) -> CaptureRequestV1 {
        CaptureRequestV1 {
            sources: self.sources,
            destination: Some(self.destination.retain()),
            reply: self.reply,
        }
    }

    fn reject(self) -> CaptureAdmissionFailureV1 {
        CaptureAdmissionFailureV1 {
            error: RuntimeAsyncDrainCaptureAdmissionErrorV1::AdmissionClosed,
            sources: self.sources,
            destination: self.destination.into_unadmitted_destination(),
        }
    }
}

pub(super) struct CaptureRequestV1 {
    sources: CaptureSourcesV1,
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
        let outcome = match &self.sources {
            CaptureSourcesV1::Single(source) => {
                context.capture_host_drain_v1(source, destination.as_bytes_mut(), quiescence)
            }
            CaptureSourcesV1::Group(sources) => {
                context.capture_host_drain_group_v1(sources, destination.as_bytes_mut(), quiescence)
            }
        };
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

enum CaptureSourcesV1 {
    Single(RuntimeHostCaptureSourceV1),
    Group(Box<[RuntimeHostCaptureSourceV1]>),
}

impl CaptureSourcesV1 {
    fn as_slice(&self) -> &[RuntimeHostCaptureSourceV1] {
        match self {
            Self::Single(source) => std::slice::from_ref(source),
            Self::Group(sources) => sources,
        }
    }

    fn byte_limit(&self) -> usize {
        match self {
            Self::Single(_) => MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1,
            Self::Group(_) => MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1,
        }
    }
}

struct CaptureAdmissionFailureV1 {
    error: RuntimeAsyncDrainCaptureAdmissionErrorV1,
    sources: CaptureSourcesV1,
    destination: Box<[u8]>,
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
        self.begin_capture(max_ticks, CaptureSourcesV1::Single(source), destination)
            .map_err(|failure| {
                let CaptureSourcesV1::Single(source) = failure.sources else {
                    unreachable!("single-range admission preserves its source");
                };
                RuntimeAsyncDrainCaptureAdmissionFailureV1 {
                    error: failure.error,
                    source,
                    destination: failure.destination,
                }
            })
    }

    /// Captures a bounded ordered roster into one concatenated owned result.
    ///
    /// Each positive registered range occupies exactly its byte length, in
    /// roster order. The boxed roster and destination must already be allocated;
    /// their aggregate length is checked before admission closes. All sources
    /// are revalidated after drain and before the first backend read. Any capture
    /// error discards the entire result, including an already-copied prefix.
    /// Quiescence is not operation success: inspect the accepted operations and
    /// drain counts separately. This does not synchronize concurrently mutating
    /// external host writers or grant native cleanup authority.
    #[allow(clippy::result_large_err)]
    pub fn begin_drain_with_capture_group(
        &self,
        max_ticks: usize,
        sources: Box<[RuntimeHostCaptureSourceV1]>,
        destination: Box<[u8]>,
    ) -> Result<
        RuntimeAsyncCommandFutureV1<RuntimeAsyncDrainCaptureReportV1>,
        RuntimeAsyncDrainCaptureGroupAdmissionFailureV1,
    > {
        self.begin_capture(max_ticks, CaptureSourcesV1::Group(sources), destination)
            .map_err(|failure| {
                let CaptureSourcesV1::Group(sources) = failure.sources else {
                    unreachable!("group admission preserves its source roster");
                };
                RuntimeAsyncDrainCaptureGroupAdmissionFailureV1 {
                    error: failure.error,
                    sources,
                    destination: failure.destination,
                }
            })
    }

    #[allow(clippy::result_large_err)]
    fn begin_capture(
        &self,
        max_ticks: usize,
        sources: CaptureSourcesV1,
        destination: Box<[u8]>,
    ) -> Result<
        RuntimeAsyncCommandFutureV1<RuntimeAsyncDrainCaptureReportV1>,
        CaptureAdmissionFailureV1,
    > {
        use RuntimeAsyncDrainCaptureAdmissionErrorV1 as AdmissionError;
        let roster = sources.as_slice();
        let invalid = if self.observer.rejects_async_enqueue() {
            Some(AdmissionError::ReentrantCall)
        } else if max_ticks == 0 || max_ticks > MAX_RUNTIME_ASYNC_DRAIN_TICKS_V1 {
            Some(AdmissionError::InvalidTickBudget)
        } else if self.observer.capture_budget.is_none() {
            Some(AdmissionError::Disabled)
        } else if roster.is_empty() || roster.len() > MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_RANGES_V1 {
            Some(AdmissionError::InvalidSources)
        } else if roster
            .iter()
            .any(|source| !source.belongs_to_context(self.observer.context_generation))
        {
            Some(AdmissionError::ForeignContext)
        } else if destination.is_empty()
            || roster
                .iter()
                .try_fold(0usize, |sum, source| sum.checked_add(source.byte_len()))
                != Some(destination.len())
        {
            Some(AdmissionError::InvalidDestination)
        } else if destination.len() > sources.byte_limit() {
            Some(AdmissionError::StorageCapacity)
        } else {
            None
        };
        if let Some(error) = invalid {
            return Err(CaptureAdmissionFailureV1 {
                error,
                sources,
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
                return Err(CaptureAdmissionFailureV1 {
                    error: AdmissionError::StorageCapacity,
                    sources,
                    destination,
                });
            }
        };
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(_) => {
                return Err(CaptureAdmissionFailureV1 {
                    error: AdmissionError::ReplyCapacity,
                    sources,
                    destination: destination.into_unadmitted_destination(),
                });
            }
        };
        let pending = PendingCaptureV1 {
            sources,
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
