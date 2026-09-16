//! Shared ownership ordering; environments only implement platform primitives.

use super::*;
use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
use crate::shared_memory::DispatchDataReleaseV1;

pub(in crate::queue::live) trait PrimaryReleaseMemoryV1:
    PrimaryMemoryV1 + PristineControlReleaseV1 + DispatchDataReleaseV1
{
    fn restore_foundation(
        &mut self,
        foundation: &mut QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError>;
    fn release_queue_resources(
        &mut self,
        resources: &mut QueueResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError>;
    fn release_signals(
        &mut self,
        signals: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError>;
}

pub(in crate::queue::live) trait PrimaryReleaseEnvironmentV1:
    PrimaryEnvironmentV1
{
    type Teardown;
    type TeardownArm;

    fn validate_release_owners(
        exception: &QueueExceptionStateV1<Self>,
        doorbell: &Self::Doorbell,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>
    where
        Self: Sized;
    fn arm_teardown() -> Self::TeardownArm;
    // Pure field moves only: no allocation, callbacks or fallible native work.
    fn retain_platform(
        exception: QueueExceptionStateV1<Self>,
        doorbell: Self::Doorbell,
    ) -> Self::Teardown
    where
        Self: Sized;
    fn after_queue_destroyed(
        platform: &mut Self::Teardown,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn release_doorbell(platform: &mut Self::Teardown)
    -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn validate_for_release(platform: &Self::Teardown)
    -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn complete_shadows(platform: &mut Self::Teardown)
    -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn confirm_destroyed(arm: Self::TeardownArm);
}

pub(in crate::queue::live) struct PrimaryReleasePartsV1<'a, E: PrimaryEnvironmentV1> {
    pub(in crate::queue::live) engine:
        &'a mut NativeQueueEngineV1<PrimaryQueueBackendV1<E::Memory>>,
    pub(in crate::queue::live) key: QueueKeyV1,
    pub(in crate::queue::live) queue_id: u32,
    pub(in crate::queue::live) exception: &'a mut Option<QueueExceptionStateV1<E>>,
    pub(in crate::queue::live) doorbell: &'a mut Option<E::Doorbell>,
    pub(in crate::queue::live) dispatch: &'a mut Option<DispatchResourceOwnerV1>,
    pub(in crate::queue::live) signals: &'a mut Option<CompletionSignalAuthority>,
}

pub(in crate::queue::live) trait PrimaryReleaseParentV1<E: PrimaryEnvironmentV1> {
    fn preflight_release(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn release_parts(
        &mut self,
    ) -> Result<PrimaryReleasePartsV1<'_, E>, ComputeAqlQueueSessionErrorV1>;
    fn poison_release(&mut self);
}

pub(in crate::queue::live) struct PrimaryReleaseStateV1<E: PrimaryReleaseEnvironmentV1> {
    pub(in crate::queue::live) platform: Option<E::Teardown>,
    pub(in crate::queue::live) authority: Option<QueueResourceAuthorityV1>,
    pub(in crate::queue::live) resources: Option<QueueResourceCleanupCustodyV1>,
    pub(in crate::queue::live) dispatch: Option<ReturningControlCleanupCustodyV1>,
    pub(in crate::queue::live) signals: Option<ControlCleanupCustodyV1>,
    pub(in crate::queue::live) gate: Option<E::TeardownArm>,
    pub(in crate::queue::live) destroy: NativeQueueDestroyProgressV1,
    pub(in crate::queue::live) started: bool,
    pub(in crate::queue::live) complete: bool,
}

pub(in crate::queue::live) fn preflight_primary_owners_v1<E: PrimaryReleaseEnvironmentV1>(
    engine: &NativeQueueEngineV1<PrimaryQueueBackendV1<E::Memory>>,
    key: QueueKeyV1,
    signals: Option<&CompletionSignalAuthority>,
    exception: Option<&QueueExceptionStateV1<E>>,
    doorbell: Option<&E::Doorbell>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if engine.phase(key) != Some(ComputeAqlQueuePhaseV1::Active)
        || signals.is_none()
        || !engine.backend.foundation_in_engine
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "primary release owner phase",
        ));
    }
    let authority = engine
        .resources
        .iter()
        .find(|r| r.key == key)
        .and_then(|r| r.authority.as_ref())
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "missing primary resource authority",
        ))?;
    if !matches!(authority.ring, RingAuthority::AqlSpecial(_)) {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "primary release ring profile",
        ));
    }
    engine
        .foundation
        .preflight_memory_transition_revisions(1)
        .map_err(|_| map_native(NativeQueueAdapterErrorV1::ModelProjection))?;
    let exception = exception.ok_or(ComputeAqlQueueSessionErrorV1::Contract(
        "missing primary exception",
    ))?;
    if exception.runtime_control.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "primary release runtime control",
        ));
    }
    E::validate_release_owners(
        exception,
        doorbell.ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?,
        &engine.backend.session,
    )
}

