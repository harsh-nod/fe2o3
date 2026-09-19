//! Exact-roster XGMI progress inside one full currentness envelope.

#![forbid(unsafe_code)]

use super::xgmi_batch_diagnostic::{CallTimer, Phase};
use super::*;
use crate::{
    MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1, RuntimePeerCopyBatchBackendV1,
    RuntimePeerCopyBatchPollV1,
};
use fe2o3_kfd::{
    Gfx942NativeXgmiSdmaBatchV1, Gfx942SdmaErrorV1, Gfx942XgmiBatchWaitFailureV1,
    Gfx942XgmiCompletedCopyV1,
};

const _: () = assert!(MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1 == GFX942_SDMA_MAX_IN_FLIGHT_V1);

#[cfg(test)]
mod tests;

fn finish_native_attempt<T>(result: std::thread::Result<T>, terminal: &mut bool) -> T {
    match result {
        Ok(result) => result,
        Err(_) => {
            // Moved native authority cannot be reconstructed after unwinding.
            // Do not format diagnostics, allocate, drop the payload or resume.
            *terminal = true;
            std::process::abort();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdmissionError {
    Invalid,
    Busy,
    Corrupt,
    Capacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Admission {
    direction: usize,
    published: bool,
}

fn admit(
    requested: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    ready: &[VecDeque<u64>; 2],
    in_flight: &[Vec<u64>; 2],
    completed: &HashMap<u64, SubmissionRecordV1>,
) -> Result<Admission, AdmissionError> {
    admit_with_reservation(
        requested,
        active,
        ready,
        in_flight,
        completed,
        Vec::try_reserve_exact,
    )
}

fn admit_with_reservation(
    requested: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    ready: &[VecDeque<u64>; 2],
    in_flight: &[Vec<u64>; 2],
    completed: &HashMap<u64, SubmissionRecordV1>,
    mut reserve: impl FnMut(&mut Vec<u64>, usize) -> Result<(), std::collections::TryReserveError>,
) -> Result<Admission, AdmissionError> {
    if requested.is_empty() || requested.len() > MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1 {
        return Err(AdmissionError::Invalid);
    }
    for (index, id) in requested.iter().enumerate() {
        if requested[..index].contains(id) {
            return Err(AdmissionError::Invalid);
        }
    }
    // Sorted scratch indexes avoid quadratic backlog scans without changing
    // FIFO execution order. Allocation rejection remains before native effects.
    let mut ready_index = [Vec::new(), Vec::new()];
    for d in 0..2 {
        reserve(&mut ready_index[d], ready[d].len()).map_err(|_| AdmissionError::Capacity)?;
        ready_index[d].extend(ready[d].iter().copied());
        ready_index[d].sort_unstable();
    }
    // Check index integrity before classifying an unknown ID or caller subset.
    for d in 0..2 {
        for (index, id) in ready_index[d].iter().enumerate() {
            let record = active.get(id).ok_or(AdmissionError::Corrupt)?;
            if record.id != *id
                || index > 0 && ready_index[d][index - 1] == *id
                || !xgmi_submission_is_ready_v1(record, completed, d)
                || completed.contains_key(id)
            {
                return Err(AdmissionError::Corrupt);
            }
        }
        for (index, id) in in_flight[d].iter().enumerate() {
            let record = active.get(id).ok_or(AdmissionError::Corrupt)?;
            if record.id != *id
                || record.direction != d
                || record.ticket.is_none()
                || index > 0 && in_flight[d][index - 1] >= *id
                || completed.contains_key(id)
                || record.dependencies.iter().any(|dependency| {
                    completed
                        .get(dependency)
                        .is_none_or(|r| r.status != BackendPollV1::Succeeded)
                })
            {
                return Err(AdmissionError::Corrupt);
            }
        }
    }
    for (id, record) in active {
        let d = record.direction;
        if d > 1
            || record.id != *id
            || record.source == record.destination
            || completed.contains_key(id)
            || ready_index[1 - d].binary_search(id).is_ok()
            || in_flight[1 - d].binary_search(id).is_ok()
            || in_flight[d].binary_search(id).is_ok() != record.ticket.is_some()
            || ready_index[d].binary_search(id).is_ok()
                != xgmi_submission_is_ready_v1(record, completed, d)
        {
            return Err(AdmissionError::Corrupt);
        }
    }
    if requested.iter().any(|id| !active.contains_key(id)) {
        return Err(AdmissionError::Invalid);
    }
    let first = &active[&requested[0]];
    let admission = Admission {
        direction: first.direction,
        published: first.ticket.is_some(),
    };
    let direction = admission.direction;
    if admission.published {
        if in_flight[direction].len() != requested.len()
            || requested
                .iter()
                .any(|id| in_flight[direction].binary_search(id).is_err())
        {
            return Err(AdmissionError::Busy);
        }
    } else if !in_flight[direction].is_empty()
        || ready[direction].len() != requested.len()
        || requested
            .iter()
            .any(|id| ready_index[direction].binary_search(id).is_err())
    {
        return Err(AdmissionError::Busy);
    }
    for (index, id) in requested.iter().enumerate() {
        let record = &active[id];
        if record.direction != direction || record.ticket.is_some() != admission.published {
            return Err(AdmissionError::Corrupt);
        }
        if admission.published {
            if ready_index[direction].binary_search(id).is_ok() {
                return Err(AdmissionError::Corrupt);
            }
        } else if !xgmi_submission_is_ready_v1(record, completed, direction) {
            return Err(AdmissionError::Corrupt);
        }
        for earlier in &requested[..index] {
            let previous = active.get(earlier).ok_or(AdmissionError::Corrupt)?;
            if previous.stream == record.stream
                || [previous.source, previous.destination]
                    .iter()
                    .any(|id| *id == record.source || *id == record.destination)
            {
                return Err(AdmissionError::Corrupt);
            }
        }
    }
    Ok(admission)
}

enum Input<R, T> {
    Ready(Vec<R>),
    Published(Vec<T>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DependencyScratchCapacity;

struct DependencyUseCount {
    id: u64,
    records: usize,
    last_record: usize,
}

fn count_dependency_uses<'a>(
    relevant: &mut [DependencyUseCount],
    records: impl Iterator<Item = &'a XgmiRuntimeSubmissionV1>,
) -> bool {
    for (ordinal, record) in records.enumerate() {
        for dependency in &record.dependencies {
            let Ok(slot) = relevant.binary_search_by_key(dependency, |entry| entry.id) else {
                continue;
            };
            let entry = &mut relevant[slot];
            // Match contains-per-record semantics, including malformed duplicate
            // edges in an unrelated record. Rows start with the MAX sentinel.
            if entry.last_record != ordinal {
                let Some(count) = entry.records.checked_add(1) else {
                    return false;
                };
                entry.records = count;
                entry.last_record = ordinal;
            }
        }
    }
    true
}

fn valid_dependency_indexes(
    ids: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &HashMap<u64, SubmissionRecordV1>,
    dependency_waiters: &HashMap<u64, Vec<u64>>,
    retain_counts: &HashMap<u64, usize>,
) -> Result<bool, DependencyScratchCapacity> {
    valid_dependency_indexes_with_reservation(
        ids,
        active,
        completed,
        dependency_waiters,
        retain_counts,
        Vec::try_reserve_exact,
    )
}

fn valid_dependency_indexes_with_reservation(
    ids: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &HashMap<u64, SubmissionRecordV1>,
    dependency_waiters: &HashMap<u64, Vec<u64>>,
    retain_counts: &HashMap<u64, usize>,
    mut reserve: impl FnMut(
        &mut Vec<DependencyUseCount>,
        usize,
    ) -> Result<(), std::collections::TryReserveError>,
) -> Result<bool, DependencyScratchCapacity> {
    if ids.is_empty() {
        return Ok(true);
    }
    let raw_len = ids
        .iter()
        .try_fold(ids.len(), |length, id| {
            length.checked_add(active[id].dependencies.len())
        })
        .ok_or(DependencyScratchCapacity)?;
    let mut relevant = Vec::new();
    reserve(&mut relevant, raw_len).map_err(|_| DependencyScratchCapacity)?;
    relevant.extend(
        ids.iter()
            .chain(ids.iter().flat_map(|id| &active[id].dependencies))
            .map(|id| DependencyUseCount {
                id: *id,
                records: 0,
                last_record: usize::MAX,
            }),
    );
    relevant.sort_unstable_by_key(|entry| entry.id);
    relevant.dedup_by_key(|entry| entry.id);
    if !count_dependency_uses(&mut relevant, active.values()) {
        return Ok(false);
    }
    let uses = |id: &u64| {
        relevant
            .binary_search_by_key(id, |entry| entry.id)
            .ok()
            .map(|slot| relevant[slot].records)
    };
    for id in ids {
        let record = &active[id];
        let waiters = dependency_waiters.get(id).map_or(&[][..], Vec::as_slice);
        let Some(expected_waiters) = uses(id) else {
            return Ok(false);
        };
        if waiters.len() != expected_waiters
            || retain_counts.get(id).copied().unwrap_or(0) != expected_waiters
            || waiters.iter().enumerate().any(|(index, waiter)| {
                index > 0 && waiters[index - 1] >= *waiter
                    || active
                        .get(waiter)
                        .is_none_or(|record| !record.dependencies.contains(id))
            })
        {
            return Ok(false);
        }
        for (index, dependency) in record.dependencies.iter().enumerate() {
            if *dependency >= *id
                || record.dependencies[..index].contains(dependency)
                || completed
                    .get(dependency)
                    .is_none_or(|record| record.status != BackendPollV1::Succeeded)
                || dependency_waiters.contains_key(dependency)
                || retain_counts
                    .get(dependency)
                    .is_none_or(|count| uses(dependency) != Some(*count))
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

enum Operation<R, T, C, E> {
    Unpublished { error: E, requests: Vec<R> },
    PublicationIndeterminate { error: E, tickets: Vec<T> },
    Retained { error: E, tickets: Vec<T> },
    Completed(Vec<C>),
    Indeterminate { error: E, completed: Vec<C> },
}

trait Scope: Sized {
    type Request;
    type Ticket;
    type Completed;
    type Error;
    fn submit(
        &mut self,
        requests: Vec<Self::Request>,
    ) -> Result<Vec<Self::Ticket>, ScopedOperation<Self>>;
    fn wait(
        &mut self,
        tickets: Vec<Self::Ticket>,
        deadline: Instant,
    ) -> Operation<Self::Request, Self::Ticket, Self::Completed, Self::Error>;
    fn is_timeout(error: &Self::Error) -> bool;
    fn finish(self, terminal: bool) -> Result<(), Self::Error>;
}

type ScopedOperation<S> = Operation<
    <S as Scope>::Request,
    <S as Scope>::Ticket,
    <S as Scope>::Completed,
    <S as Scope>::Error,
>;

#[cfg(test)]
fn execute<S: Scope>(
    scope: S,
    input: Input<S::Request, S::Ticket>,
    deadline: Instant,
) -> (ScopedOperation<S>, Result<(), S::Error>) {
    execute_profiled::<S, false>(scope, input, deadline, &mut CallTimer::new())
}

fn execute_profiled<S: Scope, const PROFILE: bool>(
    mut scope: S,
    input: Input<S::Request, S::Ticket>,
    deadline: Instant,
    timer: &mut CallTimer<PROFILE>,
) -> (ScopedOperation<S>, Result<(), S::Error>) {
    let tickets = match input {
        Input::Ready(requests) => timer.measure(Phase::Submission, || scope.submit(requests)),
        Input::Published(tickets) => Ok(tickets),
    };
    let operation = match tickets {
        Ok(tickets) => timer.measure(Phase::Wait, || scope.wait(tickets, deadline)),
        Err(operation) => operation,
    };
    let terminal = match &operation {
        Operation::Retained { error, .. } => !S::is_timeout(error),
        Operation::PublicationIndeterminate { .. } | Operation::Indeterminate { .. } => true,
        Operation::Unpublished { .. } | Operation::Completed(_) => false,
    };
    // Custody in `operation` remains local and unobservable until full close.
    let closing = timer.measure(Phase::Closing, || scope.finish(terminal));
    (operation, closing)
}

impl Scope for Gfx942NativeXgmiSdmaBatchV1<'_> {
    type Request = Gfx942XgmiSdmaCopyRequestV1;
    type Ticket = Gfx942SdmaCopyTicketV1;
    type Completed = Gfx942XgmiCompletedCopyV1;
    type Error = Gfx942SdmaErrorV1;

    fn submit(
        &mut self,
        requests: Vec<Self::Request>,
    ) -> Result<Vec<Self::Ticket>, ScopedOperation<Self>> {
        self.submit_batch(requests)
            .map_err(|failure| match failure {
                Gfx942XgmiBatchSubmissionFailureV1::Recoverable { error, requests } => {
                    Operation::Unpublished { error, requests }
                }
                Gfx942XgmiBatchSubmissionFailureV1::Retained { error, tickets } => {
                    Operation::PublicationIndeterminate { error, tickets }
                }
            })
    }

    fn wait(&mut self, tickets: Vec<Self::Ticket>, deadline: Instant) -> ScopedOperation<Self> {
        match self.wait_batch_until(tickets, deadline) {
            Ok(completed) => Operation::Completed(completed),
            Err(Gfx942XgmiBatchWaitFailureV1::Retained { error, tickets }) => {
                Operation::Retained { error, tickets }
            }
            Err(Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
                error,
                completed,
            }) => Operation::Indeterminate { error, completed },
        }
    }

    fn is_timeout(error: &Self::Error) -> bool {
        matches!(error, Gfx942SdmaErrorV1::Timeout)
    }

    fn finish(self, terminal: bool) -> Result<(), Self::Error> {
        if terminal {
            self.finish_terminal()
        } else {
            self.finish()
        }
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    fn batch_custody_is_valid(
        &self,
        ids: &[u64],
        admission: Admission,
    ) -> Result<bool, DependencyScratchCapacity> {
        if self.completion_reservations != self.active.len()
            || self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                < self.completion_reservations
            || (0..2).any(|direction| {
                self.active_by_direction[direction]
                    != self
                        .active
                        .values()
                        .filter(|record| record.direction == direction)
                        .count()
            })
            || self.active_stream_owners.len() != self.active.len()
        {
            return Ok(false);
        }
        if !valid_dependency_indexes(
            ids,
            &self.active,
            &self.submissions,
            &self.dependency_waiters,
            &self.dependency_retain_counts,
        )? {
            return Ok(false);
        }
        for (id, record) in &self.active {
            let Ok(depth) =
                next_xgmi_dependency_depth_v1(&self.dependency_depths, &record.dependencies)
            else {
                return Ok(false);
            };
            if self.active_stream_owners.get(&record.stream) != Some(id)
                || self.streams.get(&record.stream) != Some(&(1 - record.direction))
                || self.dependency_depths.get(id) != Some(&depth)
            {
                return Ok(false);
            }
        }
        for id in ids {
            let active = &self.active[id];
            if self.active_stream_owners.get(&active.stream) != Some(id)
                || self.streams.get(&active.stream) != Some(&(1 - admission.direction))
                || active.byte_len == 0
                || active.dependency_cursor > active.dependencies.len()
            {
                return Ok(false);
            }
            for (allocation, device, offset) in [
                (active.source, admission.direction, active.source_offset),
                (
                    active.destination,
                    1 - admission.direction,
                    active.destination_offset,
                ),
            ] {
                let Some(record) = self.allocations.get(&allocation) else {
                    return Ok(false);
                };
                if record.device != device
                    || offset
                        .checked_add(u64::from(active.byte_len))
                        .is_none_or(|end| end > record.byte_len)
                    || match &record.authority {
                        None => !admission.published,
                        Some(XgmiAllocationAuthorityV1::Unmapped(_)) => admission.published,
                        Some(XgmiAllocationAuthorityV1::Mapped(mapping)) => {
                            admission.published || !mapping.is_fully_mapped()
                        }
                        Some(XgmiAllocationAuthorityV1::QuarantinedMapped(_)) => true,
                    }
                {
                    return Ok(false);
                }
                let Some(owners) = self.active_allocation_owners.get(&allocation) else {
                    return Ok(false);
                };
                if !owners.contains(id) {
                    return Ok(false);
                }
                // Later dependency-blocked copies may share this allocation.
                // They are not part of the admitted native frontier.
                for (index, owner) in owners.iter().enumerate() {
                    let Some(other) = self.active.get(owner) else {
                        return Ok(false);
                    };
                    if owners[..index].contains(owner)
                        || ![other.source, other.destination].contains(&allocation)
                        || *owner != *id
                            && (*owner < *id
                                || other.ticket.is_some()
                                || !other.dependencies.contains(id))
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    fn batch_quarantine(&mut self, direction: usize) {
        self.terminal = true;
        if let Some(queue) = self.queues[direction].as_mut() {
            let (source, destination) = Self::session_pair(&mut self.sessions, direction);
            queue.quarantine_batch_v1(source, destination);
        }
    }

    fn restore_batch_pair(
        &mut self,
        id: u64,
        source: Gfx942XgmiMappedDeviceMemoryV1,
        destination: Gfx942XgmiMappedDeviceMemoryV1,
        quarantine: bool,
    ) {
        let active = self
            .active
            .get(&id)
            .unwrap_or_else(|| std::process::abort());
        let allocations = [active.source, active.destination];
        for (id, mapping) in allocations.into_iter().zip([source, destination]) {
            let record = self
                .allocations
                .get_mut(&id)
                .unwrap_or_else(|| std::process::abort());
            if record.authority.is_some() || (!quarantine && !mapping.is_fully_mapped()) {
                std::process::abort();
            }
            record.authority = Some(if quarantine {
                XgmiAllocationAuthorityV1::QuarantinedMapped(mapping)
            } else {
                XgmiAllocationAuthorityV1::Mapped(mapping)
            });
        }
    }

    fn restore_batch_requests(
        &mut self,
        ids: &[u64],
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        quarantine: bool,
    ) {
        if ids.len() != requests.len() {
            std::process::abort();
        }
        for (id, request) in ids.iter().copied().zip(requests) {
            let (source, destination) = request.into_mappings();
            self.restore_batch_pair(id, source, destination, quarantine);
        }
    }

    fn commit_batch_status(&mut self, ids: &[u64], status: BackendPollV1) {
        for id in ids {
            let active = self
                .active
                .remove(id)
                .unwrap_or_else(|| std::process::abort());
            self.settle_submission(active, status);
        }
    }

    fn install_batch_tickets(
        &mut self,
        ids: &[u64],
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        admission: Admission,
    ) {
        if ids.len() != tickets.len() {
            std::process::abort();
        }
        for (id, ticket) in ids.iter().copied().zip(tickets) {
            let active = self
                .active
                .get_mut(&id)
                .unwrap_or_else(|| std::process::abort());
            if admission.published && active.ticket != Some(ticket) {
                std::process::abort();
            }
            active.ticket = Some(ticket);
            if !admission.published {
                insert_ordered_xgmi_id_v1(
                    &mut self.in_flight_by_direction[admission.direction],
                    id,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn run_admitted_peer_batch<const PROFILE: bool>(
        &mut self,
        admission: Admission,
        ids: Vec<u64>,
        mut requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: Instant,
        timer: &mut CallTimer<PROFILE>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let preparation_start = timer.start();
        let direction = admission.direction;
        self.ensure_queue(direction)?;
        if !admission.published {
            for id in &ids {
                if self.ready_by_direction[direction].pop_front() != Some(*id) {
                    std::process::abort();
                }
            }
            for (index, id) in ids.iter().copied().enumerate() {
                let active = &self.active[&id];
                let (source_id, destination_id, source_offset, destination_offset, byte_len) = (
                    active.source,
                    active.destination,
                    active.source_offset,
                    active.destination_offset,
                    active.byte_len,
                );
                let source = match self.map_allocation(source_id, direction) {
                    Ok(source) => source,
                    Err(failure) => {
                        return self.fail_batch_preparation(
                            &ids,
                            &ids[..index],
                            requests,
                            direction,
                            failure,
                        );
                    }
                };
                let destination = match self.map_allocation(destination_id, direction) {
                    Ok(destination) => destination,
                    Err(failure) => {
                        let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
                        let record = self
                            .allocations
                            .get_mut(&source_id)
                            .unwrap_or_else(|| std::process::abort());
                        if record.authority.is_some() {
                            std::process::abort();
                        }
                        record.authority = Some(if terminal {
                            XgmiAllocationAuthorityV1::QuarantinedMapped(source)
                        } else {
                            XgmiAllocationAuthorityV1::Mapped(source)
                        });
                        return self.fail_batch_preparation(
                            &ids,
                            &ids[..index],
                            requests,
                            direction,
                            failure,
                        );
                    }
                };
                requests.push(Gfx942XgmiSdmaCopyRequestV1::new(
                    source,
                    source_offset,
                    destination,
                    destination_offset,
                    byte_len,
                ));
            }
        }
        timer.end(Phase::Preparation, preparation_start);
        let result = {
            let (source, destination) = Self::session_pair(&mut self.sessions, direction);
            let queue = self.queues[direction]
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            match timer.measure(Phase::Opening, || queue.begin_batch(source, destination)) {
                Ok(scope) => Ok(execute_profiled::<_, PROFILE>(
                    scope,
                    if admission.published {
                        Input::Published(tickets)
                    } else {
                        Input::Ready(requests)
                    },
                    deadline,
                    timer,
                )),
                Err(error) => Err((error, requests)),
            }
        };
        let (operation, closing) = match result {
            Ok(result) => result,
            Err((error, requests)) => {
                self.batch_quarantine(direction);
                if !admission.published {
                    self.restore_batch_requests(&ids, requests, true);
                }
                return Err(self.terminal_error(format!("XGMI aggregate opening: {error}")));
            }
        };
        let settlement_start = timer.start();
        let outcome = match operation {
            Operation::PublicationIndeterminate { error, tickets } => {
                self.install_batch_tickets(&ids, tickets, admission);
                self.batch_quarantine(direction);
                Err(self.terminal_error(format!(
                    "XGMI aggregate publication indeterminate: {error}; closing: {closing:?}"
                )))
            }
            Operation::Retained { error, tickets } => {
                self.install_batch_tickets(&ids, tickets, admission);
                if matches!(error, Gfx942SdmaErrorV1::Timeout) && closing.is_ok() {
                    return Ok(RuntimePeerCopyBatchPollV1::Pending);
                }
                self.batch_quarantine(direction);
                Err(self.terminal_error(format!(
                    "XGMI aggregate retained work: {error}; closing: {closing:?}"
                )))
            }
            Operation::Unpublished { error, requests } => {
                let terminal = closing.is_err();
                if terminal {
                    self.batch_quarantine(direction);
                }
                self.restore_batch_requests(&ids, requests, terminal);
                if terminal {
                    Err(self.terminal_error(format!(
                        "XGMI aggregate unpublished: {error}; closing: {closing:?}"
                    )))
                } else {
                    self.commit_batch_status(
                        &ids,
                        BackendPollV1::Failed {
                            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
                        },
                    );
                    Err(Self::quiescent_error(
                        KfdRuntimeBackendErrorKindV1::Native,
                        format!("XGMI aggregate unpublished: {error}"),
                    ))
                }
            }
            Operation::Completed(completed) => {
                self.finish_batch_completions(&ids, completed, direction, closing.err())
            }
            Operation::Indeterminate { error, completed } => {
                self.finish_batch_completions(&ids, completed, direction, Some(error))
            }
        };
        timer.end(Phase::Settlement, settlement_start);
        outcome
    }

    fn fail_batch_preparation(
        &mut self,
        ids: &[u64],
        prepared: &[u64],
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
        direction: usize,
        failure: RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let terminal = matches!(failure, RuntimeBackendFailureV1::Terminal(_));
        if terminal {
            self.batch_quarantine(direction);
        }
        self.restore_batch_requests(prepared, requests, terminal);
        match failure {
            failure @ RuntimeBackendFailureV1::Terminal(_) => Err(failure),
            RuntimeBackendFailureV1::Rejected(error)
            | RuntimeBackendFailureV1::Quiescent(error) => {
                self.commit_batch_status(
                    ids,
                    BackendPollV1::Failed {
                        code: COOPERATIVE_COPY_FAILURE_CODE_V1,
                    },
                );
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
        }
    }

    fn finish_batch_completions(
        &mut self,
        ids: &[u64],
        completed: Vec<Gfx942XgmiCompletedCopyV1>,
        direction: usize,
        error: Option<Gfx942SdmaErrorV1>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if ids.len() != completed.len() {
            std::process::abort();
        }
        let terminal = error.is_some();
        if terminal {
            self.batch_quarantine(direction);
        }
        for (id, completed) in ids.iter().copied().zip(completed) {
            if self.active[&id].byte_len != completed.copy_bytes() {
                std::process::abort();
            }
            let (source, destination) = completed.into_mappings();
            self.restore_batch_pair(id, source, destination, terminal);
        }
        if let Some(error) = error {
            return Err(
                self.terminal_error(format!("XGMI aggregate completion currentness: {error}"))
            );
        }
        // All mapping authority is restored before any logical Success exists.
        self.commit_batch_status(ids, BackendPollV1::Succeeded);
        Ok(RuntimePeerCopyBatchPollV1::Succeeded)
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    fn progress_peer_copy_batch_profiled<const PROFILE: bool>(
        &mut self,
        requested: &[u64],
        deadline: Instant,
        timer: &mut CallTimer<PROFILE>,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let admission_start = timer.start();
        self.require_live()?;
        let admission = admit(
            requested,
            &self.active,
            &self.ready_by_direction,
            &self.in_flight_by_direction,
            &self.submissions,
        )
        .map_err(|error| match error {
            AdmissionError::Invalid => Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "invalid XGMI aggregate roster",
            ),
            AdmissionError::Busy => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "XGMI aggregate requires the complete ready or in-flight roster",
            ),
            AdmissionError::Corrupt => self.terminal_error("XGMI aggregate index corruption"),
            AdmissionError::Capacity => Self::rejected(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "XGMI aggregate ready-index storage",
            ),
        })?;
        let custody_is_valid = self
            .batch_custody_is_valid(requested, admission)
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI aggregate dependency-index storage",
                )
            })?;
        if !custody_is_valid {
            return Err(self.terminal_error("XGMI aggregate custody corruption"));
        }
        let mut ids = Vec::new();
        let mut requests = Vec::new();
        let mut tickets = Vec::new();
        for reserve in [
            ids.try_reserve_exact(requested.len()),
            requests.try_reserve_exact(requested.len()),
            tickets.try_reserve_exact(requested.len()),
        ] {
            reserve.map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "XGMI aggregate storage",
                )
            })?;
        }
        if admission.published {
            ids.extend_from_slice(&self.in_flight_by_direction[admission.direction]);
            tickets.extend(ids.iter().map(|id| {
                self.active[id]
                    .ticket
                    .unwrap_or_else(|| std::process::abort())
            }));
        } else {
            ids.extend(self.ready_by_direction[admission.direction].iter().copied());
        }
        if self.in_flight_by_direction[admission.direction].capacity() < ids.len() {
            return Err(self.terminal_error("XGMI aggregate lacks reserved in-flight slots"));
        }
        timer.end(Phase::Admission, admission_start);
        #[cfg(feature = "hardware-diagnostic")]
        if let Some(recorder) = self.xgmi_diagnostic.as_mut() {
            recorder.invalidate();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_admitted_peer_batch::<PROFILE>(
                admission, ids, requests, tickets, deadline, timer,
            )
        }));
        finish_native_attempt(result, &mut self.terminal)
    }
}

impl RuntimePeerCopyBatchBackendV1 for KfdNativeXgmiRuntimeBackendV1 {
    fn progress_peer_copy_batch_v1(
        &mut self,
        requested: &[u64],
        deadline: Instant,
    ) -> Result<RuntimePeerCopyBatchPollV1, RuntimeBackendFailureV1<Self::Error>> {
        #[cfg(feature = "hardware-diagnostic")]
        if self.xgmi_aggregate_diagnostic.is_some() {
            // This lookup identifies a diagnostic candidate only. Operational
            // admission below still validates the complete roster and custody.
            let identity = match requested {
                [id] => self
                    .active
                    .get(id)
                    .map(|active| xgmi_batch_diagnostic::CallIdentity {
                        direction: active.direction,
                        submission: *id,
                        published: active.ticket.is_some(),
                    }),
                _ => None,
            };
            let recorder = self
                .xgmi_aggregate_diagnostic
                .as_mut()
                .expect("armed recorder");
            let armed = match identity {
                Some(identity) => recorder.begin(identity, requested.len()),
                None => {
                    recorder.invalidate();
                    false
                }
            };
            if armed {
                let start = Instant::now();
                let mut timer = CallTimer::<true>::new();
                let result =
                    self.progress_peer_copy_batch_profiled(requested, deadline, &mut timer);
                let observed = matches!(result, Ok(RuntimePeerCopyBatchPollV1::Succeeded))
                    .then(|| timer.finish(start));
                self.xgmi_aggregate_diagnostic
                    .as_mut()
                    .expect("armed recorder")
                    .finish(identity.expect("armed identity"), observed);
                return result;
            }
        }
        self.progress_peer_copy_batch_profiled(requested, deadline, &mut CallTimer::<false>::new())
    }
}
