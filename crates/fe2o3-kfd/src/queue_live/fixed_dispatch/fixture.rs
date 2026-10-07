//! CPU-only fixed-dispatch owner adapters.

use super::*;

impl ComputeAqlQueueSessionV1 {
    #[cfg(test)]
    pub(in crate::queue) fn submit_registry_binding_for_test(
        &mut self,
        owner: &mut crate::queue::dispatch_binding::RegistryRecipeV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1> {
        self.submit_selected_fixed_dispatch_using(
            FixedDispatchBindingModeV1::Ordinary,
            &mut recipe::RecipeV1::Registry(owner),
            native_submit,
        )
        .map_err(FixedDispatchSubmissionFailureV1::into_public)
    }

    #[cfg(test)]
    pub(in crate::queue) fn complete_registry_binding_for_test(
        &mut self,
        owner: &mut crate::queue::dispatch_binding::RegistryRecipeV1,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<Gfx942CompletedDispatchBatchV1<1>, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        owner.validate_published(identity, &completion)?;
        let completed = self
            .completion_owner
            .complete_one_without_native_for_test(completion);
        owner.mark_completed(identity, &completed)?;
        Ok(wrap_completed(completed, identity))
    }

    #[cfg(test)]
    pub(in crate::queue) fn recycle_registry_binding_for_test(
        &mut self,
        owner: &mut crate::queue::dispatch_binding::RegistryRecipeV1,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_completed(completed);
        owner.validate_completed(identity, &completion)?;
        let occurrence = completion.occurrence_v1()?;
        let recycled = self
            .completion_owner
            .recycle_one_without_native_for_test(completion);
        owner.recycle(identity, occurrence)?;
        Ok(recycled)
    }

    #[cfg(test)]
    pub(in crate::queue) fn submit_ordinary_binding_for_test<const N: usize>(
        &mut self,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<N>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<N>, Gfx942FixedDispatchSubmissionFailureV1> {
        self.submit_fixed_dispatch_inner_classified_using(
            FixedDispatchBindingModeV1::Ordinary,
            native_submit,
        )
        .map_err(FixedDispatchSubmissionFailureV1::into_public)
    }

    #[cfg(test)]
    pub(in crate::queue) fn with_ordinary_binding_session_v1<R>(
        queue: QueueKeyV1,
        owner: DispatchResourceOwnerV1,
        operation: impl FnOnce(&mut Self) -> R,
    ) -> (
        R,
        DispatchResourceOwnerV1,
        bool,
        [super::super::completion::CompletionCustodySnapshotV1; 2],
    ) {
        let mut session = tests::persistent_compute_cancellation_test_session(queue, None, None);
        session.dispatch = Some(owner);
        let before = session.completion_owner.custody_snapshot_for_test();
        let result = operation(&mut session);
        session.completion_owner.ensure_releasable().unwrap();
        let after = session.completion_owner.custody_snapshot_for_test();
        let owner = session.dispatch.take().unwrap();
        (result, owner, session.terminal_poisoned, [before, after])
    }

    #[cfg(test)]
    pub(in super::super) fn submit_fixed_dispatch_inner_classified_with_test_owner(
        &mut self,
        owner: &mut super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1,
        template: impl FnOnce(u64) -> CompletionPacketTemplateV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
        let generation = match owner
            .bind_one()
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))
        {
            Ok(generation) => generation,
            Err(error) => {
                return self.terminalize_fixed_dispatch_submission_result_v1(Err(error));
            }
        };
        let identity = DispatchEpochIdentityV1::for_test(self.key, generation);
        let completion = self.submit_with_completions_classified_using(
            Box::new([template(generation)]),
            native_submit,
        );
        let result = finish_fixed_dispatch_submission(identity, completion, |identity| {
            owner.cancel_binding(identity.dispatch_generation())
        });
        self.terminalize_fixed_dispatch_submission_result_v1(result)
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(in super::super) fn submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        template: impl FnOnce(u64) -> CompletionPacketTemplateV1,
        native_submit: impl FnOnce(
            &mut Self,
            AqlPreparedKernelDispatchBatchV2<1>,
        ) -> Result<u64, NativeAqlSubmissionFailureV1>,
    ) -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
        if self.terminal_poisoned {
            return Err(FixedDispatchSubmissionFailureV1::Terminal(
                Gfx942DispatchBindingErrorV1::Poisoned.into(),
            ));
        }
        let template = template(owner.next_generation());
        let identity = match owner
            .reserve_one(self.key, template)
            .map_err(|error| FixedDispatchSubmissionFailureV1::Terminal(error.into()))
        {
            Ok(identity) => identity,
            Err(error) => {
                return self.terminalize_fixed_dispatch_submission_result_v1(Err(error));
            }
        };
        let completion =
            self.submit_with_completions_classified_using(Box::new([template]), native_submit);
        let completion = match completion {
            Ok(completion) => {
                if let Err(error) = owner.mark_published(identity, &completion) {
                    Err(FixedDispatchSubmissionFailureV1::Terminal(error.into()))
                } else {
                    Ok(completion)
                }
            }
            Err(error) => Err(error),
        };
        let result = finish_fixed_dispatch_submission(identity, completion, |identity| {
            owner.cancel(identity)
        });
        self.terminalize_fixed_dispatch_submission_result_v1(result)
    }

    #[cfg(test)]
    pub(in super::super) fn complete_fixed_dispatch_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<Gfx942CompletedDispatchBatchV1<1>, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_published(batch);
        let occurrence = owner.validate_published(identity, &completion)?;
        let completed = self
            .completion_owner
            .complete_one_without_native_for_test(completion);
        owner.mark_completed(identity, occurrence)?;
        Ok(wrap_completed(completed, identity))
    }

    #[cfg(test)]
    pub(in super::super) fn recycle_fixed_dispatch_with_multi_inflight_test_owner(
        &mut self,
        owner: &mut super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942DispatchBindingErrorV1> {
        let (completion, identity) = unwrap_completed(completed);
        let occurrence = owner.validate_completed(identity, &completion)?;
        let recycled = self
            .completion_owner
            .recycle_one_without_native_for_test(completion);
        owner.mark_recycled(identity, occurrence)?;
        Ok(recycled)
    }
}