impl<E: PrimaryReleaseEnvironmentV1> PrimaryReleaseStateV1<E>
where
    E::Memory: PrimaryReleaseMemoryV1,
{
    pub(in crate::queue::live) fn new() -> Self {
        Self {
            platform: None,
            authority: None,
            resources: None,
            dispatch: None,
            signals: None,
            gate: None,
            destroy: NativeQueueDestroyProgressV1::default(),
            started: false,
            complete: false,
        }
    }

    pub(in crate::queue::live) fn release_in_place(
        &mut self,
        parent: &mut impl PrimaryReleaseParentV1<E>,
    ) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        if self.started {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "primary release is one-shot",
            ));
        }
        self.started = true;
        let result = catch_unwind(AssertUnwindSafe(|| self.release_inner(parent)));
        match result {
            Ok(Ok(destroyed)) => Ok(destroyed),
            result => {
                parent.poison_release();
                E::poison();
                match result {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                    Ok(Ok(_)) => unreachable!(),
                }
            }
        }
    }

    fn release_inner(
        &mut self,
        parent: &mut impl PrimaryReleaseParentV1<E>,
    ) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        parent.preflight_release()?;
        let parts = parent.release_parts()?;
        self.gate = Some(E::arm_teardown());
        self.platform = Some(E::retain_platform(
            parts.exception.take().expect("preflight exception owner"),
            parts.doorbell.take().expect("preflight doorbell owner"),
        ));
        let engine = parts.engine;
        engine
            .destroy_retaining(parts.key, &mut self.destroy)
            .map_err(map_native)?;
        let platform = self.platform.as_mut().expect("retained platform");
        E::after_queue_destroyed(platform, &engine.backend.session)?;
        E::release_doorbell(platform)?;
        engine.prepare_operation().map_err(map_native)?;
        self.authority = Some(
            engine
                .release_destroyed_resources(parts.key)
                .map_err(map_native)?,
        );
        // Root the removed publication before borrowed, fallible permanent restoration.
        engine
            .backend
            .session
            .restore_foundation(&mut engine.foundation)?;
        engine.backend.foundation_in_engine = false;
        if !matches!(
            self.authority.as_ref().expect("retained authority").ring,
            RingAuthority::AqlSpecial(_)
        ) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "primary release ring substitution",
            ));
        }
        let authority = self.authority.take().expect("validated retained authority");
        let RingAuthority::AqlSpecial(ring) = authority.ring else {
            unreachable!("validated ring")
        };
        self.resources = Some(QueueResourceCleanupCustodyV1::new(
            ring.into_token(),
            authority.control.into_token(),
            authority.eop.into_token(),
            authority.context_save.into_token(),
        ));
        let memory = &mut engine.backend.session;
        E::validate_for_release(platform)?;
        let resources = self.resources.as_mut().expect("retained queue resources");
        memory.release_queue_resources(resources)?;
        if !resources.is_complete() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "incomplete primary resources",
            ));
        }
        E::complete_shadows(platform)?;
        if let Some(dispatch) = parts.dispatch.take() {
            self.dispatch = Some(ReturningControlCleanupCustodyV1::new(
                dispatch,
                ReturningControlModeV1::Ordinary,
            ));
            let dispatch = self.dispatch.as_mut().expect("retained dispatch");
            dispatch.release_ordinary_in_place(memory)?;
            if !dispatch.is_complete() {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "incomplete primary dispatch",
                ));
            }
        }
        self.signals = Some(ControlCleanupCustodyV1::host_data(
            parts
                .signals
                .take()
                .expect("preflight signal arena")
                .into_token(),
        ));
        let signals = self.signals.as_mut().expect("retained signal arena");
        memory.release_signals(signals)?;
        if !signals.is_complete() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "incomplete primary signals",
            ));
        }
        E::confirm_destroyed(self.gate.take().expect("retained outer teardown arm"));
        self.complete = true;
        Ok(destroyed_queue_observation(parts.queue_id))
    }
}

