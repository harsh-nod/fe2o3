//! Exact-fixture multi-device admission without exposing live child composition.

use super::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendErrorV1,
    KfdRuntimeLaunchGateV1,
};

const MAX_QUALIFICATION_DEVICES_V1: usize = 8;

pub(super) fn validate_qualification_devices_v1(
    unique_ids: &[u64],
) -> Result<(), KfdRuntimeBackendErrorV1> {
    if !(2..=MAX_QUALIFICATION_DEVICES_V1).contains(&unique_ids.len()) {
        return Err(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            "multi-device qualification requires two to eight devices",
        ));
    }
    for (index, unique_id) in unique_ids.iter().enumerate() {
        if *unique_id == 0 || unique_ids[..index].contains(unique_id) {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device qualification unique IDs must be nonzero and distinct",
            ));
        }
    }
    Ok(())
}

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Opens two to eight devices for the exact repository-owned vecadd fixture.
    ///
    /// Every UID and fixture is validated before native device admission. The
    /// existing multi-device constructor then admits every device before any
    /// child may create a VM or queue. This grants no production launch authority;
    /// peer copies retain the ordinary bounded host-staging implementation.
    pub fn open_gfx942_vecadd_qualification_v1(
        unique_ids: &[u64],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        validate_qualification_devices_v1(unique_ids)?;
        let mut gates = Vec::new();
        gates.try_reserve_exact(unique_ids.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device qualification gate roster allocation failed",
            )
        })?;
        for unique_id in unique_ids {
            let admitted =
                crate::qualification_gfx942_vecadd_v1::admit_gfx942_vecadd_qualification_v1()
                    .map_err(|error| {
                        KfdRuntimeBackendErrorV1::new(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            error.to_string(),
                        )
                    })?;
            gates.push((
                *unique_id,
                KfdRuntimeLaunchGateV1::ExactGfx942Vecadd(admitted),
            ));
        }
        Self::open_default_with_gates_v1(gates)
    }
}

#[cfg(test)]
mod tests;
