//! Parent-rooted ordinary primary release, retaining every destructive prefix.

#![forbid(unsafe_code)]

use super::*;
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue_linux::{
    LinuxPrimaryTeardownCustodyV1, ProcessGlobalKfdRuntimeTeardownArmV1,
    arm_process_global_kfd_runtime_gate_for_teardown_v1,
};
use crate::sdma::retained_release::{
    preflight_retained_sdma_composition_v1, supports_retained_sdma_composition_v1,
};
use crate::shared_memory::{ControlCleanupCustodyV1, QueueResourceCleanupCustodyV1};
use construction_primary::{LinuxPrimaryEnvironmentV1, PrimaryEnvironmentV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[path = "primary_release/driver.rs"]
mod driver;
pub(super) use driver::{
    PrimaryReleaseEnvironmentV1, PrimaryReleaseMemoryV1, PrimaryReleaseStateV1,
    preflight_primary_owners_v1,
};
#[cfg(test)]
pub(super) use driver::{PrimaryReleaseParentV1, PrimaryReleasePartsV1};

pub(super) fn primary_dispatch_release_admitted_v1(
    unpublished: &UnpublishedDispatchStateV1,
    attached: bool,
    recycled: Option<u64>,
    count: usize,
    identities: usize,
    insertion: Option<usize>,
) -> bool {
    count == 0
        && (unpublished.is_clear()
            || unpublished.quiescent(true, attached, recycled, count, identities, insertion))
}

/// Retains the queue, native results and memory owners through terminal teardown.
/// A failed or unfinished owner must remain retained until process termination.
#[must_use = "primary teardown must complete or remain retained until process termination"]
pub struct PrimaryQueueReleaseCustodyV1 {
    parent: ComputeAqlQueueSessionV1,
    state: PrimaryReleaseStateV1<LinuxPrimaryEnvironmentV1>,
}

impl ComputeAqlQueueSessionV1 {
    /// Selects ordinary primary with optional SDMA; errors never authorize fallback.
    pub fn supports_retained_primary_release_v1(
        &self,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        if self.auxiliary_release.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished auxiliary release",
            ));
        }
        self.require_no_sdma_recycle_v1()?;
        self.require_no_sdma_owner_transition_v1()?;
        if self.sdma_allocation.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA allocation",
            ));
        }
        if self.sdma_pool_trim.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unfinished SDMA pool trim",
            ));
        }
        let sdma_supported =
            supports_retained_sdma_composition_v1(self.sdma.as_ref(), self.striped_sdma.as_ref())?;
        if !sdma_supported
            || !self.sdma_pool_free.is_empty()
            || self.sdma_outstanding_buffers != 0
            || self.has_any_persistent_compute_attachment_v1()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.state.is_some())
        {
            return Ok(false);
        }
        let exception = self
            .exception
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing primary exception",
            ))?;
        if exception.runtime_control.is_some() {
            return Ok(false);
        }
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing primary engine",
            ))?;
        let authority = engine
            .resources
            .iter()
            .find(|r| r.key == self.key)
            .and_then(|r| r.authority.as_ref())
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing primary resource authority",
            ))?;
        Ok(matches!(authority.ring, RingAuthority::AqlSpecial(_)))
    }

    /// Borrowed admission, before a caller transfers the queue into its retained root.
    pub fn preflight_primary_release_v1(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if !self.supports_retained_primary_release_v1()?
            || self.terminal_poisoned
            || self.terminal_dependency.is_some()
            || !primary_dispatch_release_admitted_v1(
                &self.unpublished_dispatch,
                self.dispatch.is_some(),
                self.detached_dispatch_generation,
                self.detached_data_count,
                self.detached_data_identities.len(),
                self.detached_next_insertion_index,
            )
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unsupported or busy primary release",
            ));
        }
        self.dependency_owner
            .ensure_idle()
            .map_err(map_dependency_target_use_error_v1)?;
        self.completion_owner.ensure_releasable()?;
        if let Some(dispatch) = &self.dispatch {
            dispatch.ensure_releasable()?;
        }
        preflight_retained_sdma_composition_v1(
            self.sdma.as_ref(),
            self.striped_sdma.as_ref(),
            self.key,
            self.observation.queue_id,
        )?;
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        preflight_primary_owners_v1::<LinuxPrimaryEnvironmentV1>(
            engine,
            self.key,
            self.completion_signals.as_ref(),
            self.exception.as_ref(),
            self.doorbell.as_ref(),
        )
    }
}

impl PrimaryQueueReleaseCustodyV1 {
    /// Moves only. Callers can reserve/install storage before any native effects.
    pub fn new(parent: ComputeAqlQueueSessionV1) -> Self {
        Self {
            parent,
            state: PrimaryReleaseStateV1::new(),
        }
    }

    pub fn poison_after_runtime_owner_failure_v1(&mut self) {
        self.parent.poison_after_runtime_owner_failure_v1();
    }

    pub fn device_backing_usage_v1(&self) -> Option<Gfx942DeviceBackingUsageV1> {
        self.parent.device_backing_usage_v1()
    }

    pub fn host_visible_backing_usage_v1(&self) -> Option<Gfx942HostVisibleBackingUsageV1> {
        self.parent.host_visible_backing_usage_v1()
    }

    pub fn native_backing_usage_v1(
        &self,
    ) -> Option<fe2o3_resource_accounting::ResourceCreditUsageV1> {
        self.parent.native_backing_usage_v1()
    }

