//! Retained auxiliary teardown while the primary keeps the shared foundation.

#![forbid(unsafe_code)]

use super::*;
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::shared_memory::{ControlCleanupCustodyV1, QueueResourceCleanupCustodyV1};
use construction_primary::{LinuxPrimaryEnvironmentV1, PrimaryEnvironmentV1};
use primary_release::{
    PrimaryReleaseEnvironmentV1, PrimaryReleaseMemoryV1, preflight_primary_owners_v1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuxiliaryReleaseIdentityV1 {
    pub(super) index: usize,
    pub(super) generation: u64,
    pub(super) key: QueueKeyV1,
}

pub(super) struct AuxiliaryReleaseCustodyV1<E: PrimaryReleaseEnvironmentV1> {
    pub(super) identity: AuxiliaryReleaseIdentityV1,
    pub(super) platform: Option<E::Teardown>,
    pub(super) authority: Option<QueueResourceAuthorityV1>,
    pub(super) resources: Option<QueueResourceCleanupCustodyV1>,
    pub(super) dispatch: Option<ReturningControlCleanupCustodyV1>,
    pub(super) signals: Option<ControlCleanupCustodyV1>,
    pub(super) gate: Option<E::TeardownArm>,
    pub(super) destroy: NativeQueueDestroyProgressV1,
    pub(super) memory_complete: bool,
}

impl<E: PrimaryReleaseEnvironmentV1> AuxiliaryReleaseCustodyV1<E> {
    fn new(identity: AuxiliaryReleaseIdentityV1) -> Self {
        Self {
            identity,
            platform: None,
            authority: None,
            resources: None,
            dispatch: None,
            signals: None,
            gate: None,
            destroy: NativeQueueDestroyProgressV1::default(),
            memory_complete: false,
        }
    }
}

pub(super) struct AuxiliaryReleasePartsV1<'a, E: PrimaryReleaseEnvironmentV1> {
    pub(super) engine: &'a mut NativeQueueEngineV1<PrimaryQueueBackendV1<E::Memory>>,
    pub(super) session_key: QueueKeyV1,
    pub(super) dependency: &'a ComputeDependencySessionOwnerV1,
    pub(super) lanes: &'a mut [AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1<E>>],
    pub(super) custody: &'a mut Option<AuxiliaryReleaseCustodyV1<E>>,
}

pub(super) trait AuxiliaryReleaseContextV1 {
    type Environment: PrimaryReleaseEnvironmentV1;
    fn admit(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn parts(
        &mut self,
    ) -> Result<AuxiliaryReleasePartsV1<'_, Self::Environment>, ComputeAqlQueueSessionErrorV1>;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison(&mut self);
}

fn retained_lane<E: PrimaryReleaseEnvironmentV1>(
    lanes: &mut [AuxiliaryComputeLaneSlotV1<ComputeAqlQueueLaneStateV1<E>>],
    identity: AuxiliaryReleaseIdentityV1,
) -> Result<&mut ComputeAqlQueueLaneStateV1<E>, ComputeAqlQueueSessionErrorV1> {
    lanes
        .get_mut(identity.index)
        .filter(|slot| slot.generation == identity.generation)
        .and_then(|slot| slot.state.as_mut())
        .filter(|state| state.key == identity.key)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "auxiliary release owner substitution",
        ))
}

fn poison_preserving_failure<C: AuxiliaryReleaseContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
    core::mem::forget(catch_unwind(AssertUnwindSafe(C::Environment::poison)));
}

