use crate::Retained;
use std::sync::{Arc, Mutex, MutexGuard};

/// Maximum number of resource sets retained by one quarantine.
pub const MAX_QUARANTINED_OPERATIONS: usize = 4_096;

/// Observable state of one submitted asynchronous operation.
///
/// Only `Completed`, `Failed`, and `StreamQuiesced` establish that the backend
/// can no longer access retained resources. `Quarantined` and `Leaked` are
/// terminal bookkeeping outcomes, but they deliberately grant no reclamation
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationState<E> {
    /// Work may be executing and all resources must remain retained.
    Submitted,
    /// Cancellation was requested, but work may still be executing.
    CancelRequested,
    /// The operation completed and the GPU can no longer access its resources.
    Completed,
    /// The operation failed after the GPU became quiescent.
    Failed(E),
    /// A stronger stream synchronization established quiescence without an
    /// authoritative per-operation completion result.
    StreamQuiesced,
    /// Ownership moved into a bounded quarantine and cannot be reclaimed via
    /// this lifecycle.
    Quarantined(QuarantineTicket),
    /// Ownership could not be quarantined and was intentionally leaked.
    Leaked(LeakReason),
}

impl<E> OperationState<E> {
    /// Returns whether the backend has established that GPU work is quiescent.
    pub fn is_quiescent(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed(_) | Self::StreamQuiesced
        )
    }

    /// Returns the lifecycle's terminal state, if no further transition is valid.
    pub fn terminal_state(&self) -> Option<TerminalState> {
        match self {
            Self::Submitted | Self::CancelRequested => None,
            Self::Completed => Some(TerminalState::Completed),
            Self::Failed(_) => Some(TerminalState::Failed),
            Self::StreamQuiesced => Some(TerminalState::StreamQuiesced),
            Self::Quarantined(_) => Some(TerminalState::Quarantined),
            Self::Leaked(_) => Some(TerminalState::Leaked),
        }
    }
}

/// A terminal lifecycle state that does not borrow payload data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalState {
    Completed,
    Failed,
    StreamQuiesced,
    Quarantined,
    Leaked,
}

/// An attempted transition from an already-terminal operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionError {
    pub terminal: TerminalState,
}

/// Failure to record a cancellation request.
#[derive(Debug, Eq, PartialEq)]
pub enum CancelRequestError<E> {
    /// The backend rejected or failed to submit the cancellation request.
    Request(E),
    /// The lifecycle was already terminal, so the callback was not invoked.
    Terminal(TerminalState),
}

/// Failure while applying a terminal transition and notifying its observer.
#[derive(Debug, Eq, PartialEq)]
pub enum NotificationError<E> {
    /// The operation was already terminal, so the callback was not invoked.
    Transition(TransitionError),
    /// The terminal state was recorded, but the notification callback failed.
    Callback(E),
}

/// Stable identifier for a resource set held by a [`BoundedQuarantine`].
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct QuarantineTicket(u64);

impl QuarantineTicket {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Why non-quiescent resources were intentionally leaked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeakReason {
    /// The configured quarantine capacity was already occupied.
    QuarantineFull,
    /// Reserving storage for the quarantine failed.
    QuarantineAllocationFailed,
    /// The quarantine exhausted its non-repeating ticket space.
    QuarantineTicketExhausted,
}

/// The fail-safe retention outcome for an abandoned non-quiescent operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetentionOutcome {
    Quarantined(QuarantineTicket),
    Leaked(LeakReason),
}

/// An invalid requested bound for a resource quarantine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuarantineCapacityError {
    pub requested: usize,
    pub maximum: usize,
}

