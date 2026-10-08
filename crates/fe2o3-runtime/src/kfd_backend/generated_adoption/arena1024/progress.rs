use super::*;
use crate::ArenaReceiptEventV1 as Event;

// Called only after the existing selected step succeeds. Neither this classifier
// nor its inert output is usable as a receipt or permits another native step.
fn event_after<B, C>(
    was_unpublished: bool,
    was_published: bool,
    receipt: &ReceiptV1<B, C>,
) -> Event {
    match receipt {
        ReceiptV1::Published(_) if was_unpublished => Event::Published,
        ReceiptV1::Published(_) if was_published => Event::Pending,
        ReceiptV1::Completed(_) if was_published => Event::Completed,
        _ => Event::None,
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn progress_generated_arena_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
    ) -> Result<(bool, Event), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_arena_v1(plan, roster)?;
        if plan
            .profile
            .arena_slots()
            .is_none_or(|slots| index >= slots)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena recipe index",
            ));
        }
        let mut observation = (false, Event::None);
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
            let was_unpublished = matches!(cell.receipt, ReceiptV1::Ready | ReceiptV1::RetryReady);
            let was_published = matches!(cell.receipt, ReceiptV1::Published(_));
            selected_step::step(
                session,
                &mut cell.receipt,
                index,
                SessionV1::submit,
                SessionV1::poll,
                SessionV1::recycle,
            )
            .map_err(|error| self.generated_native_error_v1("arena progress", error))?;
            let receipt = &self
                .generated_shells
                .get(&plan.key)
                .and_then(|record| record.arena.as_ref())
                .unwrap_or_else(|| std::process::abort())
                .storage
                .cells[index]
                .receipt;
            let event = event_after(was_unpublished, was_published, receipt);
            let recycled = matches!(receipt, ReceiptV1::Recycled);
            self.check_arena_device_v1(plan)?;
            observation = (recycled, event);
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)?;
        Ok(observation)
    }

    pub(crate) fn read_generated_arena_recipe_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        index: usize,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_arena_v1(plan, roster)?;
        if plan
            .profile
            .arena_slots()
            .is_none_or(|slots| index >= slots)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_observation_classifies_only_successful_original_receipt_transitions() {
        let published = ReceiptV1::<(), ()>::Published(());
        assert!(matches!(
            event_after(true, false, &published),
            Event::Published
        ));
        assert!(matches!(
            event_after(false, true, &published),
            Event::Pending
        ));
        assert!(matches!(
            event_after(false, true, &ReceiptV1::<(), ()>::Completed(())),
            Event::Completed
        ));
        for receipt in [
            ReceiptV1::<(), ()>::Ready,
            ReceiptV1::RetryReady,
            ReceiptV1::Recycled,
            ReceiptV1::HandedToLower(receipt::HandoffV1::Poll),
        ] {
            assert!(matches!(event_after(true, false, &receipt), Event::None));
            assert!(matches!(event_after(false, true, &receipt), Event::None));
        }
        assert!(matches!(event_after(false, false, &published), Event::None));
    }
}
