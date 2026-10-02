//! Exact-fixture multi-device admission without exposing live child composition.

use super::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendErrorKindV1, KfdRuntimeBackendErrorV1,
    KfdRuntimeLaunchGateV1,
};

const MAX_QUALIFICATION_DEVICES_V1: usize = 8;

fn admit_qualification_devices_v1(
    unique_ids: &[u64],
    mut admit: impl FnMut() -> Result<KfdRuntimeLaunchGateV1, KfdRuntimeBackendErrorV1>,
) -> Result<Vec<(u64, KfdRuntimeLaunchGateV1)>, KfdRuntimeBackendErrorV1> {
    validate_qualification_devices_v1(unique_ids)?;
    let mut gates = Vec::new();
    gates.try_reserve_exact(unique_ids.len()).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "multi-device qualification gate roster allocation failed",
        )
    })?;
    for unique_id in unique_ids {
        gates.push((*unique_id, admit()?));
    }
    Ok(gates)
}

fn admit_r57_n3_v2() -> Result<KfdRuntimeLaunchGateV1, KfdRuntimeBackendErrorV1> {
    crate::qualification_gfx942_r57_n3_v1::admit_gfx942_r57_n3_qualification_v2()
        .map(KfdRuntimeLaunchGateV1::ExactGfx942R57N3V2)
        .map_err(|error| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            )
        })
}

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
        let gates = admit_qualification_devices_v1(unique_ids, || {
            crate::qualification_gfx942_vecadd_v1::admit_gfx942_vecadd_qualification_v1()
                .map(KfdRuntimeLaunchGateV1::ExactGfx942Vecadd)
                .map_err(|error| {
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        error.to_string(),
                    )
                })
        })?;
        Self::open_default_with_gates_v1(gates)
    }

    /// Opens two to eight devices for independent exact DeviceLocal R57 N3 V2 sequences.
    ///
    /// Each child retains its own unchanged two-launch authority: `A+B -> C`,
    /// then `C+B -> D` using that child's exact local allocation identities.
    /// No authority state or allocation identity is shared between devices.
    /// All fixtures are admitted before native device admission begins.
    ///
    /// This constructor does not enable PUBLIC allocation, native XGMI routing
    /// or general kernel authority. Peer copies retain the ordinary bounded
    /// host-staging implementation; this is not a native pipeline witness.
    pub fn open_gfx942_r57_n3_qualification_v2(
        unique_ids: &[u64],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let gates = admit_qualification_devices_v1(unique_ids, admit_r57_n3_v2)?;
        Self::open_default_with_gates_v1(gates)
    }

    /// Opens the exact R57 N3 V2 fixture with peer-visible DeviceLocal backing.
    ///
    /// This opt-in selects genuine KFD PUBLIC allocations before any child
    /// queue or allocation exists. Private and PUBLIC cached buffers remain
    /// separate; HostVisible storage and the existing launch policies are
    /// unchanged. Equal complete persistent allocations within one native copy
    /// packet use XGMI after dependency resolution and endpoint quiescence.
    /// Other copies retain the ordinary bounded host-staging path.
    /// This development profile executes each native transfer synchronously in
    /// flush/drain, with a 30-second completion wait. Drain checks its deadline
    /// between steps, not within this transfer; poll and wait remain observers.
    pub fn open_gfx942_r57_n3_peer_qualification_v2(
        unique_ids: &[u64],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut backend = Self::open_gfx942_r57_n3_qualification_v2(unique_ids)?;
        backend.admit_compute_xgmi_routes_v1()?;
        for child in &mut backend.children {
            child.peer_visible_device_allocations = true;
        }
        Ok(backend)
    }
}

#[cfg(test)]
mod tests;