/// A bounded holding area for resources whose operation is not known quiescent.
///
/// One quarantine must contain operations from a synchronization domain for
/// which one stronger backend observation can establish quiescence. Dropping a
/// non-empty quarantine intentionally leaks its entries. Releasing entries is
/// unsafe and requires an independent stream-quiescence observation.
#[derive(Debug)]
#[must_use = "dropping a non-empty quarantine intentionally leaks its resources"]
pub struct BoundedQuarantine<R> {
    entries: Vec<Retained<R>>,
    capacity: usize,
    next_ticket: u64,
}

impl<R> BoundedQuarantine<R> {
    pub fn new(capacity: usize) -> Result<Self, QuarantineCapacityError> {
        if capacity > MAX_QUARANTINED_OPERATIONS {
            return Err(QuarantineCapacityError {
                requested: capacity,
                maximum: MAX_QUARANTINED_OPERATIONS,
            });
        }
        Ok(Self {
            entries: Vec::new(),
            capacity,
            next_ticket: 0,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns all quarantined resources after stronger stream quiescence.
    ///
    /// # Safety
    ///
    /// The caller must have established that the backend can no longer access
    /// any resource in this quarantine.
    pub unsafe fn release_after_stream_quiescence(&mut self) -> Vec<R> {
        self.entries
            .drain(..)
            .map(|mut retained| retained.take())
            .collect()
    }

    fn reserve_ticket(&mut self) -> Result<QuarantineTicket, LeakReason> {
        if self.entries.len() >= self.capacity {
            return Err(LeakReason::QuarantineFull);
        }
        let next = self
            .next_ticket
            .checked_add(1)
            .ok_or(LeakReason::QuarantineTicketExhausted)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| LeakReason::QuarantineAllocationFailed)?;
        let ticket = QuarantineTicket(self.next_ticket);
        self.next_ticket = next;
        Ok(ticket)
    }

    fn insert_reserved(&mut self, resources: R) {
        self.entries.push(Retained::new(resources));
    }
}

/// Why retained resources cannot be reclaimed by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReclaimError {
    /// Work may still be executing.
    NotQuiescent,
    /// Ownership moved into the identified quarantine.
    Quarantined(QuarantineTicket),
    /// Ownership was intentionally leaked for safety.
    Leaked(LeakReason),
    /// The resources were already reclaimed exactly once.
    AlreadyReclaimed,
}

/// Runtime-independent lifecycle and retention policy for an owned operation.
///
/// Cancellation is deliberately non-terminal and never authorizes resource
/// reclamation. Backend completion, backend failure after completion, or a
/// stronger stream-quiescence observation permits reclamation. Non-quiescent
/// operations may instead transfer ownership into a bounded quarantine. If no
/// quarantine slot is available, resources are intentionally leaked.
#[derive(Debug)]
#[must_use = "an operation lifecycle retains resources until GPU quiescence"]
pub struct OperationLifecycle<R, E> {
    resources: Retained<R>,
    state: OperationState<E>,
}

impl<R, E> OperationLifecycle<R, E> {
    /// Starts in `Submitted` with all participating resources retained.
    pub fn submitted(resources: R) -> Self {
        Self {
            resources: Retained::new(resources),
            state: OperationState::Submitted,
        }
    }

    pub fn state(&self) -> &OperationState<E> {
        &self.state
    }

    /// Runs the backend cancellation request at most once.
    ///
    /// Success moves `Submitted` to `CancelRequested` and returns `true`.
    /// Repeated requests are idempotent and return `false` without invoking the
    /// callback. An error or panic leaves the operation submitted. Cancellation
    /// never establishes completion.
    pub fn request_cancel_with<C>(
        &mut self,
        request: impl FnOnce() -> Result<(), C>,
    ) -> Result<bool, CancelRequestError<C>> {
        match self.state.terminal_state() {
            Some(terminal) => return Err(CancelRequestError::Terminal(terminal)),
            None if matches!(self.state, OperationState::CancelRequested) => return Ok(false),
            None => {}
        }

        request().map_err(CancelRequestError::Request)?;
        self.state = OperationState::CancelRequested;
        Ok(true)
    }

