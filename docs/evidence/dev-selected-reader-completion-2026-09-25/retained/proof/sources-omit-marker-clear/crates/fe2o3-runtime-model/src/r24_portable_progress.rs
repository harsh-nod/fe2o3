//! Independent executable R24 model for bounded portable progress.
//!
//! This model performs no I/O and does not refine Rust threads, the runtime,
//! KFD, HSA, HIP, native execution, hardware scheduling, or liveness.

use alloc::vec::Vec;

pub const R24_MAX_REGISTRATIONS_V1: usize = 65_536;
pub const R24_MAX_PROGRESS_BUDGET_V1: usize = 1_024;
pub const R24_MAX_WINDOW_PACKETS_V1: u16 = 63;
pub const R24_MAX_TRANSFER_PACKETS_V1: u16 = 65;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct R24PortableProgressKeyV1 {
    pub context_generation: u64,
    pub event_id: u64,
    pub stream_id: u64,
}

impl R24PortableProgressKeyV1 {
    pub const fn is_valid(self) -> bool {
        self.context_generation != 0 && self.event_id != 0 && self.stream_id != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R24PortableProgressConfigV1 {
    pub capacity: usize,
    pub poll_budget: usize,
    pub flush_budget: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R24PortableRegistrationRequestV1 {
    pub key: R24PortableProgressKeyV1,
    pub total_packets: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R24RegistrationDispositionV1 {
    Install,
    RejectAfterEventPreflight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R24ProgressPhaseV1 {
    WindowPending {
        ordinal: u8,
        packet_count: u16,
    },
    ContinuationReady {
        completed_packets: u16,
        next_packet_count: u16,
        polled_before_continuation: bool,
    },
    TerminalSucceeded,
    TerminalQuarantined,
}

impl R24ProgressPhaseV1 {
    const fn pollable(self) -> bool {
        matches!(self, Self::WindowPending { .. })
    }

    const fn flushable(self) -> bool {
        matches!(self, Self::ContinuationReady { .. })
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::TerminalSucceeded | Self::TerminalQuarantined)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R24PollDispositionV1 {
    Pending,
    /// Backend rejection retains logical custody but resolves this observer.
    Retryable,
    Completed,
    TerminalFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R24FlushDispositionV1 {
    Published,
    Retryable,
    TerminalFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R24PollStepV1 {
    pub key: R24PortableProgressKeyV1,
    pub disposition: R24PollDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R24FlushStepV1 {
    pub key: R24PortableProgressKeyV1,
    pub disposition: R24FlushDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R24PortableRegistrationSnapshotV1 {
    pub key: R24PortableProgressKeyV1,
    pub total_packets: u16,
    /// Logical event custody; this is not active progress-registry membership.
    pub event_installed: bool,
    /// Logical stream custody; this is not active progress-registry membership.
    pub stream_installed: bool,
    pub custody_retained: bool,
    /// Whether the historical roster entry remains eligible for progress visits.
    pub observing: bool,
    pub abandoned: bool,
    pub phase: R24ProgressPhaseV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R24PortableProgressSnapshotV1 {
    pub config: R24PortableProgressConfigV1,
    pub registrations: Vec<R24PortableRegistrationSnapshotV1>,
    pub poll_cursor: usize,
    pub flush_cursor: usize,
    pub poll_visits: u64,
    pub flush_visits: u64,
    pub stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R24PortableProgressErrorV1 {
    InvalidConfiguration,
    InvalidRegistration,
    DuplicateEvent,
    DuplicateStream,
    CapacityExceeded,
    BudgetExceeded,
    VisitSubstitution,
    UnknownRegistration,
    EngineStopped,
    InvariantViolation,
}

/// Append-only bounded registry with independent poll and flush cursors.
pub struct R24PortableProgressModelV1 {
    config: R24PortableProgressConfigV1,
    registrations: Vec<R24PortableRegistrationSnapshotV1>,
    poll_cursor: usize,
    flush_cursor: usize,
    poll_visits: u64,
    flush_visits: u64,
    stopped: bool,
}

impl R24PortableProgressModelV1 {
    pub fn new_model_only(
        config: R24PortableProgressConfigV1,
    ) -> Result<Self, R24PortableProgressErrorV1> {
        if config.capacity == 0
            || config.capacity > R24_MAX_REGISTRATIONS_V1
            || config.poll_budget == 0
            || config.poll_budget > R24_MAX_PROGRESS_BUDGET_V1
            || config.flush_budget == 0
            || config.flush_budget > R24_MAX_PROGRESS_BUDGET_V1
        {
            return Err(R24PortableProgressErrorV1::InvalidConfiguration);
        }
        Ok(Self {
            config,
            registrations: Vec::new(),
            poll_cursor: 0,
            flush_cursor: 0,
            poll_visits: 0,
            flush_visits: 0,
            stopped: false,
        })
    }

    pub fn snapshot(&self) -> R24PortableProgressSnapshotV1 {
        R24PortableProgressSnapshotV1 {
            config: self.config,
            registrations: self.registrations.clone(),
            poll_cursor: self.poll_cursor,
            flush_cursor: self.flush_cursor,
            poll_visits: self.poll_visits,
            flush_visits: self.flush_visits,
            stopped: self.stopped,
        }
    }

    pub fn register_model_only(
        &mut self,
        request: R24PortableRegistrationRequestV1,
    ) -> Result<(), R24PortableProgressErrorV1> {
        self.register_with_disposition_model_only(request, R24RegistrationDispositionV1::Install)
    }

    pub fn register_with_disposition_model_only(
        &mut self,
        request: R24PortableRegistrationRequestV1,
        disposition: R24RegistrationDispositionV1,
    ) -> Result<(), R24PortableProgressErrorV1> {
        if self.stopped {
            return Err(R24PortableProgressErrorV1::EngineStopped);
        }
        if !request.key.is_valid()
            || request.total_packets == 0
            || request.total_packets > R24_MAX_TRANSFER_PACKETS_V1
        {
            return Err(R24PortableProgressErrorV1::InvalidRegistration);
        }
        if self.registrations.iter().any(|registration| {
            registration.observing && same_event_identity_v1(registration.key, request.key)
        }) {
            return Err(R24PortableProgressErrorV1::DuplicateEvent);
        }
        if self.registrations.iter().any(|registration| {
            registration.observing && same_stream_identity_v1(registration.key, request.key)
        }) {
            return Err(R24PortableProgressErrorV1::DuplicateStream);
        }
        if disposition == R24RegistrationDispositionV1::RejectAfterEventPreflight {
            return Err(R24PortableProgressErrorV1::InvalidRegistration);
        }
        if self.active_registration_count() >= self.config.capacity
            || self.registrations.len() >= R24_MAX_REGISTRATIONS_V1
        {
            return Err(R24PortableProgressErrorV1::CapacityExceeded);
        }
        self.registrations.push(R24PortableRegistrationSnapshotV1 {
            key: request.key,
            total_packets: request.total_packets,
            event_installed: true,
            stream_installed: true,
            custody_retained: true,
            observing: true,
            abandoned: false,
            phase: R24ProgressPhaseV1::WindowPending {
                ordinal: 0,
                packet_count: request.total_packets.min(R24_MAX_WINDOW_PACKETS_V1),
            },
        });
        Ok(())
    }

    pub fn poll_budget_model_only(
        &mut self,
        steps: &[R24PollStepV1],
    ) -> Result<Vec<R24PortableProgressKeyV1>, R24PortableProgressErrorV1> {
        if self.stopped {
            return Err(R24PortableProgressErrorV1::EngineStopped);
        }
        if steps.len() > self.config.poll_budget {
            return Err(R24PortableProgressErrorV1::BudgetExceeded);
        }
        let next_poll_visits = self
            .poll_visits
            .checked_add(steps.len() as u64)
            .ok_or(R24PortableProgressErrorV1::InvariantViolation)?;
        let indices = cyclic_indices_v1(
            &self.registrations,
            self.poll_cursor,
            steps.len(),
            |entry| entry.observing && entry.phase.pollable(),
        )?;
        if indices
            .iter()
            .zip(steps)
            .any(|(index, step)| self.registrations[*index].key != step.key)
        {
            return Err(R24PortableProgressErrorV1::VisitSubstitution);
        }
        for (index, step) in indices.iter().copied().zip(steps) {
            let entry = &mut self.registrations[index];
            match step.disposition {
                R24PollDispositionV1::Pending => {}
                R24PollDispositionV1::Retryable => {
                    entry.observing = false;
                }
                R24PollDispositionV1::Completed => {
                    let R24ProgressPhaseV1::WindowPending {
                        ordinal,
                        packet_count,
                    } = entry.phase
                    else {
                        return Err(R24PortableProgressErrorV1::InvariantViolation);
                    };
                    if ordinal == 1 || packet_count == entry.total_packets {
                        entry.phase = R24ProgressPhaseV1::TerminalSucceeded;
                        entry.observing = false;
                    } else {
                        entry.phase = R24ProgressPhaseV1::ContinuationReady {
                            completed_packets: packet_count,
                            next_packet_count: entry.total_packets - packet_count,
                            polled_before_continuation: ordinal == 0,
                        };
                    }
                }
                R24PollDispositionV1::TerminalFailure => {
                    entry.phase = R24ProgressPhaseV1::TerminalQuarantined;
                    entry.observing = false;
                }
            }
        }
        if let Some(last) = indices.last() {
            self.poll_cursor = (*last + 1) % self.registrations.len();
        }
        self.poll_visits = next_poll_visits;
        Ok(indices
            .into_iter()
            .map(|index| self.registrations[index].key)
            .collect())
    }

    pub fn flush_budget_model_only(
        &mut self,
        steps: &[R24FlushStepV1],
    ) -> Result<Vec<R24PortableProgressKeyV1>, R24PortableProgressErrorV1> {
        if self.stopped {
            return Err(R24PortableProgressErrorV1::EngineStopped);
        }
        if steps.len() > self.config.flush_budget {
            return Err(R24PortableProgressErrorV1::BudgetExceeded);
        }
        let next_flush_visits = self
            .flush_visits
            .checked_add(steps.len() as u64)
            .ok_or(R24PortableProgressErrorV1::InvariantViolation)?;
        let indices = cyclic_indices_v1(
            &self.registrations,
            self.flush_cursor,
            steps.len(),
            |entry| entry.observing && entry.phase.flushable(),
        )?;
        if indices
            .iter()
            .zip(steps)
            .any(|(index, step)| self.registrations[*index].key != step.key)
        {
            return Err(R24PortableProgressErrorV1::VisitSubstitution);
        }
        for (index, step) in indices.iter().copied().zip(steps) {
            let entry = &mut self.registrations[index];
            match step.disposition {
                R24FlushDispositionV1::Retryable => {}
                R24FlushDispositionV1::TerminalFailure => {
                    entry.phase = R24ProgressPhaseV1::TerminalQuarantined;
                    entry.observing = false;
                }
                R24FlushDispositionV1::Published => {
                    let R24ProgressPhaseV1::ContinuationReady {
                        next_packet_count,
                        polled_before_continuation: true,
                        ..
                    } = entry.phase
                    else {
                        return Err(R24PortableProgressErrorV1::InvariantViolation);
                    };
                    entry.phase = R24ProgressPhaseV1::WindowPending {
                        ordinal: 1,
                        packet_count: next_packet_count,
                    };
                }
            }
        }
        if let Some(last) = indices.last() {
            self.flush_cursor = (*last + 1) % self.registrations.len();
        }
        self.flush_visits = next_flush_visits;
        Ok(indices
            .into_iter()
            .map(|index| self.registrations[index].key)
            .collect())
    }

    /// Stops observing one registration without changing runtime progress or custody.
    pub fn abandon_model_only(
        &mut self,
        key: R24PortableProgressKeyV1,
    ) -> Result<(), R24PortableProgressErrorV1> {
        let entry = self
            .registrations
            .iter_mut()
            .rev()
            .find(|entry| entry.observing && entry.key == key)
            .ok_or(R24PortableProgressErrorV1::UnknownRegistration)?;
        entry.observing = false;
        entry.abandoned = true;
        Ok(())
    }

    /// Drop is observation-only and has the same modeled effect as abandon.
    pub fn drop_observation_model_only(
        &mut self,
        key: R24PortableProgressKeyV1,
    ) -> Result<(), R24PortableProgressErrorV1> {
        self.abandon_model_only(key)
    }

    /// Stop disables observation without a final poll or flush pass.
    pub fn stop_model_only(&mut self) {
        self.stopped = true;
        for entry in &mut self.registrations {
            entry.observing = false;
        }
    }

    #[cfg(test)]
    pub(crate) fn set_visit_counts_for_test_model_only(
        &mut self,
        poll_visits: u64,
        flush_visits: u64,
    ) {
        self.poll_visits = poll_visits;
        self.flush_visits = flush_visits;
    }

    pub fn validate_global_invariants(&self) -> Result<(), R24PortableProgressErrorV1> {
        if self.config.capacity == 0
            || self.config.capacity > R24_MAX_REGISTRATIONS_V1
            || self.config.poll_budget == 0
            || self.config.poll_budget > R24_MAX_PROGRESS_BUDGET_V1
            || self.config.flush_budget == 0
            || self.config.flush_budget > R24_MAX_PROGRESS_BUDGET_V1
            || self.registrations.len() > R24_MAX_REGISTRATIONS_V1
            || self.active_registration_count() > self.config.capacity
            || (!self.registrations.is_empty()
                && (self.poll_cursor >= self.registrations.len()
                    || self.flush_cursor >= self.registrations.len()))
            || self.registrations.iter().enumerate().any(|(index, entry)| {
                !entry.key.is_valid()
                    || entry.total_packets == 0
                    || entry.total_packets > R24_MAX_TRANSFER_PACKETS_V1
                    || !entry.event_installed
                    || !entry.stream_installed
                    || !entry.custody_retained
                    || (entry.abandoned && entry.observing)
                    || (self.stopped && entry.observing)
                    || self.registrations[..index].iter().any(|prior| {
                        prior.observing
                            && entry.observing
                            && (same_event_identity_v1(prior.key, entry.key)
                                || same_stream_identity_v1(prior.key, entry.key))
                    })
                    || !valid_phase_v1(*entry)
            })
        {
            return Err(R24PortableProgressErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn active_registration_count(&self) -> usize {
        self.registrations
            .iter()
            .filter(|entry| entry.observing)
            .count()
    }
}

const fn same_event_identity_v1(
    left: R24PortableProgressKeyV1,
    right: R24PortableProgressKeyV1,
) -> bool {
    left.context_generation == right.context_generation && left.event_id == right.event_id
}

const fn same_stream_identity_v1(
    left: R24PortableProgressKeyV1,
    right: R24PortableProgressKeyV1,
) -> bool {
    left.context_generation == right.context_generation && left.stream_id == right.stream_id
}

fn cyclic_indices_v1(
    registrations: &[R24PortableRegistrationSnapshotV1],
    cursor: usize,
    requested: usize,
    eligible: impl Fn(&R24PortableRegistrationSnapshotV1) -> bool,
) -> Result<Vec<usize>, R24PortableProgressErrorV1> {
    if requested == 0 {
        return Ok(Vec::new());
    }
    if registrations.is_empty() || cursor >= registrations.len() {
        return Err(R24PortableProgressErrorV1::InvariantViolation);
    }
    let mut indices = Vec::with_capacity(requested);
    for distance in 0..registrations.len() {
        let index = (cursor + distance) % registrations.len();
        if eligible(&registrations[index]) {
            indices.push(index);
            if indices.len() == requested {
                return Ok(indices);
            }
        }
    }
    Err(R24PortableProgressErrorV1::BudgetExceeded)
}

const fn valid_phase_v1(entry: R24PortableRegistrationSnapshotV1) -> bool {
    match entry.phase {
        R24ProgressPhaseV1::WindowPending {
            ordinal,
            packet_count,
        } => {
            (ordinal == 0
                && packet_count
                    == if entry.total_packets < R24_MAX_WINDOW_PACKETS_V1 {
                        entry.total_packets
                    } else {
                        R24_MAX_WINDOW_PACKETS_V1
                    })
                || (ordinal == 1
                    && entry.total_packets > R24_MAX_WINDOW_PACKETS_V1
                    && packet_count == entry.total_packets - R24_MAX_WINDOW_PACKETS_V1)
        }
        R24ProgressPhaseV1::ContinuationReady {
            completed_packets,
            next_packet_count,
            polled_before_continuation,
        } => {
            entry.total_packets > R24_MAX_WINDOW_PACKETS_V1
                && completed_packets == R24_MAX_WINDOW_PACKETS_V1
                && next_packet_count == entry.total_packets - completed_packets
                && polled_before_continuation
        }
        R24ProgressPhaseV1::TerminalSucceeded | R24ProgressPhaseV1::TerminalQuarantined => {
            !entry.observing
        }
    }
}
