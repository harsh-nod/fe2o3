use super::*;
use fe2o3_kfd::Gfx942NativeFillArenaPollV1;

impl KfdRuntimeBackendV1 {
    pub(crate) fn progress_generated_arena_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_arena_v1(plan, roster)?;
        if index >= SLOTS {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena recipe index",
            ));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_arena_device_v1(plan)?;
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            let session = arena
                .session
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            let cell = &mut arena.storage.cells[index];
            #[allow(
                clippy::result_large_err,
                reason = "original completion is retained without post-publication allocation"
            )]
            let recycle = |session: &mut SessionV1, completed| {
                session
                    .recycle(completed)
                    .map(|_| ())
                    .map_err(|failure| (failure.error, failure.retryable))
            };
            #[allow(
                clippy::result_large_err,
                reason = "poll refusal retains the original published batch without fallible allocation"
            )]
            let poll = |session: &mut SessionV1, batch| {
                session
                    .poll(batch)
                    .map(|poll| match poll {
                        Gfx942NativeFillArenaPollV1::Pending(original) => {
                            selected_step::SelectedPoll::Pending(original)
                        }
                        Gfx942NativeFillArenaPollV1::Ready(original) => {
                            selected_step::SelectedPoll::Ready(original)
                        }
                    })
                    .map_err(|failure| selected_step::SelectedPollFailure {
                        error: failure.error,
                        refused: failure.refused,
                    })
            };
            selected_step::step(
                session,
                &mut cell.receipt,
                index,
                SessionV1::submit,
                poll,
                recycle,
            )
            .map_err(|error| self.generated_native_error_v1("arena progress", error))?;
            self.check_arena_device_v1(plan)?;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(self
            .generated_shells
            .get(&plan.key)
            .and_then(|record| record.arena.as_ref())
            .is_some_and(|arena| matches!(arena.storage.cells[index].receipt, ReceiptV1::Recycled)))
    }

    pub(crate) fn read_generated_arena_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_arena_v1(plan, roster)?;
        if index >= SLOTS
            || !self
                .generated_shells
                .get(&plan.key)
                .and_then(|record| record.arena.as_ref())
                .is_some_and(|arena| {
                    !arena.storage.cells[index].copied
                        && matches!(arena.storage.cells[index].receipt, ReceiptV1::Recycled)
                })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena original copy phase",
            ));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_arena_device_v1(plan)?;
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            arena
                .session
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .read_into(index, destination)
                .map_err(|error| {
                    self.generated_native_error_v1("arena original range copy", error)
                })?;
            self.check_arena_device_v1(plan)?;
            self.generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort())
                .storage
                .cells[index]
                .copied = true;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }
}
