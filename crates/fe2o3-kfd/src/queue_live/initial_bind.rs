//! Initial ordinary binding on an existing primary, with rooted partial inputs.

#![forbid(unsafe_code)]

use super::*;
use crate::queue::dispatch_binding::{
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, preparation::PreparationMemoryV1,
    prepare_public_fixed_dispatch_resources_with_capacity_in_place,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) type InitialPreparationResultV1 = Result<
    (
        Result<(), ComputeAqlQueueSessionErrorV1>,
        Result<(), ComputeAqlQueueSessionErrorV1>,
    ),
    ComputeAqlQueueSessionErrorV1,
>;

pub(super) fn initial_dispatch_state_admitted_v1(
    unpublished: &UnpublishedDispatchStateV1,
    attached: bool,
    generation: Option<u64>,
    count: usize,
    identities: usize,
    insertion: Option<usize>,
) -> bool {
    unpublished.is_clear()
        && !attached
        && generation.is_none()
        && count == 0
        && identities == 0
        && insertion.is_none()
}

pub(super) trait InitialBindingParentV1 {
    type Memory: PreparationMemoryV1;
    type TerminalParent;

    fn dispatch_capacity(&self) -> Gfx942FixedDispatchCapacityV1;

    fn preflight<const N: usize>(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn with_preparation(
        &mut self,
        work: impl FnOnce(&mut Self::Memory) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> InitialPreparationResultV1;
    fn validate<const N: usize>(
        &mut self,
        preparation: &FixedDispatchPreparationCustodyV1<N>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn install(&mut self, dispatch: DispatchResourceOwnerV1);
    fn take_terminal_parent(&mut self) -> Self::TerminalParent;
}

pub(super) struct InitialBindingCustodyV1<'a, const N: usize, I, P> {
    pub(super) programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
    packets: Option<[Gfx942FixedDispatchPacketV1; N]>,
    initializer: Option<I>,
    pub(super) data: Vec<Gfx942FixedDispatchDataV1>,
    pub(super) preparation: Option<FixedDispatchPreparationCustodyV1<N>>,
    pub(super) prepared_generation: Option<PreparedDispatchGenerationV1>,
    pub(super) terminal_parent: Option<P>,
    started: bool,
    #[cfg(test)]
    pub(super) preparation_fault: Option<(
        crate::queue::dispatch_binding::preparation::PreparationStageV1,
        bool,
    )>,
}

impl<'a, const N: usize, I, P> InitialBindingCustodyV1<'a, N, I, P> {
    pub(super) fn new(
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        initializer: I,
    ) -> Box<Self> {
        Box::new(Self {
            programs,
            packets: Some(packets),
            initializer: Some(initializer),
            data: Vec::new(),
            preparation: None,
            prepared_generation: None,
            terminal_parent: None,
            started: false,
            #[cfg(test)]
            preparation_fault: None,
        })
    }
}

pub(super) fn bind_initial_with_v1<'a, const N: usize, P, I>(
    parent: &mut P,
    mut root: Box<InitialBindingCustodyV1<'a, N, I, P::TerminalParent>>,
    data_count: usize,
    retain: impl FnOnce(Box<InitialBindingCustodyV1<'a, N, I, P::TerminalParent>>),
) -> Result<(), ComputeAqlQueueSessionErrorV1>
where
    P: InitialBindingParentV1,
    I: FnMut(
        &mut P::Memory,
        usize,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1>,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        parent.preflight::<N>()?;
        let capacity = parent.dispatch_capacity();
        capacity.validate_batch::<N>()?;
        if data_count > GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
            return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
                requested: data_count,
                maximum: GFX942_MAX_FIXED_DISPATCH_DATA_V1,
            }
            .into());
        }
        root.data.try_reserve_exact(data_count).map_err(|_| {
            ComputeAqlQueueSessionErrorV1::Contract("initial dispatch data roster allocation")
        })?;
        root.prepared_generation = PreparedDispatchGenerationV1::preallocate::<N>(
            &capacity,
            DispatchGenerationSeedV1::Fresh,
        )?;
        root.started = true;
        parent.currentness()?;
        let (operation, retake) = parent.with_preparation(|memory| {
            for index in 0..data_count {
                let data = (root.initializer.as_mut().expect("rooted initializer"))(memory, index)?;
                // Capacity was reserved before entry; no fallible step separates owners.
                root.data.push(data);
            }
            root.preparation = Some(FixedDispatchPreparationCustodyV1::new(
                root.packets.take().expect("rooted initial packets"),
                core::mem::take(&mut root.data),
            ));
            let preparation = root
                .preparation
                .as_mut()
                .expect("rooted initial preparation");
            #[cfg(test)]
            if let Some((stage, panic)) = root.preparation_fault {
                preparation.primary_inject_stage_v1(stage, panic);
            }
            prepare_public_fixed_dispatch_resources_with_capacity_in_place(
                memory,
                &root.programs,
                preparation,
                &capacity,
                &mut root.prepared_generation,
            )
            .map_err(Into::into)
        })?;
        retake?;
        operation?;
        // User captures may have panicking destructors. Dispose them before commit.
        drop(root.initializer.take());
        let preparation = root
            .preparation
            .as_mut()
            .expect("completed initial preparation");
        parent.validate(preparation)?;
        let dispatch = preparation.take_completed()?;
        parent.install(dispatch);
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        result => {
            if root.started || result.is_err() {
                root.terminal_parent = Some(parent.take_terminal_parent());
                poison_process_global_after_dispatch_terminal_v1();
            }
            retain(root);
            match result {
                Ok(Err(error)) => Err(error),
                Err(payload) => resume_unwind(payload),
                Ok(Ok(())) => unreachable!(),
            }
        }
    }
}

