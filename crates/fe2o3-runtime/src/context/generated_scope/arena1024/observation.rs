//! Inert observations of original receipts, never completion/disposal authority.

use super::*;

/// Sequences record successful original native transitions followed by closing
/// device-currentness checks. They are not GPU timestamps or transferable proof.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeGfx942ArenaMemberObservationV1 {
    pub published: u64,
    pub last_pending: u64,
    pub completed: u64,
}

/// A later member's original Ready observation preceded a subsequent original
/// Pending observation of an earlier published member. No decoder ordering or
/// inference from unobserved hardware state contributes to this witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGfx942ArenaOutOfOrderObservationV1 {
    pub earlier_member: usize,
    pub later_member: usize,
    pub earlier_published: u64,
    pub later_published: u64,
    pub later_completed: u64,
    pub earlier_pending: u64,
}

/// Read-only telemetry; none of these counts authorizes disposal. In particular
/// published-without-observed-completion is not simultaneous GPU execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeGfx942ArenaObservationV1 {
    pub events: u64,
    pub publications: usize,
    pub pending_observations: u64,
    pub completions: usize,
    pub maximum_published_without_observed_completion: usize,
    pub out_of_order: Option<RuntimeGfx942ArenaOutOfOrderObservationV1>,
}

pub(crate) enum Event {
    None,
    Published,
    Pending,
    Completed,
}

pub(in crate::context) struct Observations {
    members: HostMetadataTableV1<RuntimeGfx942ArenaMemberObservationV1>,
    summary: RuntimeGfx942ArenaObservationV1,
    greatest_completed_member: Option<usize>,
}

impl Observations {
    pub(super) fn new(
        account: &ResourceCreditAccountV1,
    ) -> Result<Self, RuntimeGfx942ScopeErrorV1> {
        Ok(Self {
            members: HostMetadataTableV1::try_new(SLOTS, Some(account), Default::default)
                .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?,
            summary: RuntimeGfx942ArenaObservationV1::default(),
            greatest_completed_member: None,
        })
    }

    pub(in crate::context) fn record(&mut self, index: usize, event: Event) -> Result<(), ()> {
        let old = *self.members.get(index).ok_or(())?;
        let valid = match event {
            Event::None => return Ok(()),
            Event::Published => old == RuntimeGfx942ArenaMemberObservationV1::default(),
            Event::Pending | Event::Completed => old.published != 0 && old.completed == 0,
        };
        if !valid {
            return Err(());
        }
        let sequence = self.summary.events.checked_add(1).ok_or(())?;
        match event {
            Event::None => return Ok(()),
            Event::Published => {
                self.members[index].published = sequence;
                self.summary.publications += 1;
                self.summary.maximum_published_without_observed_completion = self
                    .summary
                    .maximum_published_without_observed_completion
                    .max(self.summary.publications - self.summary.completions);
            }
            Event::Pending => {
                self.members[index].last_pending = sequence;
                self.summary.pending_observations += 1;
                if self.summary.out_of_order.is_none()
                    && let Some(later) = self
                        .greatest_completed_member
                        .filter(|later| *later > index)
                    && old.published < self.members[later].published
                {
                    let completed = self.members[later];
                    self.summary.out_of_order = Some(RuntimeGfx942ArenaOutOfOrderObservationV1 {
                        earlier_member: index,
                        later_member: later,
                        earlier_published: old.published,
                        later_published: completed.published,
                        later_completed: completed.completed,
                        earlier_pending: sequence,
                    });
                }
            }
            Event::Completed => {
                self.members[index].completed = sequence;
                self.summary.completions += 1;
                self.greatest_completed_member = Some(
                    self.greatest_completed_member
                        .map_or(index, |old| old.max(index)),
                );
            }
        }
        self.summary.events = sequence;
        Ok(())
    }
}

impl<P: RuntimeGfx942RegistryCompletionCarrierV1> RuntimeGfx942Arena1024ScopeV1<'_, '_, P> {
    /// Available only for the independent profile after original admission.
    /// A successful copied result alone does not fabricate any receipt event.
    pub fn receipt_observations_v1(&self) -> Option<RuntimeGfx942ArenaObservationV1> {
        let root = self.root.as_ref()?;
        if root.state == State::Unknown {
            return None;
        }
        root.observations.as_ref().map(|value| value.summary)
    }

    pub fn member_receipt_observation_v1(
        &self,
        index: usize,
    ) -> Option<RuntimeGfx942ArenaMemberObservationV1> {
        let root = self.root.as_ref()?;
        if root.state == State::Unknown {
            return None;
        }
        root.observations.as_ref()?.members.get(index).copied()
    }
}

#[cfg(test)]
mod tests;
