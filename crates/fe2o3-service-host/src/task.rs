use fe2o3_host_api::{
    CompletionObservationV1, CompletionRecordIdV1, CompletionSignalIdV1, CompletionStatusV1,
    DispatchKindV1, DispatchOutcomeV1, DispatchRequestIdV1, DispatchRequestV1, DispatchResultIdV1,
    DispatchResultV1, DispatchSubmissionIdV1, WaitOutcomeV1, WaitRequestIdV1, WaitRequestV1,
    WaitResultIdV1, WaitResultV1,
};
use fe2o3_service_model::TaskIdV1;

use crate::binding::mismatch;
use crate::lifecycle::{RunningServiceV1, TicketBrandV1};
use crate::{BindingFieldV1, QueueSlotBindingV1, ServiceHostErrorV1, ServiceKeyV1};

/// Move-only task ticket branded by a live running-service borrow.
///
/// The ticket is an inert structural record, not a runtime handle. Waiting or
/// dropping it performs no operation.
#[derive(Debug)]
#[must_use = "a task ticket must receive a classified wait observation"]
pub struct TaskTicketV1<'service, 'record> {
    _service_brand: &'service TicketBrandV1,
    key: ServiceKeyV1,
    queue_capacity: u16,
    generation_modulus: u64,
    task_id: TaskIdV1,
    task_tag: u32,
    slot: QueueSlotBindingV1,
    request: &'record DispatchRequestV1,
    result: &'record DispatchResultV1,
    submission_identity: DispatchSubmissionIdV1,
    completion_signal_identity: CompletionSignalIdV1,
}

impl<'service, 'record> TaskTicketV1<'service, 'record> {
    /// Returns the exact service key that branded the ticket.
    pub const fn service_key(&self) -> ServiceKeyV1 {
        self.key
    }

    /// Returns the canonical task identity.
    pub const fn task_id(&self) -> TaskIdV1 {
        self.task_id
    }

    /// Returns the canonical closed-schema task tag.
    pub const fn task_tag(&self) -> u32 {
        self.task_tag
    }

    /// Returns the exact queue slot and generation binding.
    pub const fn slot_binding(&self) -> QueueSlotBindingV1 {
        self.slot
    }

    /// Returns the exact dispatch request commitment.
    pub const fn dispatch_request_identity(&self) -> DispatchRequestIdV1 {
        self.request.identity()
    }

    /// Returns the exact dispatch result commitment.
    pub const fn dispatch_result_identity(&self) -> DispatchResultIdV1 {
        self.result.identity()
    }

    /// Returns the accepted submission-description commitment.
    pub const fn submission_identity(&self) -> DispatchSubmissionIdV1 {
        self.submission_identity
    }

    /// Returns the inert completion-signal commitment.
    pub const fn completion_signal_identity(&self) -> CompletionSignalIdV1 {
        self.completion_signal_identity
    }

    /// Checks that a queue observation still names this exact ticket epoch.
    pub fn validate_current(&self, current: QueueSlotBindingV1) -> Result<(), ServiceHostErrorV1> {
        let slot = current.slot();
        if slot.run_id != self.key.service_run_id() {
            return Err(mismatch(BindingFieldV1::ServiceRun));
        }
        if slot.service_epoch != self.key.service_epoch() {
            return Err(mismatch(BindingFieldV1::ServiceEpoch));
        }
        if slot.queue_identity != self.key.queue_identity() {
            return Err(mismatch(BindingFieldV1::QueueIdentity));
        }
        if slot.slot_id != self.slot.slot().slot_id || slot.slot_id.0 >= self.queue_capacity {
            return Err(mismatch(BindingFieldV1::Slot));
        }
        if current.encoded_generation() != slot.logical_generation % self.generation_modulus {
            return Err(mismatch(BindingFieldV1::Generation));
        }
        if current.queue_epoch() != self.slot.queue_epoch()
            || slot.logical_generation != self.slot.slot().logical_generation
            || current.encoded_generation() != self.slot.encoded_generation()
        {
            return Err(ServiceHostErrorV1::StaleTicket);
        }
        Ok(())
    }