impl PrimaryReleaseParentV1<LinuxPrimaryEnvironmentV1> for ComputeAqlQueueSessionV1 {
    fn preflight_release(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.preflight_primary_release_v1()
    }
    fn release_parts(
        &mut self,
    ) -> Result<PrimaryReleasePartsV1<'_, LinuxPrimaryEnvironmentV1>, ComputeAqlQueueSessionErrorV1>
    {
        Ok(PrimaryReleasePartsV1 {
            engine: self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?,
            key: self.key,
            queue_id: self.observation.queue_id,
            exception: &mut self.exception,
            doorbell: &mut self.doorbell,
            dispatch: &mut self.dispatch,
            signals: &mut self.completion_signals,
        })
    }
    fn poison_release(&mut self) {
        self.poison_terminal();
    }
}

impl PrimaryReleaseMemoryV1 for SharedGttMemorySessionV1 {
    fn restore_foundation(
        &mut self,
        foundation: &mut QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError> {
        self.restore_queue_model_foundation(foundation)
    }
    fn release_queue_resources(
        &mut self,
        resources: &mut QueueResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.release_queue_resources_in_place_v1(resources)
    }
    fn release_signals(
        &mut self,
        signals: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.release_control_in_place_v1(signals)
    }
}

impl PrimaryReleaseEnvironmentV1 for LinuxPrimaryEnvironmentV1 {
    type Teardown = LinuxPrimaryTeardownCustodyV1;
    type TeardownArm = ProcessGlobalKfdRuntimeTeardownArmV1;
    fn validate_release_owners(
        exception: &QueueExceptionStateV1<Self>,
        doorbell: &Self::Doorbell,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(LinuxPrimaryTeardownCustodyV1::validate_owners(
            &exception.runtime,
            &exception.event,
            &exception.shadows,
            doorbell,
            memory.kfd_fd(),
            memory.opener_pid(),
        )?)
    }
    fn arm_teardown() -> Self::TeardownArm {
        arm_process_global_kfd_runtime_gate_for_teardown_v1()
    }
    fn retain_platform(
        exception: QueueExceptionStateV1<Self>,
        doorbell: Self::Doorbell,
    ) -> Self::Teardown {
        LinuxPrimaryTeardownCustodyV1::new(
            exception.runtime,
            exception.event,
            exception.shadows,
            doorbell,
        )
    }
    fn after_queue_destroyed(
        platform: &mut Self::Teardown,
        memory: &Self::Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(platform.after_queue_destroyed(memory.kfd_fd(), memory.opener_pid())?)
    }
    fn release_doorbell(
        platform: &mut Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(platform.release_doorbell()?)
    }
    fn validate_for_release(
        platform: &Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(platform.validate_for_release()?)
    }
    fn complete_shadows(
        platform: &mut Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        Ok(platform.complete_shadows()?)
    }
    fn confirm_destroyed(arm: Self::TeardownArm) {
        arm.confirm_destroyed();
    }
}
