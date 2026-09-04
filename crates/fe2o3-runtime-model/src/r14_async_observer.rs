//! Bounded, executor-neutral observation of exact runtime events.
//!
//! The observer keeps pending event identities in stable order. Registration,
//! polling, abandonment, and shutdown affect observation only; this model has
//! no transition that publishes work, cancels a submission, releases an event,
//! or releases submission resources.
//!
//! This is a caller-constructible pure model. It does not refine the Rust
//! async engine, host threads, wakers, KFD, HSA, HIP, or hardware execution.

use alloc::vec::Vec;

pub const MAX_R14_ASYNC_WAITERS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R14ObservedEventKeyV1 {
    pub context_generation: u64,
    pub event_id: u64,
}

impl R14ObservedEventKeyV1 {
    pub const fn is_valid(self) -> bool {
        self.context_generation != 0 && self.event_id != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R14RuntimeEventStatusV1 {
    Pending,
    Succeeded,
    Failed { code: i64 },
    QuiescentWithoutResult,
}

impl R14RuntimeEventStatusV1 {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R14AsyncObservationV1 {
    Status(R14RuntimeEventStatusV1),
    RuntimeError { code: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R14AsyncOutcomeV1 {
    Runtime(R14RuntimeEventStatusV1),
    RuntimeError { code: i64 },
    EngineStopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R14AsyncRegistrationV1 {
    Pending,
    Ready(R14RuntimeEventStatusV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R14AsyncObserverErrorV1 {
    InvalidIdentity,
    DuplicateEvent,
    CapacityExceeded,
    UnknownEvent,
    EngineStopped,
    InvariantViolation,
}

/// Pure model of one bounded async observer registry.
pub struct R14AsyncObserverModelV1 {
    capacity: usize,
    pending: Vec<R14ObservedEventKeyV1>,
    stopped: bool,
}

impl R14AsyncObserverModelV1 {
    pub fn new_model_only(capacity: usize) -> Result<Self, R14AsyncObserverErrorV1> {
        if capacity == 0 || capacity > MAX_R14_ASYNC_WAITERS_V1 {
            return Err(R14AsyncObserverErrorV1::CapacityExceeded);
        }
        Ok(Self {
            capacity,
            pending: Vec::new(),
            stopped: false,
        })
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn pending_events(&self) -> &[R14ObservedEventKeyV1] {
        &self.pending
    }

    pub fn register_model_only(
        &mut self,
        event: R14ObservedEventKeyV1,
        initial_status: R14RuntimeEventStatusV1,
    ) -> Result<R14AsyncRegistrationV1, R14AsyncObserverErrorV1> {
        if !event.is_valid() {
            return Err(R14AsyncObserverErrorV1::InvalidIdentity);
        }
        if self.stopped {
            return Err(R14AsyncObserverErrorV1::EngineStopped);
        }
        let position = match self.pending.binary_search(&event) {
            Ok(_) => return Err(R14AsyncObserverErrorV1::DuplicateEvent),
            Err(position) => position,
        };
        if initial_status.is_terminal() {
            return Ok(R14AsyncRegistrationV1::Ready(initial_status));
        }
        if self.pending.len() >= self.capacity {
            return Err(R14AsyncObserverErrorV1::CapacityExceeded);
        }
        self.pending.insert(position, event);
        Ok(R14AsyncRegistrationV1::Pending)
    }

    /// Applies one exact backend observation.
    ///
    /// Pending preserves the registration. Every conclusive result removes
    /// exactly that registration and is returned without status substitution.
    pub fn observe_model_only(
        &mut self,
        event: R14ObservedEventKeyV1,
        observation: R14AsyncObservationV1,
    ) -> Result<Option<R14AsyncOutcomeV1>, R14AsyncObserverErrorV1> {
        let position = self
            .pending
            .binary_search(&event)
            .map_err(|_| R14AsyncObserverErrorV1::UnknownEvent)?;
        match observation {
            R14AsyncObservationV1::Status(R14RuntimeEventStatusV1::Pending) => Ok(None),
            R14AsyncObservationV1::Status(status) => {
                debug_assert!(status.is_terminal());
                self.pending.remove(position);
                Ok(Some(R14AsyncOutcomeV1::Runtime(status)))
            }
            R14AsyncObservationV1::RuntimeError { code } => {
                self.pending.remove(position);
                Ok(Some(R14AsyncOutcomeV1::RuntimeError { code }))
            }
        }
    }

    /// Abandons host observation only and returns whether it was registered.
    pub fn abandon_model_only(&mut self, event: R14ObservedEventKeyV1) -> bool {
        let Ok(position) = self.pending.binary_search(&event) else {
            return false;
        };
        self.pending.remove(position);
        true
    }

    /// Stops observation and returns exact stopped outcomes in stable key order.
    pub fn stop_model_only(&mut self) -> Vec<(R14ObservedEventKeyV1, R14AsyncOutcomeV1)> {
        self.stopped = true;
        self.pending
            .drain(..)
            .map(|event| (event, R14AsyncOutcomeV1::EngineStopped))
            .collect()
    }

    pub fn validate_global_invariants(&self) -> Result<(), R14AsyncObserverErrorV1> {
        if self.capacity == 0
            || self.capacity > MAX_R14_ASYNC_WAITERS_V1
            || self.pending.len() > self.capacity
            || self.pending.iter().any(|event| !event.is_valid())
            || self.pending.windows(2).any(|pair| pair[0] >= pair[1])
            || (self.stopped && !self.pending.is_empty())
        {
            return Err(R14AsyncObserverErrorV1::InvariantViolation);
        }
        Ok(())
    }
}