impl InitialBindingParentV1 for &mut ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;
    type TerminalParent = ComputeAqlQueueSessionV1;

    fn dispatch_capacity(&self) -> Gfx942FixedDispatchCapacityV1 {
        self.dispatch_capacity.clone()
    }

    fn preflight<const N: usize>(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.key != self.compute_lane_session
            || self.has_any_persistent_compute_attachment_v1()
            || self.terminal_dependency.is_some()
            || self.sdma_pool_trim.is_some()
            || self.sdma_allocation.is_some()
            || self.sdma_promotion.is_some()
            || self.sdma_demotion.is_some()
            || self.sdma_synchronous.is_some()
            || self.sdma_recycle.is_some()
            || !initial_dispatch_state_admitted_v1(
                &self.unpublished_dispatch,
                self.dispatch.is_some(),
                self.detached_dispatch_generation,
                self.detached_data_count,
                self.detached_data_identities.len(),
                self.detached_next_insertion_index,
            )
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        validate_fixed_batch_ring::<N>(self.observation.ring_bytes)?;
        self.completion_owner.ensure_releasable()?;
        self.dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing initial binding engine",
            ))?;
        // Validate the primary owners, not teardown admission: live SDMA buffers are allowed.
        primary_release::preflight_primary_owners_v1::<
            construction_primary::LinuxPrimaryEnvironmentV1,
        >(
            engine,
            self.key,
            self.completion_signals.as_ref(),
            self.exception.as_ref(),
            self.doorbell.as_ref(),
        )
    }

    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.check_currentness()
    }

    fn with_preparation(
        &mut self,
        work: impl FnOnce(&mut Self::Memory) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> InitialPreparationResultV1 {
        self.with_live_queue_memory_model_custody(work)
    }

    fn validate<const N: usize>(
        &mut self,
        preparation: &FixedDispatchPreparationCustodyV1<N>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.validate_persistent_bind_preparation_v1(preparation)
    }

    fn install(&mut self, dispatch: DispatchResourceOwnerV1) {
        self.dispatch = Some(dispatch);
    }

    fn take_terminal_parent(&mut self) -> Self::TerminalParent {
        self.take_for_terminal_auxiliary_construction_v1()
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Prepares the first ordinary fixed batch on an existing, unused primary.
    /// This does not publish or fabricate recycled/pristine-abort history.
    ///
    /// Each initializer call returns one owner, which is rooted before the next
    /// call. Initializers must retain any native owners they create but do not
    /// return. Consumed inputs remain retained on rejection; failures after
    /// admission retain the original parent until process teardown.
    pub fn bind_initial_fixed_dispatch_v1<const N: usize>(
        &mut self,
        programs: Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'_>>,
        packets: [Gfx942FixedDispatchPacketV1; N],
        data_count: usize,
        initialize: impl FnMut(
            &mut SharedGttMemorySessionV1,
            usize,
        )
            -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let root = InitialBindingCustodyV1::new(programs, packets, initialize);
        bind_initial_with_v1(&mut &mut *self, root, data_count, core::mem::forget)
    }
}
