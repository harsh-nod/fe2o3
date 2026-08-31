//! Bounded, authority-free reusable-queue and nonblocking completion model.
//!
//! This layer owns no KFD descriptor, queue, signal, mapping, packet, or
//! completion authority. Its observations are caller-constructible. It models
//! the linear tokens and resource-retention rules that a future sealed native
//! adapter must refine before these transitions can participate in execution.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const ASYNC_QUEUE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_ASYNC_QUEUE_IN_FLIGHT_V1: usize = 8_192;

/// Exact addressless resources retained for one modeled operation.
///
/// The kernarg and completion mappings are exclusive to the operation. Data
/// mappings are canonical and may be shared concurrently only for read access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncOperationResourcesV1 {
    code: LoadedCodeKeyV1,
    kernarg: MappingKeyV1,
    completion_signal: MappingKeyV1,
    data: Vec<DispatchResourceV1>,
}

impl AsyncOperationResourcesV1 {
    pub fn new(
        code: LoadedCodeKeyV1,
        kernarg: MappingKeyV1,
        completion_signal: MappingKeyV1,
        data: Vec<DispatchResourceV1>,
    ) -> Result<Self, AsyncQueueErrorV1> {
        if kernarg == completion_signal
            || data.iter().any(|resource| {
                resource.mapping == kernarg || resource.mapping == completion_signal
            })
        {
            return Err(AsyncQueueErrorV1::ResourceRoleCollision);
        }
        if data.len() > MAX_DISPATCH_RESOURCES_V1.saturating_sub(2)
            || data
                .windows(2)
                .any(|pair| pair[0].mapping >= pair[1].mapping)
        {
            return Err(AsyncQueueErrorV1::NonCanonicalResources);
        }
        if data
            .iter()
            .any(|resource| resource.required_access == MemoryAccessV1::ReadExecute)
        {
            return Err(AsyncQueueErrorV1::InvalidDataAccess);
        }
        Ok(Self {
            code,
            kernarg,
            completion_signal,
            data,
        })
    }

    pub const fn code(&self) -> LoadedCodeKeyV1 {
        self.code
    }

    pub const fn kernarg(&self) -> MappingKeyV1 {
        self.kernarg
    }

    pub const fn completion_signal(&self) -> MappingKeyV1 {
        self.completion_signal
    }

    pub fn data(&self) -> &[DispatchResourceV1] {
        &self.data
    }

    fn runtime_resources(&self) -> Vec<DispatchResourceV1> {
        let mut resources = Vec::with_capacity(self.data.len() + 2);
        resources.push(DispatchResourceV1 {
            mapping: self.kernarg,
            required_access: MemoryAccessV1::ReadWrite,
        });
        resources.push(DispatchResourceV1 {
            mapping: self.completion_signal,
            required_access: MemoryAccessV1::ReadWrite,
        });
        resources.extend_from_slice(&self.data);
        resources.sort_unstable_by_key(|resource| resource.mapping);
        resources
    }
}

/// Exact reusable slot incarnation assigned to one modeled operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AsyncOperationBindingV1 {
    queue: QueueKeyV1,
    slot_index: u16,
    slot_generation: u64,
    dispatch: DispatchKeyV1,
    completion: CompletionKeyV1,
}

impl AsyncOperationBindingV1 {
    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub const fn slot_index(self) -> u16 {
        self.slot_index
    }

    pub const fn slot_generation(self) -> u64 {
        self.slot_generation
    }

    pub const fn dispatch(self) -> DispatchKeyV1 {
        self.dispatch
    }

    pub const fn completion(self) -> CompletionKeyV1 {
        self.completion
    }
}

/// Modeled phase of one bounded reusable queue slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncQueueSlotPhaseV1 {
    Available,
    Reserved,
    Submitted,
    CompletionObserved,
    Indeterminate,
}

impl AsyncQueueSlotPhaseV1 {
    pub const fn retains_operation(self) -> bool {
        !matches!(self, Self::Available)
    }
}

/// Read-only model record for one reusable slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncQueueSlotRecordV1 {
    index: u16,
    generation: u64,
    phase: AsyncQueueSlotPhaseV1,
    binding: Option<AsyncOperationBindingV1>,
    resources: Option<AsyncOperationResourcesV1>,
    cancellation_requested: bool,
    timeout_observations: u64,
}