    /// Consumes the ticket after validating one exact canonical wait result.
    ///
    /// This method observes records only; it does not block, poll, or execute.
    pub fn wait(
        self,
        current: QueueSlotBindingV1,
        request: &WaitRequestV1,
        result: &WaitResultV1,
    ) -> Result<TaskCompletionV1, TaskWaitRejectedV1<'service, 'record>> {
        let observation = match self.validate_wait(current, request, result) {
            Ok(observation) => observation,
            Err(error) => {
                return Err(TaskWaitRejectedV1 {
                    error,
                    ticket: self,
                });
            }
        };
        Ok(TaskCompletionV1 {
            key: self.key,
            task_id: self.task_id,
            task_tag: self.task_tag,
            slot: self.slot,
            dispatch_request_identity: self.request.identity(),
            dispatch_result_identity: self.result.identity(),
            wait_request_identity: request.identity(),
            wait_result_identity: result.identity(),
            completion_record_identity: observation.record_identity(),
            status: observation.status(),
        })
    }

    fn validate_wait(
        &self,
        current: QueueSlotBindingV1,
        request: &WaitRequestV1,
        result: &WaitResultV1,
    ) -> Result<CompletionObservationV1, ServiceHostErrorV1> {
        self.validate_current(current)?;
        if request.targets() != [self.completion_signal_identity] {
            return Err(mismatch(BindingFieldV1::CompletionSignal));
        }
        request
            .validate_dispatch_results(core::slice::from_ref(self.result))
            .map_err(|_| mismatch(BindingFieldV1::DispatchResult))?;
        if result.request_identity() != request.identity() || result.mode() != request.mode() {
            return Err(mismatch(BindingFieldV1::WaitRequest));
        }
        let WaitOutcomeV1::Satisfied(observations) = result.outcome() else {
            return Err(ServiceHostErrorV1::WaitNotSatisfied);
        };
        let Some(observation) = observations.first().copied() else {
            return Err(mismatch(BindingFieldV1::CompletionObservation));
        };
        if observations.len() != 1
            || observation.signal_identity() != self.completion_signal_identity
        {
            return Err(mismatch(BindingFieldV1::CompletionObservation));
        }
        Ok(observation)
    }
}

/// Rejected wait paired with the still-live move-only ticket.
#[derive(Debug)]
pub struct TaskWaitRejectedV1<'service, 'record> {
    error: ServiceHostErrorV1,
    ticket: TaskTicketV1<'service, 'record>,
}

impl<'service, 'record> TaskWaitRejectedV1<'service, 'record> {
    /// Returns the structural rejection.
    pub const fn error(&self) -> ServiceHostErrorV1 {
        self.error
    }

    /// Recovers the ticket so a failed observation cannot silently discharge it.
    pub fn into_ticket(self) -> TaskTicketV1<'service, 'record> {
        self.ticket
    }

    /// Splits the rejection and retained ticket.
    pub fn into_parts(self) -> (ServiceHostErrorV1, TaskTicketV1<'service, 'record>) {
        (self.error, self.ticket)
    }
}

/// Terminal task observation detached from the running-service borrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskCompletionV1 {
    key: ServiceKeyV1,
    task_id: TaskIdV1,
    task_tag: u32,
    slot: QueueSlotBindingV1,
    dispatch_request_identity: DispatchRequestIdV1,
    dispatch_result_identity: DispatchResultIdV1,
    wait_request_identity: WaitRequestIdV1,
    wait_result_identity: WaitResultIdV1,
    completion_record_identity: CompletionRecordIdV1,
    status: CompletionStatusV1,
}

impl TaskCompletionV1 {
    /// Returns the exact service key.
    pub const fn service_key(self) -> ServiceKeyV1 {
        self.key
    }

