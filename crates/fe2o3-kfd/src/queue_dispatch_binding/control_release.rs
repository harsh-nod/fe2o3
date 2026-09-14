//! Returning and detached dispatch controls retain their complete owner through cleanup.

use super::*;
use crate::shared_memory::ControlCleanupCustodyV1;
use pristine_abort::PristineControlReleaseV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(test)]
#[path = "control_release/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::queue) enum ReturningControlModeV1 {
    AfterRecycle,
    ReturningDestroy,
    DetachedPersistent { expected_generation: u64 },
}

/// Completion stays in this root until the caller explicitly transfers it.
/// Live callers must separately retain the root across their model loan.
pub(in crate::queue) struct ReturningControlCleanupCustodyV1 {
    mode: ReturningControlModeV1,
    kernarg: Option<KernargAuthority>,
    code: std::vec::IntoIter<CodeAuthority>,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<DispatchDataAuthorityV1>,
    data_premises: Vec<RetainedDataPremiseV1>,
    generation: DispatchGenerationOwnerV1,
    persistent_control: PersistentFixedDispatchControlStateV1,
    active_control: Option<ControlCleanupCustodyV1>,
    returned: Vec<ReturnedDispatchDataLeaseV1>,
    returned_generation: Option<u64>,
    started: bool,
    complete: bool,
    #[cfg(test)]
    return_capacity_override: Option<usize>,
}

impl ReturningControlCleanupCustodyV1 {
    pub(in crate::queue) fn new(
        owner: DispatchResourceOwnerV1,
        mode: ReturningControlModeV1,
    ) -> Self {
        Self {
            mode,
            kernarg: Some(owner.kernarg),
            code: owner.code.into_iter(),
            code_identity: owner.code_identity,
            packets: owner.packets,
            data: owner.data,
            data_premises: owner.data_premises,
            generation: owner.generation,
            persistent_control: owner.persistent_control,
            active_control: None,
            returned: Vec::new(),
            returned_generation: None,
            started: false,
            complete: false,
            #[cfg(test)]
            return_capacity_override: None,
        }
    }

    pub(in crate::queue) fn release_in_place(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.started {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.started = true;
        let generation = match self.mode {
            ReturningControlModeV1::AfterRecycle => Some(self.generation.returned_generation()?),
            ReturningControlModeV1::ReturningDestroy => {
                Some(self.generation.returning_destroy_generation()?)
            }
            ReturningControlModeV1::DetachedPersistent {
                expected_generation,
            } => {
                validate_detached_persistent_control_release_state_v1(
                    self.persistent_control,
                    self.data.len(),
                    self.data_premises.len(),
                    self.generation.returned_generation()?,
                    expected_generation,
                )?;
                None
            }
        };
        if let Some(generation) = generation {
            if self.data.len() != self.data_premises.len() {
                return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    index: self.data.len().min(self.data_premises.len()),
                    detail: "retained data/premise cardinality",
                });
            }
            let capacity = self.data.len();
            #[cfg(test)]
            let capacity = self.return_capacity_override.unwrap_or(capacity);
            self.returned.try_reserve_exact(capacity).map_err(|_| {
                Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                    operation: "returning dispatch data",
                }
            })?;
            if self.returned.capacity() < self.data.len() {
                return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                    operation: "returning dispatch data",
                });
            }
            self.returned_generation = Some(generation);
        }

        self.active_control = Some(ControlCleanupCustodyV1::kernarg(
            self.kernarg
                .take()
                .expect("unstarted returning cleanup retains kernarg")
                .into_token(),
        ));
        self.release_active_control(memory)?;
        while let Some(code) = self.code.next() {
            self.active_control = Some(ControlCleanupCustodyV1::code(code.into_token()));
            self.release_active_control(memory)?;
        }

        if generation.is_some() {
            // Capacity and cardinality were checked before any disposal callback.
            for (authority, premise) in self.data.drain(..).zip(self.data_premises.drain(..)) {
                self.returned
                    .push(ReturnedDispatchDataLeaseV1 { authority, premise });
            }
        }
        self.complete = true;
        Ok(())
    }

    fn release_active_control(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let active = self.active_control.as_mut().expect("rooted active control");
        memory.release_control(active)?;
        if !active.is_complete() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.active_control = None;
        Ok(())
    }

    pub(in crate::queue) fn take_completed(
        &mut self,
    ) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
        if !self.complete || matches!(self.mode, ReturningControlModeV1::DetachedPersistent { .. })
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let generation = self
            .returned_generation
            .take()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        Ok(ReturnedDispatchDataV1 {
            generation,
            data: core::mem::take(&mut self.returned),
        })
    }
}

pub(super) fn release_returning_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut impl PristineControlReleaseV1,
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        root.release_in_place(memory)?;
        root.take_completed()
    }));
    match result {
        Ok(Ok(returned)) => Ok(returned),
        Ok(Err(error)) => {
            retain(root);
            Err(error)
        }
        Err(payload) => {
            retain(root);
            resume_unwind(payload)
        }
    }
}

pub(super) fn release_detached_persistent_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut impl PristineControlReleaseV1,
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        root.release_in_place(memory)
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            retain(root);
            Err(error)
        }
        Err(payload) => {
            retain(root);
            resume_unwind(payload)
        }
    }
}
