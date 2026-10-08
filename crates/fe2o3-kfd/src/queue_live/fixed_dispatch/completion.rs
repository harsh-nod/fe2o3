//! Fixed-dispatch completion, bounded waits, and recycling.

use super::*;

impl ComputeAqlQueueSessionV1 {
    pub(in super::super) fn terminalize_fixed_dispatch_submission_result_v1<T>(
        &mut self,
        result: Result<T, FixedDispatchSubmissionFailureV1>,
    ) -> Result<T, FixedDispatchSubmissionFailureV1> {
        if matches!(&result, Err(FixedDispatchSubmissionFailureV1::Terminal(_))) {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    pub(in super::super) fn terminalize_fixed_dispatch_observation_result_v1<T>(
        &mut self,
        result: Result<T, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<T, ComputeAqlQueueSessionErrorV1> {
        if result.is_err() {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn terminalize_fixed_dispatch_recycle_result_v1<const N: usize>(
        &mut self,
        result: Result<
            Gfx942CompletionRecycleObservationV1,
            Gfx942FixedDispatchRecycleFailureV1<N>,
        >,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        if result
            .as_ref()
            .is_err_and(|failure| failure.retryable_completed.is_none())
        {
            self.poison_terminal();
            poison_process_global_after_dispatch_terminal_v1();
        }
        result
    }

    pub(in super::super) fn classify_fixed_dispatch_binding<T>(
        &mut self,
        mode: FixedDispatchBindingModeV1,
        binding: Result<T, Gfx942DispatchBindingErrorV1>,
    ) -> Result<T, FixedDispatchSubmissionFailureV1> {
        binding.map_err(|error| {
            let error = error.into();
            match mode {
                FixedDispatchBindingModeV1::Ordinary => {
                    FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)
                }
                FixedDispatchBindingModeV1::ExactPersistentAttachment => {
                    self.poison_terminal();
                    FixedDispatchSubmissionFailureV1::Terminal(error)
                }
            }
        })
    }

    /// Polls every packet signal once and returns linear pending or completed custody.
    pub fn poll_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_fixed_dispatch_with_progress_inner(batch)
        }));
        let poll = match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result)?,
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match poll {
            Gfx942DispatchPollWithProgressV1::Pending { batch, .. } => {
                Ok(Gfx942DispatchPollV1::Pending(batch))
            }
            Gfx942DispatchPollWithProgressV1::Ready { completed, .. } => {
                Ok(Gfx942DispatchPollV1::Ready(completed))
            }
        }
    }

    /// Polls every packet signal once and returns custody plus same-scan progress.
    pub fn poll_fixed_dispatch_with_progress<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_fixed_dispatch_with_progress_inner(batch)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(in super::super) fn poll_fixed_dispatch_with_progress_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        self.poll_selected_fixed_dispatch(batch, &mut recipe::RecipeV1::Ordinary)
    }

    pub(in super::super) fn poll_selected_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        recipe: &mut recipe::RecipeV1<'_>,
    ) -> Result<Gfx942DispatchPollWithProgressV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        self.dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        if recipe
            .validate_published(self, identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.poll_completion_batch_with_progress(completion) {
            Ok(poll) => {
                if let Gfx942CompletionPollWithProgressV1::Ready { completed, .. } = &poll
                    && recipe.mark_completed(self, identity, completed).is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_poll_with_progress(poll, identity))
            }
            Err(error) => {
                recipe.poison(self);
                Err(error)
            }
        }
    }

    /// Performs a bounded wait for every signal in the exact published batch.
    pub fn wait_fixed_dispatch<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.wait_fixed_dispatch_inner(batch, polls)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(in super::super) fn wait_fixed_dispatch_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        polls: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate_published(identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        match self.wait_completion_batch(completion, polls) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(identity, &completion)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, identity))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Waits for the exact published batch until a monotonic relative deadline.
    ///
    /// This is the preferred blocking API. It performs a short latency spin,
    /// then yields and sleeps with bounded backoff. The poll-count method is
    /// retained for compatibility with callers that require an observation
    /// budget rather than a time budget.
    pub fn wait_fixed_dispatch_for<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        timeout_milliseconds: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.wait_fixed_dispatch_for_inner(batch, timeout_milliseconds)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_observation_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(in super::super) fn wait_fixed_dispatch_for_inner<const N: usize>(
        &mut self,
        batch: Gfx942DispatchBatchV1<N>,
        timeout_milliseconds: u32,
    ) -> Result<Gfx942CompletedDispatchBatchV1<N>, ComputeAqlQueueSessionErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        if self
            .dispatch
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate_published(identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_milliseconds));
        match self.wait_completion_batch_until(completion, deadline) {
            Ok(completion) => {
                if self
                    .dispatch
                    .as_mut()
                    .expect("dispatch owner retained")
                    .mark_completed(identity, &completion)
                    .is_err()
                {
                    self.poison_terminal();
                    return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into());
                }
                Ok(wrap_completed(completion, identity))
            }
            Err(error) => {
                if let Some(dispatch) = self.dispatch.as_mut() {
                    dispatch.poison();
                }
                Err(error)
            }
        }
    }

    /// Recycles all completed signal slots and returns the queue to prepared state.
    #[allow(clippy::result_large_err)]
    pub fn recycle_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        if self.terminal_poisoned {
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::Poisoned.into(),
                retryable_completed: None,
            });
        }
        if self.has_any_persistent_compute_attachment_v1() {
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::ResourcePhase.into(),
                retryable_completed: Some(completed),
            });
        }
        let operation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.recycle_fixed_dispatch_inner(completed)
        }));
        match operation {
            Ok(result) => self.terminalize_fixed_dispatch_recycle_result_v1(result),
            Err(payload) => {
                self.poison_terminal();
                permanently_poison_process_global_kfd_runtime_gate_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn recycle_fixed_dispatch_inner<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        self.recycle_selected_fixed_dispatch(completed, &mut recipe::RecipeV1::Ordinary)
    }

    #[allow(clippy::result_large_err)]
    pub(in super::super) fn recycle_selected_fixed_dispatch<const N: usize>(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<N>,
        recipe: &mut recipe::RecipeV1<'_>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<N>> {
        let (completion, identity) = unwrap_completed(completed);
        let completion_occurrence = completion.occurrence_v1();
        let completion_occurrence = match completion_occurrence {
            Ok(completion_occurrence) => completion_occurrence,
            Err(error) => {
                self.poison_terminal();
                return Err(Gfx942FixedDispatchRecycleFailureV1 {
                    error: error.into(),
                    retryable_completed: None,
                });
            }
        };
        if recipe
            .validate_completed(self, identity, &completion)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                retryable_completed: None,
            });
        }
        let observation = match self.recycle_completion_batch_retaining(completion) {
            Ok(observation) => observation,
            Err((error, completion)) => {
                let failure = Gfx942FixedDispatchRecycleFailureV1::from_completion_failure(
                    error, completion, identity,
                );
                if failure.retryable_completed.is_none() {
                    recipe.poison(self);
                }
                return Err(failure);
            }
        };
        if recipe
            .recycle(self, identity, completion_occurrence)
            .is_err()
        {
            self.poison_terminal();
            return Err(Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942DispatchBindingErrorV1::StaleDispatchGeneration.into(),
                retryable_completed: None,
            });
        }
        Ok(observation)
    }
}