    /// Returns the canonical task identity.
    pub const fn task_id(self) -> TaskIdV1 {
        self.task_id
    }

    /// Returns the canonical closed-schema task tag.
    pub const fn task_tag(self) -> u32 {
        self.task_tag
    }

    /// Returns the exact terminal slot generation binding.
    pub const fn slot_binding(self) -> QueueSlotBindingV1 {
        self.slot
    }

    /// Returns the exact dispatch request commitment.
    pub const fn dispatch_request_identity(self) -> DispatchRequestIdV1 {
        self.dispatch_request_identity
    }

    /// Returns the exact dispatch result commitment.
    pub const fn dispatch_result_identity(self) -> DispatchResultIdV1 {
        self.dispatch_result_identity
    }

    /// Returns the exact wait request commitment.
    pub const fn wait_request_identity(self) -> WaitRequestIdV1 {
        self.wait_request_identity
    }

    /// Returns the exact wait result commitment.
    pub const fn wait_result_identity(self) -> WaitResultIdV1 {
        self.wait_result_identity
    }

    /// Returns the observed completion-record commitment.
    pub const fn completion_record_identity(self) -> CompletionRecordIdV1 {
        self.completion_record_identity
    }

    /// Returns success, cancellation, or failure without promoting claims.
    pub const fn status(self) -> CompletionStatusV1 {
        self.status
    }
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    RunningServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Describes one submitted persistent task and returns a branded ticket.
    ///
    /// The supplied host records must already describe a successful dispatch.
    /// This method performs no queue publication or runtime submission.
    pub fn submit<'service, 'record>(
        &'service self,
        task_id: TaskIdV1,
        slot: QueueSlotBindingV1,
        request: &'record DispatchRequestV1,
        result: &'record DispatchResultV1,
    ) -> Result<TaskTicketV1<'service, 'record>, ServiceHostErrorV1> {
        slot.validate_for(self.contract)?;
        let key = self.contract.key();
        if request.loaded_object_identity() != key.loaded_object_identity() {
            return Err(mismatch(BindingFieldV1::LoadedObject));
        }
        if request.load_generation() != key.load_generation() {
            return Err(mismatch(BindingFieldV1::LoadGeneration));
        }
        let DispatchKindV1::PersistentTask {
            service_instance_identity,
            task_schema_identity,
            task_tag,
            service_epoch,
        } = request.kind()
        else {
            return Err(ServiceHostErrorV1::PersistentTaskRequired);
        };
        if service_instance_identity != key.service_instance_identity() {
            return Err(mismatch(BindingFieldV1::ServiceInstance));
        }
        if task_schema_identity != key.host_task_schema_id() {
            return Err(mismatch(BindingFieldV1::TaskSchema));
        }
        if service_epoch != key.service_epoch() {
            return Err(mismatch(BindingFieldV1::ServiceEpoch));
        }
        if self
            .contract
            .model_config()
            .admitted_task_tags
            .binary_search(&task_tag)
            .is_err()
        {
            return Err(mismatch(BindingFieldV1::TaskTag));
        }
        if result.request_identity() != request.identity() {
            return Err(mismatch(BindingFieldV1::DispatchRequest));
        }
        if result.loaded_object_identity() != request.loaded_object_identity() {
            return Err(mismatch(BindingFieldV1::DispatchResult));
        }
        let DispatchOutcomeV1::Submitted {
            submission_identity,
            completion_signal_identity,
        } = result.outcome()
        else {
            return Err(ServiceHostErrorV1::DispatchNotSubmitted);
        };
        Ok(TaskTicketV1 {
            _service_brand: &self.ticket_brand,
            key,
            queue_capacity: self.contract.model_config().queue_capacity,
            generation_modulus: self.contract.model_config().generation_modulus,
            task_id,
            task_tag,
            slot,
            request,
            result,
            submission_identity,
            completion_signal_identity,
        })
    }
}
