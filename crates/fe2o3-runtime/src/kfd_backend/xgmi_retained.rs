//! One exact roster under explicit ordinary-lifetime native custody.

#![forbid(unsafe_code)]

use super::xgmi_native_custody::{NativeCustody, NativeXgmiStateV1};
use super::*;
use fe2o3_kfd::{
    Gfx942NativeXgmiSdmaOwnedRetainedPairV1, Gfx942SdmaErrorV1,
    Gfx942XgmiOwnedRetainedPairFailureV1, Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    Gfx942XgmiRetainedPairWaitFailureV1,
};

pub(super) enum BatchPhase<R, T, S, W> {
    Prepared(Vec<R>),
    Pending(Vec<T>),
    Ready,
    SubmitFailure(S),
    WaitFailure(W),
}

type Phase = BatchPhase<
    Gfx942XgmiSdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1,
    Gfx942XgmiBatchSubmissionFailureV1,
    Gfx942XgmiRetainedPairWaitFailureV1,
>;

impl<R, T, S, W> BatchPhase<R, T, S, W> {
    fn ready_to_finish(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

fn publish_once<R, T, S, W>(
    phase: BatchPhase<R, T, S, W>,
    publish: impl FnOnce(Vec<R>) -> Result<Vec<T>, S>,
) -> BatchPhase<R, T, S, W> {
    match phase {
        BatchPhase::Prepared(requests) => match publish(requests) {
            Ok(tickets) => BatchPhase::Pending(tickets),
            Err(failure) => BatchPhase::SubmitFailure(failure),
        },
        other => other,
    }
}

trait Work {
    type Request;
    type Ticket;
    type SubmitFailure;
    type WaitFailure;
    type Completed;
    fn submit(
        &mut self,
        requests: Vec<Self::Request>,
    ) -> Result<Vec<Self::Ticket>, Self::SubmitFailure>;
    fn wait(
        &mut self,
        tickets: Vec<Self::Ticket>,
        deadline: Instant,
    ) -> Result<Self::Completed, Self::WaitFailure>;
}
type WorkPhase<W> = BatchPhase<
    <W as Work>::Request,
    <W as Work>::Ticket,
    <W as Work>::SubmitFailure,
    <W as Work>::WaitFailure,
>;
enum Advanced<P, C, E> {
    Unchanged(P),
    Waited(Result<C, E>),
}
type WorkAdvance<W> = Advanced<WorkPhase<W>, <W as Work>::Completed, <W as Work>::WaitFailure>;

fn advance<W: Work>(phase: WorkPhase<W>, deadline: Instant, work: &mut W) -> WorkAdvance<W> {
    let phase = publish_once(phase, |requests| work.submit(requests));
    match phase {
        BatchPhase::Pending(tickets) => Advanced::Waited(work.wait(tickets, deadline)),
        other => Advanced::Unchanged(other),
    }
}

struct NativeWork<'a> {
    owner: &'a mut Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
    backend: &'a mut KfdNativeXgmiRuntimeBackendV1,
    ids: &'a [u64],
    direction: usize,
}
impl Work for NativeWork<'_> {
    type Request = Gfx942XgmiSdmaCopyRequestV1;
    type Ticket = Gfx942SdmaCopyTicketV1;
    type SubmitFailure = Gfx942XgmiBatchSubmissionFailureV1;
    type WaitFailure = Gfx942XgmiRetainedPairWaitFailureV1;
    type Completed = fe2o3_kfd::Gfx942XgmiRetainedPairCompletedBatchV1;
    fn submit(
        &mut self,
        requests: Vec<Self::Request>,
    ) -> Result<Vec<Self::Ticket>, Self::SubmitFailure> {
        self.owner.submit_batch(requests).inspect(|tickets| {
            self.backend.install_batch_tickets(
                self.ids,
                tickets,
                xgmi_batch::Admission {
                    direction: self.direction,
                    published: false,
                },
            );
        })
    }
    fn wait(
        &mut self,
        tickets: Vec<Self::Ticket>,
        deadline: Instant,
    ) -> Result<Self::Completed, Self::WaitFailure> {
        self.owner.wait_batch_until(tickets, deadline)
    }
}

