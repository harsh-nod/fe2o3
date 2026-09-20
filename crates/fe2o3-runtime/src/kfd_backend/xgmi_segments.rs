//! Serial reuse of one mapping pair inside a full-currentness scope.

#![forbid(unsafe_code)]

use super::*;
use crate::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsBackendV1};
use fe2o3_kfd::{Gfx942NativeXgmiSdmaBatchV1, Gfx942SdmaErrorV1, Gfx942XgmiBatchWaitFailureV1};
use fe2o3_runtime_model::{
    OrderedPeerCopyActionV1 as Action, OrderedPeerCopyCursorV1,
    validate_ordered_peer_copy_segments_v1,
};

pub(super) struct Sequence {
    segments: Vec<RuntimePeerCopySegmentV1>,
    cursor: OrderedPeerCopyCursorV1,
}

impl Sequence {
    pub(super) fn ever_published(&self) -> bool {
        self.cursor.ever_published()
    }

    pub(super) fn cancel_before_publication(&mut self) {
        step(&mut self.cursor, Action::Cancel);
    }
}

enum Custody<P, T> {
    Pair(P),
    Ticket(T),
}

enum Observation<P, T, E> {
    Completed(P),
    Pending(T),
    Failed {
        error: E,
        custody: Custody<P, T>,
        terminal: bool,
    },
}

type ScopedObservation<S> =
    Observation<<S as Scope>::Pair, <S as Scope>::Ticket, <S as Scope>::Error>;

trait Scope: Sized {
    type Pair;
    type Ticket;
    type Error;

    fn submit(
        &mut self,
        pair: Self::Pair,
        segment: RuntimePeerCopySegmentV1,
    ) -> Result<Self::Ticket, ScopedObservation<Self>>;
    fn wait(
        &mut self,
        ticket: Self::Ticket,
        bytes: u64,
        deadline: Instant,
    ) -> Observation<Self::Pair, Self::Ticket, Self::Error>;
    fn finish(self, terminal: bool) -> Result<(), Self::Error>;
}

struct Execution<P, T, E> {
    custody: Custody<P, T>,
    error: Option<E>,
    closing: Result<(), E>,
    terminal: bool,
}

fn step(cursor: &mut OrderedPeerCopyCursorV1, action: Action) {
    *cursor = cursor
        .transition(action)
        .unwrap_or_else(|| std::process::abort());
}

/// One initial publication/scan is allowed even at expiry. After a completion,
/// the same deadline gates every subsequent publication. Poll uses a one-step
/// budget; wait may consume the complete bounded roster within its deadline.
fn execute<S: Scope>(
    mut scope: S,
    sequence: &mut Sequence,
    mut custody: Custody<S::Pair, S::Ticket>,
    deadline: Instant,
    one_step: bool,
) -> Execution<S::Pair, S::Ticket, S::Error> {
    step(&mut sequence.cursor, Action::Open);
    let (error, terminal) = loop {
        let segment = sequence.segments[sequence.cursor.completed() as usize];
        let observation = match custody {
            Custody::Pair(pair) => {
                step(&mut sequence.cursor, Action::Publish);
                match scope.submit(pair, segment) {
                    Ok(ticket) => scope.wait(ticket, segment.byte_len, deadline),
                    Err(observation) => observation,
                }
            }
            Custody::Ticket(ticket) => scope.wait(ticket, segment.byte_len, deadline),
        };
        match observation {
            Observation::Completed(pair) => {
                let segment = sequence.cursor.completed();
                step(&mut sequence.cursor, Action::Complete { segment });
                custody = Custody::Pair(pair);
                if sequence.cursor.completed() == sequence.cursor.count()
                    || one_step
                    || Instant::now() >= deadline
                {
                    break (None, false);
                }
            }
            Observation::Pending(ticket) => {
                custody = Custody::Ticket(ticket);
                break (None, false);
            }
            Observation::Failed {
                error,
                custody: retained,
                terminal,
            } => {
                if matches!(&retained, Custody::Pair(_)) {
                    step(&mut sequence.cursor, Action::Recover);
                }
                custody = retained;
                break (Some(error), terminal);
            }
        }
    };
    let closing = scope.finish(terminal);
    if closing.is_err() {
        step(&mut sequence.cursor, Action::CloseFailed);
    } else {
        step(&mut sequence.cursor, Action::Close);
    }
    if terminal && closing.is_ok() {
        step(&mut sequence.cursor, Action::Quarantine);
    } else if !terminal && closing.is_ok() && error.is_some() {
        step(&mut sequence.cursor, Action::Fail);
    }
    let terminal = terminal || closing.is_err();
    Execution {
        custody,
        error,
        closing,
        terminal,
    }
}