impl AsyncQueueSlotRecordV1 {
    pub const fn index(&self) -> u16 {
        self.index
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn phase(&self) -> AsyncQueueSlotPhaseV1 {
        self.phase
    }

    pub const fn binding(&self) -> Option<AsyncOperationBindingV1> {
        self.binding
    }

    pub fn resources(&self) -> Option<&AsyncOperationResourcesV1> {
        self.resources.as_ref()
    }

    pub const fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }

    pub const fn timeout_observations(&self) -> u64 {
        self.timeout_observations
    }
}

/// Failure from constructing a reusable registry, paired with the input model.
#[must_use]
pub struct AsyncQueueCreateFailureV1 {
    error: Box<AsyncQueueErrorV1>,
    runtime: Box<RuntimeStateV1>,
}

impl AsyncQueueCreateFailureV1 {
    pub fn error(&self) -> &AsyncQueueErrorV1 {
        &self.error
    }

    pub fn into_runtime(self) -> RuntimeStateV1 {
        *self.runtime
    }
}

impl core::fmt::Debug for AsyncQueueCreateFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AsyncQueueCreateFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Rejection from a consuming token transition with exact token custody.
#[must_use]
pub struct AsyncTokenTransitionFailureV1<T> {
    error: Box<AsyncQueueErrorV1>,
    retained: Box<T>,
}

impl<T> AsyncTokenTransitionFailureV1<T> {
    pub fn error(&self) -> &AsyncQueueErrorV1 {
        &self.error
    }

    pub fn into_retained(self) -> T {
        *self.retained
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for AsyncTokenTransitionFailureV1<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AsyncTokenTransitionFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

/// Authority-free reusable queue state over the existing runtime model.
///
/// The registry may contain several reserved, submitted, or completed
/// operations at once. Dropping a token never changes this registry, so it
/// cannot release a slot or any modeled resource.
pub struct AsyncQueueRegistryV1 {
    runtime: RuntimeStateV1,
    queue: QueueKeyV1,
    slots: Vec<AsyncQueueSlotRecordV1>,
}

impl core::fmt::Debug for AsyncQueueRegistryV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AsyncQueueRegistryV1")
            .field("queue", &self.queue)
            .field("slots", &self.slots)
            .finish_non_exhaustive()
    }
}

