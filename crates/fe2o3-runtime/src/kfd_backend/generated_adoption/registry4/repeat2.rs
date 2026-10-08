//! Profile-preserving dispatch to the same lower receipt machinery.

use super::*;
use fe2o3_kfd::*;

pub(crate) enum RegistryStorageV1 {
    Once(Gfx942NativeFillRegistryStorageV1),
    Repeat2(Gfx942NativeFillRegistryRepeat2StorageV1),
    Sixteen(Gfx942NativeFillResidentRegistryStorageV1<16>),
}

impl RegistryStorageV1 {
    pub(crate) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Once(_) => GeneratedProfileV1::NativeFillRegistry4,
            Self::Repeat2(_) => GeneratedProfileV1::NativeFillRegistry4Repeat2,
            Self::Sixteen(_) => GeneratedProfileV1::NativeFillRegistry16,
        }
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn rearm_generated_registry4_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_registry4_v1(plan, roster)?;
        self.check_registry4_device_v1(plan)?;
        let native = self
            .generated_shells
            .get_mut(&plan.key)
            .and_then(|record| record.registry.as_mut())
            .unwrap_or_else(|| std::process::abort());
        if native.profile != GeneratedProfileV1::NativeFillRegistry4Repeat2
            || native.cycle != 0
            || native.copied[..4] != [true; 4]
            || native.receipts[..4]
                .iter()
                .any(|receipt| !matches!(receipt, ReceiptV1::Recycled))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "registry second cycle not ready",
            ));
        }
        native.phase = PhaseV1::Retiring;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            let Some(Session::Repeat2(session)) = native.session.as_mut() else {
                std::process::abort()
            };
            session.rearm_second_cycle().map_err(|error| {
                self.generated_native_error_v1("registry second-cycle rearm", error)
            })?;
            self.check_registry4_device_v1(plan)?;
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.registry.as_mut())
                .unwrap_or_else(|| std::process::abort());
            native.receipts = core::array::from_fn(|_| ReceiptV1::Ready);
            native.copied = [false; 16];
            native.cycle = 1;
            native.phase = PhaseV1::Adopted;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    pub(crate) fn validate_registry4_cycle_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        cycle: u8,
    ) -> bool {
        self.generated_shells
            .get(&plan.key)
            .and_then(|record| record.registry.as_ref())
            .is_some_and(|native| native.profile == plan.profile && native.cycle == cycle)
    }
}
