use super::*;
use crate::queue::live::primary_release::PrimaryReleaseEnvironmentV1;
use crate::queue_linux::primary_fixture::LocalTeardownArmV1;

pub(in crate::queue::live) struct LocalTeardownV1 {
    exception: QueueExceptionStateV1<Fixture>,
    doorbell: Owner,
    doorbell_released: bool,
    complete: bool,
}

impl LocalTeardownV1 {
    pub(in crate::queue::live::construction_primary::integration_tests) fn identities(
        &self,
    ) -> [OwnerIdentity; 4] {
        [
            self.exception.runtime.identity,
            self.exception.event.identity,
            self.exception.shadows.identity,
            self.doorbell.identity,
        ]
    }
    pub(in crate::queue::live::construction_primary::integration_tests) fn observation(
        &self,
    ) -> (bool, (bool, bool), bool, bool) {
        (
            self.exception
                .event
                .local_event
                .as_ref()
                .unwrap()
                .is_active(),
            self.exception
                .shadows
                .local_published
                .as_ref()
                .unwrap()
                .state(),
            self.doorbell_released,
            self.complete,
        )
    }
    pub(in crate::queue::live::construction_primary::integration_tests) fn progress(
        &self,
    ) -> ((&'static str, bool), u8) {
        (
            self.exception
                .runtime
                .local_runtime
                .as_ref()
                .unwrap()
                .release_observation(),
            self.exception
                .shadows
                .local_published
                .as_ref()
                .unwrap()
                .payload_progress(),
        )
    }
}

impl PrimaryReleaseEnvironmentV1 for Fixture {
    type Teardown = LocalTeardownV1;
    type TeardownArm = LocalTeardownArmV1;

    fn validate_release_owners(
        exception: &QueueExceptionStateV1<Self>,
        doorbell: &Owner,
        memory: &Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("release-validate-owners")?;
        exception.runtime.validate(Role::Runtime, memory)?;
        exception.event.validate(Role::Event, memory)?;
        exception.shadows.validate(Role::Shadow, memory)?;
        doorbell.validate(Role::Doorbell, memory)?;
        exception
            .runtime
            .assert_local_runtime(trace().borrow().local_gate.as_ref().unwrap(), true);
        assert_eq!(
            exception.shadows.shadow.unwrap().1,
            exception.event.identity
        );
        assert_eq!(
            Some(doorbell.doorbell.unwrap().queue_id().value()),
            trace()
                .borrow()
                .destroy_target
                .or_else(|| trace().borrow().create_return.map(|v| v.0))
        );
        exception
            .shadows
            .local_published
            .as_ref()
            .unwrap()
            .validate_event(exception.event.local_event.as_ref().unwrap())?;
        Ok(())
    }
    fn arm_teardown() -> Self::TeardownArm {
        trace().borrow().local_gate.as_ref().unwrap().arm_teardown()
    }
    fn retain_platform(exception: QueueExceptionStateV1<Self>, doorbell: Owner) -> Self::Teardown {
        LocalTeardownV1 {
            exception,
            doorbell,
            doorbell_released: false,
            complete: false,
        }
    }
    fn after_queue_destroyed(
        platform: &mut Self::Teardown,
        _: &Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let exception = &mut platform.exception;
        exception
            .runtime
            .local_runtime
            .as_mut()
            .unwrap()
            .mark_queue_destroyed()?;
        step("destroy-event")?;
        let shadows = exception.shadows.local_published.as_mut().unwrap();
        exception
            .event
            .local_event
            .as_ref()
            .unwrap()
            .destroy_local(shadows)?;
        for (index, name) in ["zero-payload", "protect-payload", "unmap-payload"]
            .into_iter()
            .enumerate()
        {
            step(name)?;
            shadows.release_payload_step(index as u8)?;
        }
        let runtime = exception.runtime.local_runtime.as_mut().unwrap();
        runtime.mark_event_destroyed()?;
        runtime.disable_local(|| {
            step("disable-runtime").map_err(|_| {
                crate::queue_linux::LinuxDoorbellErrorV1::Runtime("fixture runtime disable")
            })
        })?;
        Ok(())
    }
    fn release_doorbell(
        platform: &mut Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("release-doorbell")?;
        assert!(!platform.doorbell_released);
        platform.doorbell_released = true;
        Ok(())
    }
    fn validate_for_release(
        platform: &Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("release-validate-platform")?;
        assert!(
            !platform
                .exception
                .event
                .local_event
                .as_ref()
                .unwrap()
                .is_active()
        );
        assert!(
            platform
                .exception
                .shadows
                .local_published
                .as_ref()
                .unwrap()
                .release_ready()
        );
        assert!(platform.doorbell_released && !platform.complete);
        Ok(())
    }
    fn complete_shadows(
        platform: &mut Self::Teardown,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("complete-shadows")?;
        platform
            .exception
            .runtime
            .local_runtime
            .as_mut()
            .unwrap()
            .complete_shadow_release()?;
        platform.complete = true;
        Ok(())
    }
    fn confirm_destroyed(arm: Self::TeardownArm) {
        arm.confirm();
    }
}