impl AsyncQueueRegistryV1 {
    pub fn new_model_only(
        runtime: RuntimeStateV1,
        queue: QueueKeyV1,
        max_in_flight: usize,
    ) -> Result<Self, AsyncQueueCreateFailureV1> {
        let result = Self::validate_creation(&runtime, queue, max_in_flight);
        if let Err(error) = result {
            return Err(AsyncQueueCreateFailureV1 {
                error: Box::new(error),
                runtime: Box::new(runtime),
            });
        }
        let slots = (0..max_in_flight)
            .map(|index| AsyncQueueSlotRecordV1 {
                index: index as u16,
                generation: 1,
                phase: AsyncQueueSlotPhaseV1::Available,
                binding: None,
                resources: None,
                cancellation_requested: false,
                timeout_observations: 0,
            })
            .collect();
        Ok(Self {
            runtime,
            queue,
            slots,
        })
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn queue(&self) -> QueueKeyV1 {
        self.queue
    }

    pub fn runtime_state(&self) -> &RuntimeStateV1 {
        &self.runtime
    }

    pub fn slots(&self) -> &[AsyncQueueSlotRecordV1] {
        &self.slots
    }

    pub fn max_in_flight(&self) -> usize {
        self.slots.len()
    }

    pub fn retained_operation_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.phase.retains_operation())
            .count()
    }

    pub fn available_slot_count(&self) -> usize {
        self.slots.len() - self.retained_operation_count()
    }

    /// Returns the underlying model only when no slot retains an operation.
    pub fn into_runtime_state(self) -> Result<RuntimeStateV1, Box<Self>> {
        if self.retained_operation_count() == 0 {
            Ok(self.runtime)
        } else {
            Err(Box::new(self))
        }
    }

    /// Reserves one bounded slot and records exact model resource retention.
    ///
    /// This performs no native reservation or publication and grants no launch
    /// authority. The returned token is deliberately move-only.
    pub fn reserve_model_only(
        &mut self,
        dispatch: DispatchKeyV1,
        completion: CompletionKeyV1,
        resources: AsyncOperationResourcesV1,
    ) -> Result<AsyncReservedOperationTokenV1, AsyncQueueErrorV1> {
        if dispatch.queue != self.queue {
            return Err(AsyncQueueErrorV1::QueueMismatch);
        }
        if completion.dispatch != dispatch {
            return Err(AsyncQueueErrorV1::CompletionMismatch);
        }
        self.validate_resource_vm(&resources)?;
        self.validate_resource_compatibility(&resources)?;
        let slot_index = self
            .slots
            .iter()
            .position(|slot| slot.phase == AsyncQueueSlotPhaseV1::Available)
            .ok_or(AsyncQueueErrorV1::QueueFull)?;
        let slot_generation = self.slots[slot_index].generation;
        let next_runtime = self
            .runtime
            .next(RuntimeTransitionV1::PrepareDispatch {
                key: dispatch,
                code: resources.code,
                completion,
                resources: resources.runtime_resources(),
            })
            .map_err(AsyncQueueErrorV1::Runtime)?;
        let binding = AsyncOperationBindingV1 {
            queue: self.queue,
            slot_index: slot_index as u16,
            slot_generation,
            dispatch,
            completion,
        };
        self.runtime = next_runtime;
        let slot = &mut self.slots[slot_index];
        slot.phase = AsyncQueueSlotPhaseV1::Reserved;
        slot.binding = Some(binding);
        slot.resources = Some(resources);
        slot.cancellation_requested = false;
        slot.timeout_observations = 0;
        Ok(AsyncReservedOperationTokenV1 { binding })
    }

    pub fn validate_global_invariants(&self) -> Result<(), AsyncQueueInvariantViolationV1> {
        self.runtime
            .validate_global_invariants()
            .map_err(AsyncQueueInvariantViolationV1::Runtime)?;
        if self.slots.is_empty() || self.slots.len() > MAX_ASYNC_QUEUE_IN_FLIGHT_V1 {
            return Err(AsyncQueueInvariantViolationV1::InvalidCapacity);
        }
        let queue = self
            .runtime
            .queues()
            .iter()
            .find(|record| record.key == self.queue)
            .ok_or(AsyncQueueInvariantViolationV1::QueueMissing)?;
        if queue.state != QueueStateV1::Ready || self.slots.len() > queue.capacity as usize {
            return Err(AsyncQueueInvariantViolationV1::QueueUnavailable);
        }
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.index as usize != index || slot.generation == 0 {
                return Err(AsyncQueueInvariantViolationV1::InvalidSlot(index));
            }
            match slot.phase {
                AsyncQueueSlotPhaseV1::Available => {
                    if slot.binding.is_some()
                        || slot.resources.is_some()
                        || slot.cancellation_requested
                        || slot.timeout_observations != 0
                    {
                        return Err(AsyncQueueInvariantViolationV1::InvalidSlot(index));
                    }
                }
                phase => self.validate_retained_slot(index, phase, slot)?,
            }
        }
        for left in 0..self.slots.len() {
            let Some(left_resources) = self.slots[left].resources.as_ref() else {
                continue;
            };
            for right in left + 1..self.slots.len() {
                let Some(right_resources) = self.slots[right].resources.as_ref() else {
                    continue;
                };
                if resource_sets_conflict(left_resources, right_resources) {
                    return Err(AsyncQueueInvariantViolationV1::ResourceConflict);
                }
            }
        }
        Ok(())
    }

    fn validate_creation(
        runtime: &RuntimeStateV1,
        queue: QueueKeyV1,
        max_in_flight: usize,
    ) -> Result<(), AsyncQueueErrorV1> {
        runtime
            .validate_global_invariants()
            .map_err(AsyncQueueErrorV1::RuntimeInvariant)?;
        let record = runtime
            .queues()
            .iter()
            .find(|record| record.key == queue)
            .ok_or(AsyncQueueErrorV1::QueueNotFound)?;
        if record.state != QueueStateV1::Ready {
            return Err(AsyncQueueErrorV1::QueueNotReady);
        }
        if max_in_flight == 0
            || max_in_flight > MAX_ASYNC_QUEUE_IN_FLIGHT_V1
            || max_in_flight > record.capacity as usize
        {
            return Err(AsyncQueueErrorV1::InvalidCapacity);
        }
        if runtime
            .dispatches()
            .iter()
            .any(|dispatch| dispatch.key.queue == queue && dispatch.state.retains_resources())
        {
            return Err(AsyncQueueErrorV1::QueueAlreadyRetainsOperations);
        }
        Ok(())
    }

    fn validate_resource_vm(
        &self,
        resources: &AsyncOperationResourcesV1,
    ) -> Result<(), AsyncQueueErrorV1> {
        let vm = self.queue.vm;
        if resources.code.vm != vm
            || resources.kernarg.allocation.vm != vm
            || resources.completion_signal.allocation.vm != vm
            || resources
                .data
                .iter()
                .any(|resource| resource.mapping.allocation.vm != vm)
        {
            return Err(AsyncQueueErrorV1::VmMismatch);
        }
        Ok(())
    }

    fn validate_resource_compatibility(
        &self,
        candidate: &AsyncOperationResourcesV1,
    ) -> Result<(), AsyncQueueErrorV1> {
        if self
            .slots
            .iter()
            .filter_map(|slot| slot.resources.as_ref())
            .any(|active| resource_sets_conflict(active, candidate))
        {
            return Err(AsyncQueueErrorV1::ResourceConflict);
        }
        Ok(())
    }

    fn validate_retained_slot(
        &self,
        index: usize,
        phase: AsyncQueueSlotPhaseV1,
        slot: &AsyncQueueSlotRecordV1,
    ) -> Result<(), AsyncQueueInvariantViolationV1> {
        let binding = slot
            .binding
            .ok_or(AsyncQueueInvariantViolationV1::InvalidSlot(index))?;
        let resources = slot
            .resources
            .as_ref()
            .ok_or(AsyncQueueInvariantViolationV1::InvalidSlot(index))?;
        if binding.queue != self.queue
            || binding.slot_index as usize != index
            || binding.slot_generation != slot.generation
            || binding.completion.dispatch != binding.dispatch
        {
            return Err(AsyncQueueInvariantViolationV1::InvalidSlot(index));
        }
        if phase != AsyncQueueSlotPhaseV1::Submitted
            && (slot.cancellation_requested || slot.timeout_observations != 0)
        {
            return Err(AsyncQueueInvariantViolationV1::InvalidSlot(index));
        }
        let dispatch = self
            .runtime
            .dispatches()
            .iter()
            .find(|record| record.key == binding.dispatch)
            .ok_or(AsyncQueueInvariantViolationV1::DispatchMissing(index))?;
        let completion = self
            .runtime
            .completions()
            .iter()
            .find(|record| record.key == binding.completion)
            .ok_or(AsyncQueueInvariantViolationV1::CompletionMissing(index))?;
        let expected = match phase {
            AsyncQueueSlotPhaseV1::Reserved => {
                (DispatchStateV1::Prepared, CompletionStateV1::Armed)
            }
            AsyncQueueSlotPhaseV1::Submitted => {
                (DispatchStateV1::Published, CompletionStateV1::Armed)
            }
            AsyncQueueSlotPhaseV1::CompletionObserved => {
                (DispatchStateV1::Completed, CompletionStateV1::Observed)
            }
            AsyncQueueSlotPhaseV1::Indeterminate => {
                (DispatchStateV1::Ambiguous, CompletionStateV1::Ambiguous)
            }
            AsyncQueueSlotPhaseV1::Available => unreachable!(),
        };
        if dispatch.code != resources.code
            || dispatch.completion != binding.completion
            || dispatch.resources != resources.runtime_resources()
            || (dispatch.state, completion.state) != expected
        {
            return Err(AsyncQueueInvariantViolationV1::RuntimeBindingMismatch(
                index,
            ));
        }
        Ok(())
    }

    fn validate_token(
        &self,
        binding: AsyncOperationBindingV1,
        phase: AsyncQueueSlotPhaseV1,
    ) -> Result<usize, AsyncQueueErrorV1> {
        if binding.queue != self.queue {
            return Err(AsyncQueueErrorV1::QueueMismatch);
        }
        let index = binding.slot_index as usize;
        let slot = self
            .slots
            .get(index)
            .ok_or(AsyncQueueErrorV1::TokenMismatch)?;
        if slot.phase != phase
            || slot.generation != binding.slot_generation
            || slot.binding != Some(binding)
        {
            return Err(AsyncQueueErrorV1::TokenMismatch);
        }
        Ok(index)
    }

    fn publish(&mut self, binding: AsyncOperationBindingV1) -> Result<(), AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::Reserved)?;
        let next_runtime = self
            .runtime
            .next(RuntimeTransitionV1::PublishDispatch {
                completion: binding.completion,
            })
            .map_err(AsyncQueueErrorV1::Runtime)?;
        self.runtime = next_runtime;
        self.slots[index].phase = AsyncQueueSlotPhaseV1::Submitted;
        Ok(())
    }

    fn cancel_before_publication(
        &mut self,
        binding: AsyncOperationBindingV1,
    ) -> Result<(), AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::Reserved)?;
        let next_generation = self.slots[index]
            .generation
            .checked_add(1)
            .ok_or(AsyncQueueErrorV1::SlotGenerationExhausted)?;
        let next_runtime = self
            .runtime
            .next(RuntimeTransitionV1::AbortPrepared {
                completion: binding.completion,
            })
            .map_err(AsyncQueueErrorV1::Runtime)?;
        self.runtime = next_runtime;
        self.release_slot(index, next_generation);
        Ok(())
    }

    fn poll(
        &mut self,
        binding: AsyncOperationBindingV1,
        observation: AsyncCompletionObservationV1,
    ) -> Result<AsyncQueuePollTransitionV1, AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::Submitted)?;
        match observation {
            AsyncCompletionObservationV1::Pending => Ok(AsyncQueuePollTransitionV1::Pending),
            AsyncCompletionObservationV1::Completed => {
                let next_runtime = self
                    .runtime
                    .next(RuntimeTransitionV1::ObserveCompletion {
                        completion: binding.completion,
                    })
                    .map_err(AsyncQueueErrorV1::Runtime)?;
                self.runtime = next_runtime;
                let slot = &mut self.slots[index];
                slot.phase = AsyncQueueSlotPhaseV1::CompletionObserved;
                slot.cancellation_requested = false;
                slot.timeout_observations = 0;
                Ok(AsyncQueuePollTransitionV1::Completed)
            }
            AsyncCompletionObservationV1::Indeterminate(reason) => {
                let next_runtime = self
                    .runtime
                    .next(RuntimeTransitionV1::MarkDispatchAmbiguous {
                        completion: binding.completion,
                    })
                    .map_err(AsyncQueueErrorV1::Runtime)?;
                self.runtime = next_runtime;
                let slot = &mut self.slots[index];
                slot.phase = AsyncQueueSlotPhaseV1::Indeterminate;
                slot.cancellation_requested = false;
                slot.timeout_observations = 0;
                Ok(AsyncQueuePollTransitionV1::Indeterminate(reason))
            }
        }
    }

    fn observe_timeout(
        &mut self,
        binding: AsyncOperationBindingV1,
    ) -> Result<u64, AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::Submitted)?;
        let observations = self.slots[index]
            .timeout_observations
            .checked_add(1)
            .ok_or(AsyncQueueErrorV1::ObservationCounterExhausted)?;
        self.slots[index].timeout_observations = observations;
        Ok(observations)
    }

    fn request_cancellation(
        &mut self,
        binding: AsyncOperationBindingV1,
    ) -> Result<bool, AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::Submitted)?;
        let first_request = !self.slots[index].cancellation_requested;
        self.slots[index].cancellation_requested = true;
        Ok(first_request)
    }

    fn recycle_completed(
        &mut self,
        binding: AsyncOperationBindingV1,
    ) -> Result<(), AsyncQueueErrorV1> {
        let index = self.validate_token(binding, AsyncQueueSlotPhaseV1::CompletionObserved)?;
        let next_generation = self.slots[index]
            .generation
            .checked_add(1)
            .ok_or(AsyncQueueErrorV1::SlotGenerationExhausted)?;
        self.release_slot(index, next_generation);
        Ok(())
    }

    fn release_slot(&mut self, index: usize, next_generation: u64) {
        self.slots[index] = AsyncQueueSlotRecordV1 {
            index: index as u16,
            generation: next_generation,
            phase: AsyncQueueSlotPhaseV1::Available,
            binding: None,
            resources: None,
            cancellation_requested: false,
            timeout_observations: 0,
        };
    }
}