type Pair = (
    Gfx942XgmiMappedDeviceMemoryV1,
    Gfx942XgmiMappedDeviceMemoryV1,
);
struct NativeScope<'a>(Gfx942NativeXgmiSdmaBatchV1<'a>);

impl Scope for NativeScope<'_> {
    type Pair = Pair;
    type Ticket = Gfx942SdmaCopyTicketV1;
    type Error = Gfx942SdmaErrorV1;

    fn submit(
        &mut self,
        (source, destination): Pair,
        segment: RuntimePeerCopySegmentV1,
    ) -> Result<Self::Ticket, Observation<Pair, Self::Ticket, Self::Error>> {
        self.0
            .submit(
                source,
                segment.source_offset,
                destination,
                segment.destination_offset,
                segment.byte_len as u32,
            )
            .map_err(|failure| match failure {
                Gfx942XgmiCopyFailureV1::Recoverable {
                    error,
                    source,
                    destination,
                } => Observation::Failed {
                    error,
                    custody: Custody::Pair((source, destination)),
                    terminal: false,
                },
                Gfx942XgmiCopyFailureV1::Retained { error, ticket } => Observation::Failed {
                    error,
                    custody: Custody::Ticket(ticket),
                    terminal: true,
                },
                Gfx942XgmiCopyFailureV1::CompletedCurrentnessIndeterminate { error, completed } => {
                    if u64::from(completed.copy_bytes()) != segment.byte_len {
                        std::process::abort();
                    }
                    Observation::Failed {
                        error,
                        custody: Custody::Pair(completed.into_mappings()),
                        terminal: true,
                    }
                }
            })
    }

    fn wait(
        &mut self,
        ticket: Self::Ticket,
        bytes: u64,
        deadline: Instant,
    ) -> Observation<Pair, Self::Ticket, Self::Error> {
        match self.0.wait_batch_until(vec![ticket], deadline) {
            Ok(mut completed) => {
                if completed.len() != 1 || u64::from(completed[0].copy_bytes()) != bytes {
                    std::process::abort();
                }
                Observation::Completed(completed.pop().unwrap().into_mappings())
            }
            Err(Gfx942XgmiBatchWaitFailureV1::Retained { error, tickets }) => {
                if tickets.as_slice() != [ticket] {
                    std::process::abort();
                }
                if matches!(error, Gfx942SdmaErrorV1::Timeout) {
                    Observation::Pending(ticket)
                } else {
                    Observation::Failed {
                        error,
                        custody: Custody::Ticket(ticket),
                        terminal: true,
                    }
                }
            }
            Err(Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
                error,
                mut completed,
            }) => {
                if completed.len() != 1 || u64::from(completed[0].copy_bytes()) != bytes {
                    std::process::abort();
                }
                Observation::Failed {
                    error,
                    custody: Custody::Pair(completed.pop().unwrap().into_mappings()),
                    terminal: true,
                }
            }
        }
    }

    fn finish(self, terminal: bool) -> Result<(), Self::Error> {
        if terminal {
            self.0.finish_terminal()
        } else {
            self.0.finish()
        }
    }
}

impl RuntimePeerCopySegmentsBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn peer_copy_segments_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let invalid = || {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI segment envelope or descriptor",
            )
        };
        validate_ordered_peer_copy_segments_v1(
            source.byte_offset,
            source.byte_len,
            destination.byte_offset,
            destination.byte_len,
            segments,
        )
        .map_err(|_| invalid())?;
        let source_record = self
            .allocations
            .get(&source.allocation)
            .ok_or_else(invalid)?;
        let destination_record = self
            .allocations
            .get(&destination.allocation)
            .ok_or_else(invalid)?;
        let stream_device = *self.streams.get(&stream).ok_or_else(invalid)?;
        if source.byte_offset + source.byte_len > source_record.byte_len
            || destination.byte_offset + destination.byte_len > destination_record.byte_len
        {
            return Err(invalid());
        }
        let mut absolute = Vec::new();
        absolute.try_reserve_exact(segments.len()).map_err(|_| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "native XGMI descriptor snapshot",
            )
        })?;
        for segment in segments {
            let segment = RuntimePeerCopySegmentV1 {
                source_offset: source.byte_offset + segment.source_offset,
                destination_offset: destination.byte_offset + segment.destination_offset,
                byte_len: segment.byte_len,
            };
            admit_xgmi_peer_copy_v1(XgmiPeerCopyAdmissionV1 {
                stream_device,
                source_device: source_record.device,
                destination_device: destination_record.device,
                source_offset: segment.source_offset,
                source_len: segment.byte_len,
                source_allocation_len: source_record.byte_len,
                source_access: source.access,
                destination_offset: segment.destination_offset,
                destination_len: segment.byte_len,
                destination_allocation_len: destination_record.byte_len,
                destination_access: destination.access,
            })
            .map_err(|_| invalid())?;
            absolute.push(segment);
        }
        if self.active_by_direction[source_record.device] != 0 {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "ordered XGMI copy requires an idle directional submission domain",
            ));
        }
        let direction = source_record.device;
        let first = absolute[0];
        let sequence = Sequence {
            cursor: OrderedPeerCopyCursorV1::new(absolute.len()).unwrap(),
            segments: absolute,
        };
        let id = self.peer_copy_v1(
            stream,
            BackendMemoryRegionV1 {
                byte_offset: first.source_offset,
                byte_len: first.byte_len,
                ..source
            },
            BackendMemoryRegionV1 {
                byte_offset: first.destination_offset,
                byte_len: first.byte_len,
                ..destination
            },
            dependencies,
        )?;
        self.active
            .get_mut(&id)
            .unwrap_or_else(|| std::process::abort())
            .sequence = Some(sequence);
        self.sequence_by_direction[direction] = Some(id);
        Ok(id)
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    pub(super) fn progress_peer_segments(
        &mut self,
        id: u64,
        deadline: Instant,
        one_step: bool,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let active = &self.active[&id];
        if xgmi_submission_has_failed_dependency_v1(active, &self.submissions) {
            if active.ticket.is_some()
                || active
                    .sequence
                    .as_ref()
                    .is_some_and(Sequence::ever_published)
            {
                return Err(
                    self.terminal_error("ordered XGMI dependency changed after publication")
                );
            }
            let active = self.active.remove(&id).unwrap();
            return Ok(self.finish_failed(active));
        }
        if active.dependencies.iter().any(|dependency| {
            self.submissions
                .get(dependency)
                .is_none_or(|record| record.status != BackendPollV1::Succeeded)
        }) {
            return Ok(BackendPollV1::Pending);
        }
        let sequence = active
            .sequence
            .as_ref()
            .unwrap_or_else(|| std::process::abort());
        let admission = super::xgmi_batch::Admission {
            direction: active.direction,
            published: active.ticket.is_some(),
        };
        let valid = active.direction < 2
            && !sequence.cursor.is_open()
            && sequence.cursor.outcome() == fe2o3_runtime_model::OrderedPeerCopyOutcomeV1::Running
            && sequence.cursor.has_ticket() == active.ticket.is_some()
            && sequence.cursor.count() as usize == sequence.segments.len()
            && sequence.cursor.completed() < sequence.cursor.count()
            && self.sequence_by_direction[active.direction] == Some(id)
            && self.active_by_direction[active.direction] == 1
            && self.batch_custody_is_valid(&[id], admission).map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "ordered XGMI custody scratch",
                )
            })?;
        if !valid {
            return Err(self.terminal_error("ordered XGMI cursor or custody corruption"));
        }
        #[cfg(feature = "hardware-diagnostic")]
        {
            if let Some(recorder) = self.xgmi_diagnostic.as_mut() {
                recorder.invalidate();
            }
            if let Some(recorder) = self.xgmi_aggregate_diagnostic.as_mut() {
                recorder.invalidate();
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_peer_segments(id, deadline, one_step)
        }));
        super::xgmi_batch::finish_native_attempt(result, &mut self.terminal)
    }

    fn run_peer_segments(
        &mut self,
        id: u64,
        deadline: Instant,
        one_step: bool,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = &self.active[&id];
        let (direction, source_id, destination_id) =
            (active.direction, active.source, active.destination);
        self.ensure_queue(direction)?;
        let custody = if let Some(ticket) = self.active.get_mut(&id).unwrap().ticket.take() {
            Custody::Ticket(ticket)
        } else {
            let source = match self.map_allocation(source_id, direction) {
                Ok(source) => source,
                Err(failure) => return self.fail_sequence_preparation(id, failure),
            };
            match self.map_allocation(destination_id, direction) {
                Ok(destination) => Custody::Pair((source, destination)),
                Err(failure) => {
                    if self.terminal {
                        self.quarantine_mapping(source_id, source);
                    } else {
                        self.restore_mapped(source_id, source)?;
                    }
                    return self.fail_sequence_preparation(id, failure);
                }
            }
        };
        let result = {
            let (source, destination) = Self::session_pair(&mut self.sessions, direction);
            let queue = self.queues[direction].as_mut().unwrap();
            match queue.begin_batch(source, destination) {
                Ok(scope) => {
                    let sequence = self.active.get_mut(&id).unwrap().sequence.as_mut().unwrap();
                    Ok(execute(
                        NativeScope(scope),
                        sequence,
                        custody,
                        deadline,
                        one_step,
                    ))
                }
                Err(error) => Err((error, custody)),
            }
        };
        let execution = match result {
            Ok(execution) => execution,
            Err((error, custody)) => {
                step(
                    &mut self
                        .active
                        .get_mut(&id)
                        .unwrap()
                        .sequence
                        .as_mut()
                        .unwrap()
                        .cursor,
                    Action::Quarantine,
                );
                self.restore_sequence_custody(id, custody, true);
                self.batch_quarantine(direction);
                return Err(
                    self.terminal_error(format!("ordered XGMI opening currentness: {error}"))
                );
            }
        };
        self.restore_sequence_custody(id, execution.custody, execution.terminal);
        if execution.terminal {
            self.batch_quarantine(direction);
            return Err(self.terminal_error(format!(
                "ordered XGMI native failure: {:?}; closing: {:?}",
                execution.error, execution.closing
            )));
        }
        if execution.error.is_some() {
            let active = self.active.remove(&id).unwrap();
            return Ok(self.finish_failed(active));
        }
        let sequence = self.active.get_mut(&id).unwrap().sequence.as_mut().unwrap();
        if sequence.cursor.completed() == sequence.cursor.count() {
            step(&mut sequence.cursor, Action::Succeed);
            let active = self.active.remove(&id).unwrap();
            Ok(self.settle_submission(active, BackendPollV1::Succeeded))
        } else {
            Ok(BackendPollV1::Pending)
        }
    }

    fn restore_sequence_custody(
        &mut self,
        id: u64,
        custody: Custody<Pair, Gfx942SdmaCopyTicketV1>,
        terminal: bool,
    ) {
        let direction = self.active[&id].direction;
        remove_xgmi_progress_index_v1(
            &mut self.ready_by_direction[direction],
            &mut self.in_flight_by_direction[direction],
            id,
        );
        match custody {
            Custody::Pair((source, destination)) => {
                self.restore_batch_pair(id, source, destination, terminal);
                if !terminal {
                    enqueue_xgmi_ready_id_v1(&mut self.ready_by_direction[direction], id);
                }
            }
            Custody::Ticket(ticket) => {
                self.active.get_mut(&id).unwrap().ticket = Some(ticket);
                insert_ordered_xgmi_id_v1(&mut self.in_flight_by_direction[direction], id);
            }
        }
    }

    fn fail_sequence_preparation(
        &mut self,
        id: u64,
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.terminal {
            return Err(failure);
        }
        let active = self.active.remove(&id).unwrap();
        Ok(self.finish_failed(active))
    }
}

#[cfg(test)]
mod tests;
