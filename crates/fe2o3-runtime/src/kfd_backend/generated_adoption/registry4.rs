//! Original common native root; recipe readback never discharges this owner.

use super::*;
use crate::generated_source::GeneratedProfileV1;

mod construction;
mod progress;
mod repeat2;
mod session;
pub(crate) use repeat2::RegistryStorageV1;
use session::Session;

type RegistryReceiptV1 = ReceiptV1<session::Batch, session::Completed>;

pub(in crate::kfd_backend) struct RegistryV1 {
    phase: PhaseV1,
    data: Vec<Gfx942FixedDispatchDataV1>,
    storage: Option<RegistryStorageV1>,
    session: Option<Session>,
    profile: GeneratedProfileV1,
    cycle: u8,
    receipts: [RegistryReceiptV1; 16],
    copied: [bool; 16],
}

impl RegistryV1 {
    fn has_exact_roster(&self) -> bool {
        registry_count(self.profile).is_some_and(|count| {
            self.receipts[count..]
                .iter()
                .all(|receipt| matches!(receipt, ReceiptV1::Ready))
                && self.copied[count..].iter().all(|copied| !*copied)
        })
    }
    pub(in crate::kfd_backend) fn profile(&self) -> GeneratedProfileV1 {
        self.profile
    }
    pub(in crate::kfd_backend) fn is_retired(&self) -> bool {
        self.phase == PhaseV1::Retired
            && self.data.is_empty()
            && self.storage.is_none()
            && self.session.is_none()
            && self.copied[..registry_count(self.profile).unwrap_or(0)]
                .iter()
                .all(|copied| *copied)
            && self.has_exact_roster()
            && self.receipts[..registry_count(self.profile).unwrap_or(0)]
                .iter()
                .all(|r| matches!(r, ReceiptV1::Recycled))
    }

    pub(in crate::kfd_backend) fn with_device<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        self.session
            .as_mut()
            .ok_or(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                "registry original session unavailable",
            ))?
            .with_retained_device_v1(observe)
    }
}

impl KfdRuntimeBackendV1 {
    fn check_registry4_device_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let current = self
            .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                device.model_admission()
            })?;
        if current != plan.binding.native_device
            || self.streams.get(&plan.binding.backend_stream) != Some(&plan.binding.backend_device)
        {
            return Err(self.terminal_error("registry original device or stream mismatch"));
        }
        Ok(())
    }
    pub(crate) fn commit_generated_registry4_shells_v1<E, const N: usize>(
        &mut self,
        authenticated: super::super::generated_shells::GeneratedShellCommitPlanV1,
        source: &mut crate::generated_source::RuntimeGfx942Registry4SourceMutV1<'_, E, N>,
        roster: &GeneratedHostRosterV1,
    ) {
        if !source.matches_roster(roster) {
            std::process::abort();
        }
        self.commit_generated_controls_v1(authenticated, roster, |control| {
            let controls: &mut [_] = match control {
                generated_shells::GeneratedControlV1::Registry4(controls)
                | generated_shells::GeneratedControlV1::Registry4Repeat2(controls) => controls,
                generated_shells::GeneratedControlV1::Registry16(controls) => controls,
                _ => std::process::abort(),
            };
            source.transfer_controls_into(controls)
        });
    }

    fn validate_registry4_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !matches!(
            plan.profile,
            GeneratedProfileV1::NativeFillRegistry4
                | GeneratedProfileV1::NativeFillRegistry4Repeat2
                | GeneratedProfileV1::NativeFillRegistry16
        ) || registry_count(plan.profile) != Some(plan.count)
            || !self.validate_generated_shell_records_v1(plan)
            || !readback::roster_matches_plan_v1(plan, roster)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.source_identity.matches(&roster.source_identity)
                    && record.registry.as_ref().is_some_and(|native| {
                        native.phase == PhaseV1::Adopted
                            && native.session.is_some()
                            && native.profile == plan.profile
                            && native.has_exact_roster()
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry original root, profile or roster mismatch",
            ));
        }
        Ok(())
    }

    pub(crate) fn destroy_generated_registry4_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_registry4_v1(plan, roster)?;
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .and_then(|record| record.registry.as_mut())
            .unwrap_or_else(|| std::process::abort());
        if native.copied[..plan.count].iter().any(|copied| !*copied)
            || !native.receipts[..plan.count]
                .iter()
                .all(|r| matches!(r, ReceiptV1::Recycled))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "registry common backing still has unfinished recipes",
            ));
        }
        native.phase = PhaseV1::Retiring;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            native
                .with_device(|device| device.model_admission())
                .and_then(|actual| {
                    if actual == plan.binding.native_device {
                        Ok(())
                    } else {
                        Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "registry destruction device mismatch",
                        ))
                    }
                })
                .map_err(|error| {
                    self.generated_native_error_v1("registry closing device", error)
                })?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            let result = native
                .session
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .destroy();
            result.map_err(|error| {
                self.generated_native_error_v1("registry common destruction", error)
            })?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            drop(native.session.take());
            native.phase = PhaseV1::Retired;
            self.queue_retired = true;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }
}

fn registry_count(profile: GeneratedProfileV1) -> Option<usize> {
    match profile {
        GeneratedProfileV1::NativeFillRegistry4
        | GeneratedProfileV1::NativeFillRegistry4Repeat2 => Some(4),
        GeneratedProfileV1::NativeFillRegistry16 => Some(16),
        _ => None,
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn registry16_inline_receipt_roster_cannot_hide_state_outside_four_profile() {
        // Ownerless phase markers only, not a native construction/retirement fixture.
        let mut root = RegistryV1 {
            phase: PhaseV1::Entering,
            data: Vec::new(),
            storage: None,
            session: None,
            profile: GeneratedProfileV1::NativeFillRegistry4,
            cycle: 0,
            receipts: core::array::from_fn(|_| ReceiptV1::Ready),
            copied: [false; 16],
        };
        assert!(root.has_exact_roster());
        root.receipts[15] = ReceiptV1::Recycled;
        assert!(!root.has_exact_roster());
        root.receipts[15] = ReceiptV1::Ready;
        root.copied[15] = true;
        assert!(!root.has_exact_roster());
        root.profile = GeneratedProfileV1::NativeFillRegistry16;
        assert!(root.has_exact_roster());
        assert!(!root.is_retired());
        assert!(super::super::typed_receipt::NativeReceiptV1::ready(root.profile).is_none());
        root.profile = GeneratedProfileV1::Singleton;
        assert!(!root.has_exact_roster());
    }
}