/// Model rejection or invariant failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncQueueErrorV1 {
    InvalidCapacity,
    QueueNotFound,
    QueueNotReady,
    QueueAlreadyRetainsOperations,
    QueueMismatch,
    CompletionMismatch,
    VmMismatch,
    QueueFull,
    NonCanonicalResources,
    InvalidDataAccess,
    ResourceRoleCollision,
    ResourceConflict,
    TokenMismatch,
    SlotGenerationExhausted,
    ObservationCounterExhausted,
    RuntimeInvariant(InvariantViolationV1),
    Runtime(TransitionErrorV1),
}

/// Detectable corruption of the private reusable-queue model state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncQueueInvariantViolationV1 {
    Runtime(InvariantViolationV1),
    InvalidCapacity,
    QueueMissing,
    QueueUnavailable,
    InvalidSlot(usize),
    DispatchMissing(usize),
    CompletionMissing(usize),
    RuntimeBindingMismatch(usize),
    ResourceConflict,
}

/// Caller-constructible completion observation for model execution only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncCompletionObservationV1 {
    Pending,
    Completed,
    Indeterminate(AsyncIndeterminateReasonV1),
}

/// Descriptive reason why a published operation can no longer be classified.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncIndeterminateReasonV1 {
    CompletionFault,
    QueueFault,
    DeviceCurrentnessLost,
    ObservationUnavailable,
}