    pub fn release_in_place(
        &mut self,
    ) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        self.state.release_in_place(&mut self.parent)
    }
}

impl Drop for PrimaryQueueReleaseCustodyV1 {
    fn drop(&mut self) {
        if !self.state.complete {
            // A public failed root cannot silently discard native owners or the gate.
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing_native_parent() -> ComputeAqlQueueSessionV1 {
        super::super::tests::persistent_compute_cancellation_test_session(
            super::super::tests::test_queue_key(91, 1),
            None,
            None,
        )
    }

    #[test]
    fn primary_release_missing_owner_is_error_not_legacy_profile() {
        let parent = missing_native_parent();
        for result in [
            parent.supports_retained_primary_release_v1().map(|_| ()),
            parent.preflight_primary_release_v1(),
        ] {
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing primary exception"
                ))
            ));
        }
        assert!(!parent.terminal_poisoned);
        assert!(parent.engine.is_none() && parent.exception.is_none());
    }

    #[test]
    fn primary_release_explicit_deferred_profile_is_not_admitted() {
        let mut parent = missing_native_parent();
        parent.sdma_outstanding_buffers = 1;
        assert!(matches!(
            parent.supports_retained_primary_release_v1(),
            Ok(false)
        ));
        assert!(matches!(
            parent.preflight_primary_release_v1(),
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "unsupported or busy primary release"
            ))
        ));
        assert!(!parent.terminal_poisoned);
    }

    #[test]
    fn primary_release_invalid_secondary_is_error_even_when_busy() {
        let mut parent = missing_native_parent();
        parent.sdma_outstanding_buffers = 1;
        parent.striped_sdma = Some(Gfx942SdmaQueueSetV1::Striped {
            owners: Vec::new(),
            next_owner: 0,
        });
        for result in [
            parent.supports_retained_primary_release_v1().map(|_| ()),
            parent.preflight_primary_release_v1(),
        ] {
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Sdma(
                    Gfx942SdmaErrorV1::Contract("orphan secondary SDMA set")
                ))
            ));
        }
        assert!(!parent.terminal_poisoned);
        assert!(parent.striped_sdma.is_some() && parent.sdma.is_none());
    }

    #[test]
    fn primary_release_invalid_logical_mux_is_error_even_when_busy() {
        let mut parent = missing_native_parent();
        parent.sdma_outstanding_buffers = 1;
        parent.sdma = Some(Gfx942SdmaQueueSetV1::LogicalMuxV2 {
            owners: Vec::new(),
            logical_lane_count: 4,
            next_logical_lane: 0,
        });
        for result in [
            parent.supports_retained_primary_release_v1().map(|_| ()),
            parent.preflight_primary_release_v1(),
        ] {
            assert!(matches!(
                result,
                Err(ComputeAqlQueueSessionErrorV1::Sdma(
                    Gfx942SdmaErrorV1::Contract("logical mux SDMA owner roster")
                ))
            ));
        }
        assert!(!parent.terminal_poisoned);
        assert!(parent.striped_sdma.is_none());
        assert!(parent.engine.is_none() && parent.exception.is_none());
        assert_eq!(parent.sdma_outstanding_buffers, 1);
        assert!(matches!(
            parent.sdma.as_ref().unwrap(),
            Gfx942SdmaQueueSetV1::LogicalMuxV2 {
                owners,
                logical_lane_count: 4,
                next_logical_lane: 0,
            } if owners.is_empty()
        ));
    }

    #[test]
    fn primary_release_unfinished_drop_aborts_and_failed_preflight_keeps_parent() {
        use std::os::unix::process::ExitStatusExt;
        const CHILD: &str = "FE2O3_TEST_PRIMARY_RELEASE_DROP";
        const TEST: &str = "queue::live::primary_release::tests::primary_release_unfinished_drop_aborts_and_failed_preflight_keeps_parent";
        let Some(mode) = std::env::var_os(CHILD) else {
            for mode in ["new", "failed"] {
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", TEST, "--nocapture"])
                    .env(CHILD, mode)
                    .output()
                    .unwrap();
                assert_eq!(
                    output.status.signal(),
                    Some(6),
                    "{mode}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        };
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        // Assertion failures must exit normally as test failures, not accidentally
        // trigger the very abort this test is checking during unwinding.
        let mut root =
            std::mem::ManuallyDrop::new(PrimaryQueueReleaseCustodyV1::new(missing_native_parent()));
        let key = root.parent.key;
        if mode == "failed" {
            assert!(matches!(
                root.release_in_place(),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing primary exception"
                ))
            ));
            assert!(root.state.started && root.parent.terminal_poisoned && !root.state.complete);
            assert!(matches!(
                root.release_in_place(),
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "primary release is one-shot"
                ))
            ));
        } else {
            assert_eq!(mode, "new");
        }
        assert_eq!(root.parent.key, key);
        assert_eq!(root.state.destroy, NativeQueueDestroyProgressV1::default());
        assert!(
            root.state.platform.is_none()
                && root.state.authority.is_none()
                && root.state.resources.is_none()
                && root.state.dispatch.is_none()
                && root.state.signals.is_none()
                && root.state.gate.is_none()
        );
        drop(std::mem::ManuallyDrop::into_inner(root));
        panic!("unfinished primary release Drop returned");
    }
}