pub(super) fn release_in_place<C: AuxiliaryReleaseContextV1>(
    context: &mut C,
    lane: ComputeAqlQueueLaneV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1>
where
    <C::Environment as construction_primary::PrimaryEnvironmentV1>::Memory: PrimaryReleaseMemoryV1,
{
    context.admit()?;
    let parts = context.parts()?;
    if parts.custody.is_some() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "auxiliary release is one-shot",
        ));
    }
    let AdmittedComputeLaneV1::Auxiliary(index) =
        admit_compute_lane_v1(parts.session_key, parts.lanes, lane)?
    else {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "the primary queue is destroyed with its session",
        ));
    };
    parts
        .dependency
        .ensure_idle()
        .map_err(map_dependency_target_use_error_v1)?;
    let slot = &parts.lanes[index];
    let state = slot.state.as_ref().expect("admitted auxiliary lane");
    preflight_auxiliary_compute_lane_destroy_v1(state)?;
    preflight_primary_owners_v1::<C::Environment>(
        parts.engine,
        state.key,
        state.completion_signals.as_ref(),
        state.exception.as_ref(),
        state.doorbell.as_ref(),
    )?;
    *parts.custody = Some(AuxiliaryReleaseCustodyV1::new(AuxiliaryReleaseIdentityV1 {
        index,
        generation: slot.generation,
        key: state.key,
    }));

    let result = catch_unwind(AssertUnwindSafe(|| release_inner(context)));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            poison_preserving_failure(context);
            Err(error)
        }
        Err(payload) => {
            poison_preserving_failure(context);
            resume_unwind(payload)
        }
    }
}

fn release_inner<C: AuxiliaryReleaseContextV1>(
    context: &mut C,
) -> Result<(), ComputeAqlQueueSessionErrorV1>
where
    <C::Environment as construction_primary::PrimaryEnvironmentV1>::Memory: PrimaryReleaseMemoryV1,
{
    {
        let parts = context.parts()?;
        let root = parts.custody.as_mut().expect("installed auxiliary release");
        let state = retained_lane(parts.lanes, root.identity)?;
        root.gate = Some(C::Environment::arm_teardown());
        root.platform = Some(C::Environment::retain_platform(
            state
                .exception
                .take()
                .expect("preflight auxiliary exception"),
            state.doorbell.take().expect("preflight auxiliary doorbell"),
        ));
        parts
            .engine
            .destroy_retaining(state.key, &mut root.destroy)
            .map_err(map_native)?;
        let platform = root.platform.as_mut().expect("retained auxiliary platform");
        C::Environment::after_queue_destroyed(platform, &parts.engine.backend.session)?;
        C::Environment::release_doorbell(platform)?;
        parts.engine.prepare_operation().map_err(map_native)?;
        root.authority = Some(
            parts
                .engine
                .release_destroyed_resources(state.key)
                .map_err(map_native)?,
        );
        if !matches!(
            root.authority
                .as_ref()
                .expect("retained auxiliary authority")
                .ring,
            RingAuthority::AqlSpecial(_)
        ) {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "auxiliary release ring substitution",
            ));
        }
        let authority = root
            .authority
            .take()
            .expect("validated auxiliary authority");
        let RingAuthority::AqlSpecial(ring) = authority.ring else {
            unreachable!("validated ring");
        };
        root.resources = Some(QueueResourceCleanupCustodyV1::new(
            ring.into_token(),
            authority.control.into_token(),
            authority.eop.into_token(),
            authority.context_save.into_token(),
        ));
    }

    // The primary and any SDMA neighbors still own this live shared foundation.
    let (cleanup, retake) = execute_live_model_custody_v1(
        context,
        C::loan,
        release_memory::<C>,
        C::retake,
        poison_preserving_failure,
    )?;
    retake?;
    cleanup?;
    let parts = context.parts()?;
    let root = parts.custody.as_mut().expect("retained auxiliary cleanup");
    retained_lane(parts.lanes, root.identity)?;
    if !root.memory_complete || !parts.engine.backend.foundation_in_engine {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "incomplete auxiliary cleanup settlement",
        ));
    }
    parts.engine.prepare_operation().map_err(map_native)?;
    C::Environment::confirm_destroyed(root.gate.take().expect("retained auxiliary gate"));
    // Vacancy is published only after native cleanup, model retake and currentness.
    parts.lanes[root.identity.index].state = None;
    *parts.custody = None;
    Ok(())
}

