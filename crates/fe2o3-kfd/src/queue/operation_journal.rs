//! Native queue operation admission, journal update, and completion classification.

use super::*;

impl<B: NativeQueueBackendV1> NativeQueueEngineV1<B> {
    pub(super) fn complete_plain_operation<T: Copy + Eq>(
        &mut self,
        operation: NativeQueueOperationV1,
        key: QueueKeyV1,
        args: T,
        call: impl FnOnce(&mut B, T) -> QueueKernelOutcomeV1<T>,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        let outcome = call(&mut self.backend, args);
        let mut status = outcome.status;
        let malformed = outcome.value != args;
        if malformed {
            status = QueueSyscallStatusV1::Indeterminate;
        }
        let transition = match operation {
            NativeQueueOperationV1::Update => {
                QueueTransitionV1::ObserveUpdate { queue: key, status }
            }
            NativeQueueOperationV1::Disable => {
                QueueTransitionV1::ObserveDisable { queue: key, status }
            }
            _ => return Err(NativeQueueAdapterErrorV1::ModelProjection),
        };
        self.observe(transition)?;
        let phase = self.phase(key);
        self.finish_operation()?;
        if malformed {
            self.authority_poisoned = true;
            return Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
                operation,
                "UPDATE_QUEUE immutable inputs",
            ));
        }
        self.classify_completion(operation, status, phase)
    }

    pub(super) fn classify_completion(
        &self,
        operation: NativeQueueOperationV1,
        status: QueueSyscallStatusV1,
        phase: Option<ComputeAqlQueuePhaseV1>,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        match status {
            QueueSyscallStatusV1::Succeeded
                if !matches!(phase, Some(ComputeAqlQueuePhaseV1::Ambiguous)) =>
            {
                Ok(())
            }
            QueueSyscallStatusV1::FailedNoEffect => {
                Err(NativeQueueAdapterErrorV1::BackendFailedNoEffect(operation))
            }
            _ => Err(NativeQueueAdapterErrorV1::BackendIndeterminate(operation)),
        }
    }

    pub(super) fn preflight_operation(&self) -> Result<(), NativeQueueAdapterErrorV1> {
        if self.authority_poisoned {
            return Err(NativeQueueAdapterErrorV1::AuthorityPoisoned);
        }
        let retained = self
            .model
            .queues()
            .iter()
            .filter(|queue| queue.phase.retains_resources())
            .count();
        if self
            .model
            .history()
            .len()
            .checked_add(2 + retained)
            .is_none_or(|needed| needed > MAX_QUEUE_HISTORY_ENTRIES_V1)
        {
            return Err(NativeQueueAdapterErrorV1::JournalCapacity);
        }
        Ok(())
    }

    pub(super) fn prepare_operation(&mut self) -> Result<(), NativeQueueAdapterErrorV1> {
        self.preflight_operation()?;
        if self.opener_pid != std::process::id() || self.backend.opener_pid() != self.opener_pid {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::ProcessChanged);
        }
        if let Err(detail) = self.backend.check_currentness() {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::Currentness(detail));
        }
        Ok(())
    }

    pub(super) fn finish_operation(&mut self) -> Result<(), NativeQueueAdapterErrorV1> {
        if self.opener_pid != std::process::id() || self.backend.opener_pid() != self.opener_pid {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::ProcessChanged);
        }
        if let Err(detail) = self.backend.check_currentness() {
            self.quarantine_all()?;
            return Err(NativeQueueAdapterErrorV1::Currentness(detail));
        }
        Ok(())
    }

    pub(super) fn begin(
        &mut self,
        transition: QueueTransitionV1,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        self.model = self
            .model
            .next(
                self.foundation.identity(),
                self.foundation.memory(),
                transition,
            )
            .map_err(map_model_error)?;
        Ok(())
    }

    pub(super) fn observe(
        &mut self,
        transition: QueueTransitionV1,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        match self.model.next(
            self.foundation.identity(),
            self.foundation.memory(),
            transition,
        ) {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => {
                // The syscall may have changed native state, while the exact
                // model observation could not be committed. Pending phases
                // already retain resources; poison the concrete adapter
                // without inventing a CurrentnessLost history edge.
                self.authority_poisoned = true;
                Err(NativeQueueAdapterErrorV1::ModelProjection)
            }
        }
    }
}