enum AsyncQueuePollTransitionV1 {
    Pending,
    Completed,
    Indeterminate(AsyncIndeterminateReasonV1),
}

/// Move-only reservation before modeled publication.
///
/// ```compile_fail
/// use fe2o3_runtime_model::AsyncReservedOperationTokenV1;
/// fn cannot_clone(token: AsyncReservedOperationTokenV1) {
///     let _ = token.clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "a reservation must be published or cancelled before publication"]
pub struct AsyncReservedOperationTokenV1 {
    binding: AsyncOperationBindingV1,
}

impl AsyncReservedOperationTokenV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.binding
    }

    pub fn publish_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
    ) -> Result<AsyncSubmittedOperationTokenV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.publish(self.binding) {
            Ok(()) => Ok(AsyncSubmittedOperationTokenV1 {
                binding: self.binding,
            }),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }

    pub fn cancel_before_publication_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
    ) -> Result<AsyncReleasedOperationReceiptV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.cancel_before_publication(self.binding) {
            Ok(()) => Ok(AsyncReleasedOperationReceiptV1 {
                binding: self.binding,
                outcome: AsyncReleasedOperationOutcomeV1::CancelledBeforePublication,
            }),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }
}

/// Move-only custody of one modeled published operation.
///
/// Dropping this token does not alter the registry. Timeout and cancellation
/// request methods return submitted custody and cannot free the slot.
#[derive(Debug)]
#[must_use = "submitted custody must reach completion or remain retained"]
pub struct AsyncSubmittedOperationTokenV1 {
    binding: AsyncOperationBindingV1,
}