// Owner first: occupied native Drop aborts before the opposite queue or mappings.
pub(super) struct Retained {
    pub(super) owner: Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
    opposite: Option<Gfx942NativeXgmiSdmaQueueV1>,
    pub(super) direction: usize,
    ids: Vec<u64>,
    phase: Phase,
    deadline: Instant,
}

pub(super) struct Failed {
    pub(super) owner: Gfx942XgmiOwnedRetainedPairFailureV1,
    _opposite: Option<Gfx942NativeXgmiSdmaQueueV1>,
    pub(super) direction: usize,
    _ids: Vec<u64>,
    _phase: Phase,
}

impl Retained {
    pub(super) fn queue_count(&self) -> usize {
        1 + usize::from(self.opposite.is_some())
    }
}
impl Failed {
    pub(super) fn queue_count(&self) -> usize {
        1 + usize::from(self._opposite.is_some())
    }
}

// The opposite queue must not drop if consuming begin/finish unwinds.
struct TransferGuard<T>(Option<T>);
impl<T> TransferGuard<T> {
    fn new(value: T) -> Self {
        Self(Some(value))
    }
    fn take(mut self) -> T {
        self.0.take().unwrap_or_else(|| std::process::abort())
    }
}
impl<T> Drop for TransferGuard<T> {
    fn drop(&mut self) {
        if self.0.is_some() {
            std::process::abort();
        }
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    pub(crate) fn begin_retained_peer_batch_v1(
        &mut self,
        requested: &[u64],
        deadline: Instant,
        environment: Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let (admission, ids, requests) = self.admit_retained_batch(requested)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let requests = self.prepare_batch_requests(admission, &ids, requests)?;
            let direction = admission.direction;
            let full = match self.native.state.take() {
                Some(NativeXgmiStateV1::Full(full)) => full,
                _ => std::process::abort(),
            };
            let (queue, source, destination, opposite) = full.into_direction(direction);
            let opposite = TransferGuard::new(opposite);
            let result = Gfx942NativeXgmiSdmaOwnedRetainedPairV1::begin(
                queue,
                source,
                destination,
                environment,
            );
            let phase = Phase::Prepared(requests);
            self.native.state = Some(match result {
                Ok(owner) => NativeXgmiStateV1::Retained(Retained {
                    owner,
                    opposite: opposite.take(),
                    direction,
                    ids,
                    phase,
                    deadline,
                }),
                Err(owner) => {
                    self.terminal = true;
                    NativeXgmiStateV1::Failed(Failed {
                        owner,
                        _opposite: opposite.take(),
                        direction,
                        _ids: ids,
                        _phase: phase,
                    })
                }
            });
            match self.native.state.as_ref() {
                Some(NativeXgmiStateV1::Failed(failed)) => {
                    let detail = format!("retained XGMI entry: {}", failed.owner.error());
                    Err(self.terminal_error(detail))
                }
                _ => Ok(()),
            }
        }));
        xgmi_batch::finish_native_attempt(result, &mut self.terminal)
    }

    pub(crate) fn wait_retained_peer_batch_v1(
        &mut self,
    ) -> Result<
        crate::RuntimeNativeRetainedPeerCopyPollV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_healthy_xgmi_v1()?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut retained = match self.native.state.take() {
                Some(NativeXgmiStateV1::Retained(retained)) => retained,
                other => {
                    self.native.state = other;
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "no retained XGMI batch",
                    ));
                }
            };
            let phase = std::mem::replace(&mut retained.phase, Phase::Ready);
            let advanced = advance(
                phase,
                retained.deadline,
                &mut NativeWork {
                    owner: &mut retained.owner,
                    backend: self,
                    ids: &retained.ids,
                    direction: retained.direction,
                },
            );
            let phase = match advanced {
                Advanced::Unchanged(phase) => phase,
                Advanced::Waited(result) => match result {
                    Ok(completed) => {
                        if completed.len() != retained.ids.len() {
                            std::process::abort();
                        }
                        for (id, copy) in retained.ids.iter().copied().zip(completed.into_copies())
                        {
                            let active = self
                                .active
                                .get(&id)
                                .unwrap_or_else(|| std::process::abort());
                            if active.byte_len != copy.copy_bytes() {
                                std::process::abort();
                            }
                            let (source, destination) = copy.into_mappings();
                            self.restore_batch_pair(id, source, destination, false);
                        }
                        Phase::Ready
                    }
                    Err(failure)
                        if matches!(failure.error(), Gfx942SdmaErrorV1::Timeout)
                            && !retained.owner.is_terminal() =>
                    {
                        // Timeout is semantic admission, not proof of the failure's
                        // ownership variant. A mismatch retains the complete error.
                        match failure.try_into_retained_tickets() {
                            Ok(tickets) => Phase::Pending(tickets),
                            Err(failure) => Phase::WaitFailure(failure),
                        }
                    }
                    Err(failure) => Phase::WaitFailure(failure),
                },
            };
            let failed = matches!(phase, Phase::SubmitFailure(_) | Phase::WaitFailure(_));
            retained.phase = phase;
            let ready = retained.phase.ready_to_finish();
            if failed {
                self.terminal = true;
                retained.owner.quarantine();
            }
            self.native.state = Some(NativeXgmiStateV1::Retained(retained));
            if failed {
                let detail = match &self.native.state {
                    Some(NativeXgmiStateV1::Retained(retained)) => match &retained.phase {
                        Phase::SubmitFailure(failure) => format!("retained XGMI submit: {failure}"),
                        Phase::WaitFailure(failure) => format!("retained XGMI wait: {failure}"),
                        _ => std::process::abort(),
                    },
                    _ => std::process::abort(),
                };
                Err(self.terminal_error(detail))
            } else if ready {
                Ok(crate::RuntimeNativeRetainedPeerCopyPollV1::ReadyToFinish)
            } else {
                Ok(crate::RuntimeNativeRetainedPeerCopyPollV1::Pending)
            }
        }));
        xgmi_batch::finish_native_attempt(result, &mut self.terminal)
    }

    pub(crate) fn finish_retained_peer_batch_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_healthy_xgmi_v1()?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let retained = match self.native.state.take() {
                Some(NativeXgmiStateV1::Retained(retained)) => retained,
                other => {
                    self.native.state = other;
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "no retained XGMI batch",
                    ));
                }
            };
            if !retained.phase.ready_to_finish() {
                self.native.state = Some(NativeXgmiStateV1::Retained(retained));
                self.abandon_retained_peer_batch_v1();
                return Err(self.terminal_error("retained XGMI finish requires a completed roster"));
            }
            let Retained {
                owner,
                opposite,
                direction,
                ids,
                phase,
                ..
            } = retained;
            let opposite = TransferGuard::new(opposite);
            match owner.finish() {
                Ok((queue, source, destination)) => {
                    self.native.state =
                        Some(NativeXgmiStateV1::Full(NativeCustody::from_direction(
                            queue,
                            source,
                            destination,
                            opposite.take(),
                            direction,
                        )));
                    self.commit_batch_status(&ids, BackendPollV1::Succeeded);
                    Ok(())
                }
                Err(owner) => {
                    self.terminal = true;
                    self.native.state = Some(NativeXgmiStateV1::Failed(Failed {
                        owner,
                        _opposite: opposite.take(),
                        direction,
                        _ids: ids,
                        _phase: phase,
                    }));
                    let detail = match &self.native.state {
                        Some(NativeXgmiStateV1::Failed(failed)) => {
                            format!("retained XGMI finish: {}", failed.owner.error())
                        }
                        _ => std::process::abort(),
                    };
                    Err(self.terminal_error(detail))
                }
            }
        }));
        xgmi_batch::finish_native_attempt(result, &mut self.terminal)
    }

    pub(crate) fn abandon_retained_peer_batch_v1(&mut self) {
        self.terminal = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match &mut self.native.state {
                Some(NativeXgmiStateV1::Retained(retained)) => retained.owner.quarantine(),
                Some(NativeXgmiStateV1::Full(full)) => {
                    let (sessions, queues) = full.parts_mut();
                    for (direction, queue) in queues.iter_mut().enumerate() {
                        if let Some(queue) = queue {
                            let (source, destination) = Self::session_pair(sessions, direction);
                            queue.quarantine_batch_v1(source, destination);
                        }
                    }
                }
                Some(NativeXgmiStateV1::Failed(_)) | None => {}
            }
        }));
        xgmi_batch::finish_native_attempt(result, &mut self.terminal);
    }
}

#[cfg(test)]
mod tests;