    /// Records successful completion after the backend establishes quiescence.
    ///
    /// # Safety
    ///
    /// The caller must have established that the GPU can no longer access any
    /// retained resource.
    pub unsafe fn complete(&mut self) -> Result<(), TransitionError> {
        self.transition_to(OperationState::Completed)
    }

    /// Records a device-operation failure after the backend establishes quiescence.
    ///
    /// # Safety
    ///
    /// The caller must have established that the GPU can no longer access any
    /// retained resource.
    pub unsafe fn fail(&mut self, error: E) -> Result<(), TransitionError> {
        self.transition_to(OperationState::Failed(error))
    }

    /// Records stronger stream quiescence when operation status is unavailable.
    ///
    /// # Safety
    ///
    /// The caller must have established that the stream containing this
    /// operation is quiescent and cannot access any retained resource.
    pub unsafe fn mark_stream_quiesced(&mut self) -> Result<(), TransitionError> {
        self.transition_to(OperationState::StreamQuiesced)
    }

    /// Records successful completion and invokes one terminal notification.
    ///
    /// State becomes terminal before the callback runs. A callback error or
    /// panic therefore cannot make resources in-flight again.
    ///
    /// # Safety
    ///
    /// The caller must satisfy the quiescence requirement of [`Self::complete`].
    pub unsafe fn complete_with_notification<C>(
        &mut self,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), NotificationError<C>> {
        // SAFETY: Required by this method's contract.
        unsafe { self.complete() }.map_err(NotificationError::Transition)?;
        notify(&self.state).map_err(NotificationError::Callback)
    }

    /// Records a quiescent operation failure and invokes one notification.
    ///
    /// # Safety
    ///
    /// The caller must satisfy the quiescence requirement of [`Self::fail`].
    pub unsafe fn fail_with_notification<C>(
        &mut self,
        error: E,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), NotificationError<C>> {
        // SAFETY: Required by this method's contract.
        unsafe { self.fail(error) }.map_err(NotificationError::Transition)?;
        notify(&self.state).map_err(NotificationError::Callback)
    }

    /// Records stream quiescence and invokes one notification.
    ///
    /// # Safety
    ///
    /// The caller must satisfy the quiescence requirement of
    /// [`Self::mark_stream_quiesced`].
    pub unsafe fn stream_quiesced_with_notification<C>(
        &mut self,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), NotificationError<C>> {
        // SAFETY: Required by this method's contract.
        unsafe { self.mark_stream_quiesced() }.map_err(NotificationError::Transition)?;
        notify(&self.state).map_err(NotificationError::Callback)
    }

    /// Moves non-quiescent resources into a bounded quarantine or leaks them.
    ///
    /// This is a terminal lifecycle action, but it does not establish backend
    /// quiescence and never makes the resources reclaimable through `self`.
    pub fn abandon_to_quarantine(
        &mut self,
        quarantine: &mut BoundedQuarantine<R>,
    ) -> Result<RetentionOutcome, TransitionError> {
        if let Some(terminal) = self.state.terminal_state() {
            return Err(TransitionError { terminal });
        }

        match quarantine.reserve_ticket() {
            Ok(ticket) => {
                let resources = self
                    .resources
                    .try_take()
                    .expect("non-terminal lifecycle retains resources");
                quarantine.insert_reserved(resources);
                self.state = OperationState::Quarantined(ticket);
                Ok(RetentionOutcome::Quarantined(ticket))
            }
            Err(reason) => {
                self.state = OperationState::Leaked(reason);
                Ok(RetentionOutcome::Leaked(reason))
            }
        }
    }