impl AsyncSubmittedOperationTokenV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.binding
    }

    pub fn poll_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
        observation: AsyncCompletionObservationV1,
    ) -> Result<AsyncOperationPollV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.poll(self.binding, observation) {
            Ok(AsyncQueuePollTransitionV1::Pending) => Ok(AsyncOperationPollV1::Pending(self)),
            Ok(AsyncQueuePollTransitionV1::Completed) => Ok(AsyncOperationPollV1::Completed(
                AsyncCompletedOperationTokenV1 {
                    binding: self.binding,
                },
            )),
            Ok(AsyncQueuePollTransitionV1::Indeterminate(reason)) => Ok(
                AsyncOperationPollV1::Indeterminate(QuarantinedAsyncOperationV1 {
                    binding: self.binding,
                    reason,
                }),
            ),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }

    pub fn observe_timeout_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
    ) -> Result<AsyncTimedOutOperationV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.observe_timeout(self.binding) {
            Ok(observation_count) => Ok(AsyncTimedOutOperationV1 {
                submitted: self,
                observation_count,
            }),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }

    pub fn request_cancellation_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
    ) -> Result<AsyncCancellationRequestV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.request_cancellation(self.binding) {
            Ok(first_request) => Ok(AsyncCancellationRequestV1 {
                submitted: self,
                first_request,
            }),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }
}

/// Result of one nonblocking modeled completion observation.
#[derive(Debug)]
pub enum AsyncOperationPollV1 {
    Pending(AsyncSubmittedOperationTokenV1),
    Completed(AsyncCompletedOperationTokenV1),
    Indeterminate(QuarantinedAsyncOperationV1),
}