fn release_memory<C: AuxiliaryReleaseContextV1>(
    context: &mut C,
) -> Result<(), ComputeAqlQueueSessionErrorV1>
where
    <C::Environment as construction_primary::PrimaryEnvironmentV1>::Memory: PrimaryReleaseMemoryV1,
{
    let parts = context.parts()?;
    let root = parts
        .custody
        .as_mut()
        .expect("retained auxiliary resources");
    let state = retained_lane(parts.lanes, root.identity)?;
    let memory = &mut parts.engine.backend.session;
    let platform = root.platform.as_mut().expect("retained auxiliary platform");
    C::Environment::validate_for_release(platform)?;
    let resources = root
        .resources
        .as_mut()
        .expect("retained auxiliary resources");
    memory.release_queue_resources(resources)?;
    if !resources.is_complete() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "incomplete auxiliary resources",
        ));
    }
    C::Environment::complete_shadows(platform)?;
    if let Some(dispatch) = state.dispatch.take() {
        root.dispatch = Some(ReturningControlCleanupCustodyV1::new(
            dispatch,
            ReturningControlModeV1::Ordinary,
        ));
        let dispatch = root.dispatch.as_mut().expect("retained auxiliary dispatch");
        dispatch.release_ordinary_in_place(memory)?;
        if !dispatch.is_complete() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "incomplete auxiliary dispatch",
            ));
        }
    }
    root.signals = Some(ControlCleanupCustodyV1::host_data(
        state
            .completion_signals
            .take()
            .expect("preflight auxiliary signals")
            .into_token(),
    ));
    let signals = root.signals.as_mut().expect("retained auxiliary signals");
    memory.release_signals(signals)?;
    if !signals.is_complete() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "incomplete auxiliary signals",
        ));
    }
    root.memory_complete = true;
    Ok(())
}

impl AuxiliaryReleaseContextV1 for ComputeAqlQueueSessionV1 {
    type Environment = LinuxPrimaryEnvironmentV1;

    fn admit(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if self.auxiliary_release.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "auxiliary release is one-shot",
            ));
        }
        Ok(())
    }

    fn parts(
        &mut self,
    ) -> Result<AuxiliaryReleasePartsV1<'_, Self::Environment>, ComputeAqlQueueSessionErrorV1> {
        Ok(AuxiliaryReleasePartsV1 {
            engine: self
                .engine
                .as_mut()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?,
            session_key: self.compute_lane_session,
            dependency: &self.dependency_owner,
            lanes: &mut self.auxiliary_compute_lanes,
            custody: &mut self.auxiliary_release,
        })
    }

    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }

    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }

    fn poison(&mut self) {
        self.poison_terminal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unfinished_shell() -> ComputeAqlQueueSessionV1 {
        let key = super::super::tests::test_queue_key(95, 1);
        let mut session =
            super::super::tests::persistent_compute_cancellation_test_session(key, None, None);
        // Drop/admission guard only: no native owner is manufactured by this fixture.
        session.auxiliary_release =
            Some(AuxiliaryReleaseCustodyV1::new(AuxiliaryReleaseIdentityV1 {
                index: 0,
                generation: 1,
                key,
            }));
        session
    }

    #[test]
    fn unfinished_auxiliary_release_rejects_reentry_and_primary_fallback_before_missing_engine() {
        let mut session = unfinished_shell();
        let lane = session.primary_compute_lane_v1();
        assert!(matches!(
            session.destroy_auxiliary_compute_lane_v1(lane),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "auxiliary release is one-shot"
            ))
        ));
        assert!(matches!(
            session.supports_retained_primary_release_v1(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished auxiliary release"
            ))
        ));
        assert!(session.engine.is_none() && !session.terminal_poisoned);
        session.auxiliary_release = None;
    }

    #[test]
    fn unfinished_auxiliary_release_drop_aborts() {
        use std::os::unix::process::ExitStatusExt;
        const CHILD: &str = "FE2O3_TEST_AUXILIARY_RELEASE_DROP";
        const TEST: &str =
            "queue::live::auxiliary_release::tests::unfinished_auxiliary_release_drop_aborts";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .output()
                .unwrap();
            assert_eq!(
                output.status.signal(),
                Some(6),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        drop(unfinished_shell());
        panic!("unfinished auxiliary root returned from Drop");
    }
}