    /// Returns retained resources exactly once after backend quiescence.
    pub fn reclaim(&mut self) -> Result<R, ReclaimError> {
        match self.state {
            OperationState::Submitted | OperationState::CancelRequested => {
                return Err(ReclaimError::NotQuiescent);
            }
            OperationState::Quarantined(ticket) => {
                return Err(ReclaimError::Quarantined(ticket));
            }
            OperationState::Leaked(reason) => return Err(ReclaimError::Leaked(reason)),
            OperationState::Completed
            | OperationState::Failed(_)
            | OperationState::StreamQuiesced => {}
        }
        self.resources
            .try_take()
            .ok_or(ReclaimError::AlreadyReclaimed)
    }

    fn transition_to(&mut self, next: OperationState<E>) -> Result<(), TransitionError> {
        if let Some(terminal) = self.state.terminal_state() {
            return Err(TransitionError { terminal });
        }
        self.state = next;
        Ok(())
    }
}

impl<R, E> Drop for OperationLifecycle<R, E> {
    fn drop(&mut self) {
        if self.state.is_quiescent()
            && let Some(resources) = self.resources.try_take()
        {
            drop(resources);
        }
    }
}

/// A poisoned concurrent lifecycle always fails closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoisonedLifecycle;

/// Failure to lock a concurrent lifecycle or apply a lifecycle operation.
#[derive(Debug, Eq, PartialEq)]
pub enum SynchronizedLifecycleError<E> {
    Poisoned,
    Lifecycle(E),
}

/// Linearizable shared access to one [`OperationLifecycle`].
///
/// The first terminal observation wins. Duplicate or racing observations see
/// that same terminal state and cannot invoke another terminal callback. User
/// terminal callbacks run after the transition lock is released. Cancellation
/// submission executes under the lock so it remains at-most-once; a panic in
/// that callback poisons the lifecycle and all later access fails closed.
#[derive(Debug)]
pub struct ConcurrentOperationLifecycle<R, E> {
    inner: Arc<Mutex<OperationLifecycle<R, E>>>,
}

impl<R, E> Clone for ConcurrentOperationLifecycle<R, E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<R, E> ConcurrentOperationLifecycle<R, E> {
    pub fn submitted(resources: R) -> Self {
        Self {
            inner: Arc::new(Mutex::new(OperationLifecycle::submitted(resources))),
        }
    }

    pub fn state(&self) -> Result<OperationState<E>, PoisonedLifecycle>
    where
        E: Clone,
    {
        self.lock().map(|lifecycle| lifecycle.state.clone())
    }

    /// Inspects state while holding the lifecycle mutex.
    ///
    /// A panic in `inspect` poisons subsequent access. This is primarily useful
    /// for integrations that must derive an atomic status record.
    pub fn inspect<T>(
        &self,
        inspect: impl FnOnce(&OperationState<E>) -> T,
    ) -> Result<T, PoisonedLifecycle> {
        self.lock().map(|lifecycle| inspect(&lifecycle.state))
    }

    pub fn request_cancel_with<C>(
        &self,
        request: impl FnOnce() -> Result<(), C>,
    ) -> Result<bool, SynchronizedLifecycleError<CancelRequestError<C>>> {
        self.lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?
            .request_cancel_with(request)
            .map_err(SynchronizedLifecycleError::Lifecycle)
    }

    /// # Safety
    ///
    /// The caller must satisfy [`OperationLifecycle::complete`].
    pub unsafe fn complete(&self) -> Result<(), SynchronizedLifecycleError<TransitionError>> {
        let mut lifecycle = self
            .lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
        // SAFETY: Required by this method's contract.
        unsafe { lifecycle.complete() }.map_err(SynchronizedLifecycleError::Lifecycle)
    }

    /// # Safety
    ///
    /// The caller must satisfy [`OperationLifecycle::fail`].
    pub unsafe fn fail(&self, error: E) -> Result<(), SynchronizedLifecycleError<TransitionError>> {
        let mut lifecycle = self
            .lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
        // SAFETY: Required by this method's contract.
        unsafe { lifecycle.fail(error) }.map_err(SynchronizedLifecycleError::Lifecycle)
    }

