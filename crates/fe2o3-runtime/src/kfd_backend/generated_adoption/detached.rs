//! Root the original completed DATA owner before validating or consuming it.
//!
//! This does not create a generated consumer profile or an ordinary allocation
//! version. A future consumer must independently admit its exact V5 contract.

use super::*;

mod lineage;
use lineage::ProducerLineageV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DetachedPhaseV1 {
    Empty,
    InLower,
    Held,
    Consumed,
}

pub(super) struct RetainedDetachedV1<T> {
    phase: DetachedPhaseV1,
    owner: Option<T>,
    producer: Option<ProducerLineageV1>,
}

impl<T> RetainedDetachedV1<T> {
    pub(super) fn empty() -> Self {
        Self {
            phase: DetachedPhaseV1::Empty,
            owner: None,
            producer: None,
        }
    }

    pub(super) fn is_disposed_or_unentered(&self) -> bool {
        matches!(
            self.phase,
            DetachedPhaseV1::Empty | DetachedPhaseV1::Consumed
        ) && self.owner.is_none()
            && self.producer.is_none()
    }

    pub(super) fn is_held(&self) -> bool {
        self.phase == DetachedPhaseV1::Held && self.owner.is_some()
    }

    fn matches(&self, check: impl FnOnce(&T) -> bool) -> bool {
        self.phase == DetachedPhaseV1::Held && self.owner.as_ref().is_some_and(check)
    }

    fn capture<E>(
        &mut self,
        capture: impl FnOnce() -> Result<T, E>,
    ) -> Result<(), CaptureErrorV1<E>> {
        if self.phase != DetachedPhaseV1::Empty || self.owner.is_some() {
            return Err(CaptureErrorV1::Occupied);
        }
        // Any error or unwind after entry remains terminal in the containing
        // native owner; absence of a returned token never means no effect.
        self.phase = DetachedPhaseV1::InLower;
        let owner = capture().map_err(CaptureErrorV1::Lower)?;
        self.owner = Some(owner);
        self.phase = DetachedPhaseV1::Held;
        Ok(())
    }

    fn take_checked(&mut self, check: impl FnOnce(&T) -> bool) -> Option<T> {
        if self.phase != DetachedPhaseV1::Held || !self.owner.as_ref().is_some_and(check) {
            return None;
        }
        self.phase = DetachedPhaseV1::Consumed;
        self.owner.take()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CaptureErrorV1<E> {
    Occupied,
    Lineage,
    Lower(E),
}

impl GeneratedNativeAdoptionV1 {
    /// Called only inside the original queue/lease retirement envelope. The
    /// source, submission, lane and all graph holds remain rooted by its caller.
    pub(super) fn retain_recycled_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        generation: u64,
        detach: impl FnOnce() -> Result<
            fe2o3_kfd::Gfx942DetachedFixedDispatchV1,
            fe2o3_kfd::ComputeAqlQueueSessionErrorV1,
        >,
    ) -> Result<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        use fe2o3_kfd::ComputeAqlQueueSessionErrorV1 as Error;
        if self.phase != PhaseV1::Retiring {
            return Err(Error::Contract(
                "generated detach requires original retirement envelope",
            ));
        }
        let submission = self.submission.as_ref().ok_or(Error::Contract(
            "generated detach requires original producer submission",
        ))?;
        match self
            .detached
            .capture_producer(plan, submission, generation, detach)
        {
            Ok(()) => {}
            Err(CaptureErrorV1::Lower(error)) => return Err(error),
            Err(CaptureErrorV1::Lineage) => {
                return Err(Error::Contract(
                    "generated detached producer lineage mismatch",
                ));
            }
            Err(CaptureErrorV1::Occupied) => {
                return Err(Error::Contract(
                    "generated original detached DATA already retained",
                ));
            }
        }
        self.phase = PhaseV1::Detached;
        if !self.detached.matches_producer(plan, submission, |owner| {
            (owner.dispatch_generation(), owner.data_lease_count())
        }) {
            return Err(Error::Contract(
                "generated original detached DATA shape mismatch",
            ));
        }
        Ok(())
    }

    pub(super) fn take_retained_data_for_disposal_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<Vec<Gfx942FixedDispatchDataV1>, fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        use fe2o3_kfd::ComputeAqlQueueSessionErrorV1 as Error;
        if !matches!(self.phase, PhaseV1::Detached | PhaseV1::Retiring) {
            return Err(Error::Contract(
                "generated detached DATA disposal phase mismatch",
            ));
        }
        let submission = self.submission.as_ref().ok_or(Error::Contract(
            "generated disposal requires original producer submission",
        ))?;
        let owner = self
            .detached
            .take_producer_checked(plan, submission, |owner| {
                (owner.dispatch_generation(), owner.data_lease_count())
            })
            .ok_or(Error::Contract(
                "generated original detached DATA shape mismatch",
            ))?;
        // into_data only moves the original typed owners. It performs no native
        // operation and creates no replacement byte/content authority.
        let data = owner.into_data();
        self.phase = PhaseV1::Retiring;
        Ok(data)
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn retain_scoped_completed_producer_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        // Other profiles and CPU-only control records retain their existing
        // one-call completion. No synthetic detached owner is constructed.
        if plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
            || self
                .generated_shells
                .get(&plan.key)
                .is_some_and(|record| record.native.is_none())
        {
            return Ok(false);
        }
        self.retain_generated_completed_data_v1(plan, submission)?;
        Ok(true)
    }

    /// Retains the actual lower detached owner on the same original shell.
    /// This private entry neither releases DATA nor admits a consumer, and its
    /// caller must retain the original source authority and Context graph hold.
    pub(in crate::kfd_backend) fn retain_generated_completed_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.generated_submission_owner_matches_v1(submission, plan)
            || !self.generated_lease_matches_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.as_ref().is_some_and(|native| {
                    native.phase == PhaseV1::Adopted
                        && native.detached.phase == DetachedPhaseV1::Empty
                        && native.detached.owner.is_none()
                        && native.submission.as_ref().is_some_and(|owner| {
                            owner.receipt.retirement() == Some(RetirementV1::Recycled)
                        })
                })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated detach requires exact original recycled submission and lease",
            ));
        }
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .expect("validated shell")
            .native
            .as_mut()
            .expect("native owner");
        let handle = native.native_lane.expect("validated original lane");
        native.phase = PhaseV1::Retiring;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native owner");
            let result = self
                .queue
                .as_mut()
                .expect("retained generated queue")
                .with_compute_lane_v1(handle, |lane| {
                    let generation = lane.recycled_fixed_dispatch_generation()?;
                    native.retain_recycled_data_v1(plan, generation, || {
                        lane.detach_recycled_fixed_dispatch()
                    })
                })
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("native detach", error))?;
            let current = self
                .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                    device.model_admission()
                })?;
            if current != plan.binding.native_device || !self.generated_lease_matches_v1(plan) {
                return Err(self.terminal_error("generated closing detach identity mismatch"));
            }
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }
}

#[cfg(test)]
mod tests;