/// Submitted custody paired with a host timeout observation.
///
/// Timeout is not a terminal device fact and exposes no release transition.
#[derive(Debug)]
#[must_use = "timeout retains submitted operation custody"]
pub struct AsyncTimedOutOperationV1 {
    submitted: AsyncSubmittedOperationTokenV1,
    observation_count: u64,
}

impl AsyncTimedOutOperationV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.submitted.binding
    }

    pub const fn observation_count(&self) -> u64 {
        self.observation_count
    }

    pub fn into_submitted(self) -> AsyncSubmittedOperationTokenV1 {
        self.submitted
    }
}

/// Submitted custody paired with a non-terminal cancellation request.
#[derive(Debug)]
#[must_use = "post-publication cancellation retains submitted operation custody"]
pub struct AsyncCancellationRequestV1 {
    submitted: AsyncSubmittedOperationTokenV1,
    first_request: bool,
}

impl AsyncCancellationRequestV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.submitted.binding
    }

    pub const fn is_first_request(&self) -> bool {
        self.first_request
    }

    pub fn into_submitted(self) -> AsyncSubmittedOperationTokenV1 {
        self.submitted
    }
}

/// Move-only observed-completion custody before modeled signal recycle.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{AsyncCompletedOperationTokenV1, AsyncQueueRegistryV1};
/// fn cannot_poll_again(
///     token: AsyncCompletedOperationTokenV1,
///     registry: &mut AsyncQueueRegistryV1,
/// ) {
///     let _ = token.poll_model_only(registry);
/// }
/// ```
#[derive(Debug)]
#[must_use = "observed completion must be recycled before slot reuse"]
pub struct AsyncCompletedOperationTokenV1 {
    binding: AsyncOperationBindingV1,
}

impl AsyncCompletedOperationTokenV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.binding
    }

    pub fn recycle_model_only(
        self,
        registry: &mut AsyncQueueRegistryV1,
    ) -> Result<AsyncReleasedOperationReceiptV1, AsyncTokenTransitionFailureV1<Self>> {
        match registry.recycle_completed(self.binding) {
            Ok(()) => Ok(AsyncReleasedOperationReceiptV1 {
                binding: self.binding,
                outcome: AsyncReleasedOperationOutcomeV1::RecycledAfterCompletion,
            }),
            Err(error) => Err(AsyncTokenTransitionFailureV1 {
                error: Box::new(error),
                retained: Box::new(self),
            }),
        }
    }
}

/// Permanently retained model custody after an indeterminate publication.
///
/// This slice intentionally exposes no retry, recycle, cancellation, or
/// release method. A later quiescence refinement must consume it explicitly.
#[derive(Debug)]
#[must_use = "indeterminate post-publication custody must remain quarantined"]
pub struct QuarantinedAsyncOperationV1 {
    binding: AsyncOperationBindingV1,
    reason: AsyncIndeterminateReasonV1,
}

impl QuarantinedAsyncOperationV1 {
    pub const fn binding(&self) -> AsyncOperationBindingV1 {
        self.binding
    }

    pub const fn reason(&self) -> AsyncIndeterminateReasonV1 {
        self.reason
    }
}

/// Terminal authority-free receipt after a slot can be reused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AsyncReleasedOperationReceiptV1 {
    binding: AsyncOperationBindingV1,
    outcome: AsyncReleasedOperationOutcomeV1,
}

impl AsyncReleasedOperationReceiptV1 {
    pub const fn binding(self) -> AsyncOperationBindingV1 {
        self.binding
    }

    pub const fn outcome(self) -> AsyncReleasedOperationOutcomeV1 {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncReleasedOperationOutcomeV1 {
    CancelledBeforePublication,
    RecycledAfterCompletion,
}

fn resource_sets_conflict(
    left: &AsyncOperationResourcesV1,
    right: &AsyncOperationResourcesV1,
) -> bool {
    let left_resources = left.runtime_resources();
    let right_resources = right.runtime_resources();
    left_resources.iter().any(|left| {
        right_resources.iter().any(|right| {
            left.mapping == right.mapping
                && !(left.required_access == MemoryAccessV1::Read
                    && right.required_access == MemoryAccessV1::Read)
        })
    })
}