    /// # Safety
    ///
    /// The caller must satisfy [`OperationLifecycle::mark_stream_quiesced`].
    pub unsafe fn mark_stream_quiesced(
        &self,
    ) -> Result<(), SynchronizedLifecycleError<TransitionError>> {
        let mut lifecycle = self
            .lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
        // SAFETY: Required by this method's contract.
        unsafe { lifecycle.mark_stream_quiesced() }.map_err(SynchronizedLifecycleError::Lifecycle)
    }

    /// # Safety
    ///
    /// The caller must satisfy [`OperationLifecycle::complete_with_notification`].
    pub unsafe fn complete_with_notification<C>(
        &self,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), SynchronizedLifecycleError<NotificationError<C>>>
    where
        E: Clone,
    {
        let state = {
            let mut lifecycle = self
                .lock()
                .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
            // SAFETY: Required by this method's contract.
            unsafe { lifecycle.complete() }.map_err(|error| {
                SynchronizedLifecycleError::Lifecycle(NotificationError::Transition(error))
            })?;
            lifecycle.state.clone()
        };
        notify(&state).map_err(|error| {
            SynchronizedLifecycleError::Lifecycle(NotificationError::Callback(error))
        })
    }

    /// # Safety
    ///
    /// The caller must satisfy [`OperationLifecycle::fail_with_notification`].
    pub unsafe fn fail_with_notification<C>(
        &self,
        error: E,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), SynchronizedLifecycleError<NotificationError<C>>>
    where
        E: Clone,
    {
        let state = {
            let mut lifecycle = self
                .lock()
                .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
            // SAFETY: Required by this method's contract.
            unsafe { lifecycle.fail(error) }.map_err(|error| {
                SynchronizedLifecycleError::Lifecycle(NotificationError::Transition(error))
            })?;
            lifecycle.state.clone()
        };
        notify(&state).map_err(|error| {
            SynchronizedLifecycleError::Lifecycle(NotificationError::Callback(error))
        })
    }

    /// # Safety
    ///
    /// The caller must satisfy
    /// [`OperationLifecycle::stream_quiesced_with_notification`].
    pub unsafe fn stream_quiesced_with_notification<C>(
        &self,
        notify: impl FnOnce(&OperationState<E>) -> Result<(), C>,
    ) -> Result<(), SynchronizedLifecycleError<NotificationError<C>>>
    where
        E: Clone,
    {
        let state = {
            let mut lifecycle = self
                .lock()
                .map_err(|_| SynchronizedLifecycleError::Poisoned)?;
            // SAFETY: Required by this method's contract.
            unsafe { lifecycle.mark_stream_quiesced() }.map_err(|error| {
                SynchronizedLifecycleError::Lifecycle(NotificationError::Transition(error))
            })?;
            lifecycle.state.clone()
        };
        notify(&state).map_err(|error| {
            SynchronizedLifecycleError::Lifecycle(NotificationError::Callback(error))
        })
    }

    pub fn abandon_to_quarantine(
        &self,
        quarantine: &mut BoundedQuarantine<R>,
    ) -> Result<RetentionOutcome, SynchronizedLifecycleError<TransitionError>> {
        self.lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?
            .abandon_to_quarantine(quarantine)
            .map_err(SynchronizedLifecycleError::Lifecycle)
    }

    /// Returns retained resources exactly once after backend quiescence.
    pub fn reclaim(&self) -> Result<R, SynchronizedLifecycleError<ReclaimError>> {
        self.lock()
            .map_err(|_| SynchronizedLifecycleError::Poisoned)?
            .reclaim()
            .map_err(SynchronizedLifecycleError::Lifecycle)
    }

    fn lock(&self) -> Result<MutexGuard<'_, OperationLifecycle<R, E>>, PoisonedLifecycle> {
        self.inner.lock().map_err(|_| PoisonedLifecycle)
    }
}
