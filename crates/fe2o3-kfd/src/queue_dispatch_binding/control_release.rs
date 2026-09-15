//! Dispatch cleanup retains its complete owner through every destructive prefix.

use super::*;
use crate::shared_memory::{ControlCleanupCustodyV1, DataCleanupCustodyV1, DispatchDataReleaseV1};
use pristine_abort::PristineControlReleaseV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(test)]
#[path = "control_release/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::queue) enum ReturningControlModeV1 {
    Ordinary,
    AfterRecycle,
    ReturningDestroy,
    DetachedPersistent { expected_generation: u64 },
    PersistentBeforePublication,
    PersistentAfterRecycle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistentOutputStateV1 {
    Unprepared,
    Prepared(u64),
    Returnable(u64),
    Taken,
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
    remaining_data: std::vec::IntoIter<DispatchDataAuthorityV1>,
    active_data: Option<DataCleanupCustodyV1>,
    returned: Vec<ReturnedDispatchDataLeaseV1>,
    returned_generation: Option<u64>,
    persistent_returned: Vec<Gfx942FixedDispatchDataV1>,
    persistent_output: PersistentOutputStateV1,
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
            remaining_data: Vec::new().into_iter(),
            active_data: None,
            returned: Vec::new(),
            returned_generation: None,
            persistent_returned: Vec::new(),
            persistent_output: PersistentOutputStateV1::Unprepared,
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
        if self.mode == ReturningControlModeV1::Ordinary {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.release_control_phase(memory)?;
        self.complete = true;
        Ok(())
    }

    pub(in crate::queue) fn release_ordinary_in_place(
        &mut self,
        memory: &mut (impl PristineControlReleaseV1 + DispatchDataReleaseV1),
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.mode != ReturningControlModeV1::Ordinary {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.release_control_phase(memory)?;
        self.remaining_data = core::mem::take(&mut self.data).into_iter();
        for data in self.remaining_data.by_ref() {
            self.active_data = Some(DataCleanupCustodyV1::from_authority(data));
            let active = self.active_data.as_mut().expect("rooted active data");
            memory.release_data(active)?;
            if !active.is_complete() {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
            }
            self.active_data = None;
        }
        self.complete = true;
        Ok(())
    }

    fn release_control_phase(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.started {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.started = true;
        let generation = match self.mode {
            ReturningControlModeV1::Ordinary => {
                self.generation.ensure_prepared()?;
                None
            }
            ReturningControlModeV1::AfterRecycle
            | ReturningControlModeV1::PersistentAfterRecycle => {
                Some(self.generation.returned_generation()?)
            }
            ReturningControlModeV1::ReturningDestroy
            | ReturningControlModeV1::PersistentBeforePublication => {
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
            if matches!(
                self.mode,
                ReturningControlModeV1::PersistentBeforePublication
                    | ReturningControlModeV1::PersistentAfterRecycle
            ) {
                self.persistent_returned
                    .try_reserve_exact(capacity)
                    .map_err(|_| Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "persistent dispatch data",
                    })?;
                if self.persistent_returned.capacity() < self.data.len() {
                    return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "persistent dispatch data",
                    });
                }
                self.persistent_output = PersistentOutputStateV1::Prepared(generation);
            } else {
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
        }

        let release = self.release_controls(memory);
        // Only a normal return, including Err, permits persistent data extraction.
        if let PersistentOutputStateV1::Prepared(generation) = self.persistent_output {
            self.persistent_output = PersistentOutputStateV1::Returnable(generation);
        }
        release?;
        if matches!(
            self.mode,
            ReturningControlModeV1::AfterRecycle | ReturningControlModeV1::ReturningDestroy
        ) {
            // Capacity and cardinality were checked before any disposal callback.
            for (authority, premise) in self.data.drain(..).zip(self.data_premises.drain(..)) {
                self.returned
                    .push(ReturnedDispatchDataLeaseV1 { authority, premise });
            }
        }
        Ok(())
    }

    fn release_controls(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
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
        if !self.complete
            || !matches!(
                self.mode,
                ReturningControlModeV1::AfterRecycle | ReturningControlModeV1::ReturningDestroy
            )
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

    pub(in crate::queue) fn take_persistent_data(
        &mut self,
    ) -> Result<(u64, Vec<Gfx942FixedDispatchDataV1>), Gfx942DispatchBindingErrorV1> {
        if !matches!(
            self.mode,
            ReturningControlModeV1::PersistentBeforePublication
                | ReturningControlModeV1::PersistentAfterRecycle
        ) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let PersistentOutputStateV1::Returnable(generation) = self.persistent_output else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        };
        self.persistent_output = PersistentOutputStateV1::Taken;
        for (authority, premise) in self.data.drain(..).zip(self.data_premises.iter()) {
            self.persistent_returned
                .push(dispatch_data_from_authority_v1(
                    authority,
                    premise.fully_initialized,
                ));
        }
        Ok((generation, core::mem::take(&mut self.persistent_returned)))
    }
}

pub(super) fn release_ordinary_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut (impl PristineControlReleaseV1 + DispatchDataReleaseV1),
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let result = catch_unwind(AssertUnwindSafe(|| root.release_ordinary_in_place(memory)));
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

pub(super) fn release_returning_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut impl PristineControlReleaseV1,
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !matches!(
            root.mode,
            ReturningControlModeV1::AfterRecycle | ReturningControlModeV1::ReturningDestroy
        ) {
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

pub(super) fn release_persistent_with_v1(
    mut root: ReturningControlCleanupCustodyV1,
    memory: &mut impl PristineControlReleaseV1,
    retain: impl FnOnce(ReturningControlCleanupCustodyV1),
) -> Result<
    (u64, Vec<Gfx942FixedDispatchDataV1>),
    (Gfx942DispatchBindingErrorV1, Vec<Gfx942FixedDispatchDataV1>),
> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !matches!(
            root.mode,
            ReturningControlModeV1::PersistentBeforePublication
                | ReturningControlModeV1::PersistentAfterRecycle
        ) {
            return Err((Gfx942DispatchBindingErrorV1::ResourcePhase, Vec::new()));
        }
        match root.release_in_place(memory) {
            Ok(()) => root
                .take_persistent_data()
                .map_err(|error| (error, Vec::new())),
            Err(error) => {
                let data = match root.take_persistent_data() {
                    Ok((_, data)) => data,
                    Err(_) => Vec::new(),
                };
                Err((error, data))
            }
        }
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
