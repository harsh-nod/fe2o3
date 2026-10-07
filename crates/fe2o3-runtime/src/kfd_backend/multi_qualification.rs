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

fn admit_indexed_qualification_devices_v1(
    unique_ids: &[u64],
    round: usize,
    mut admit: impl FnMut(
        usize,
        usize,
        usize,
    ) -> Result<KfdRuntimeLaunchGateV1, KfdRuntimeBackendErrorV1>,
) -> Result<Vec<(u64, KfdRuntimeLaunchGateV1)>, KfdRuntimeBackendErrorV1> {
    validate_qualification_devices_v1(unique_ids)?;
    let mut gates = Vec::new();
    gates.try_reserve_exact(unique_ids.len()).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "indexed qualification gate roster allocation failed",
        )
    })?;
    for (index, unique_id) in unique_ids.iter().enumerate() {
        gates.push((*unique_id, admit(unique_ids.len(), index, round)?));
    }
    Ok(gates)
}

fn admit_sharded_vecadd_v1(
    count: usize,
    index: usize,
    round: usize,
) -> Result<KfdRuntimeLaunchGateV1, KfdRuntimeBackendErrorV1> {
    crate::qualification_gfx942_sharded_vecadd_v1::admit_gfx942_sharded_vecadd_qualification_v1(
        count, index, round,
    )
    .map(KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecadd)
    .map_err(|error| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            error.to_string(),
        )
    })
}

fn admit_sharded_vecadd_rounds_devices_v1(
    unique_ids: &[u64],
) -> Result<Vec<(u64, KfdRuntimeLaunchGateV1)>, KfdRuntimeBackendErrorV1> {
    admit_indexed_qualification_devices_v1(unique_ids, 0, |count, index, _| {
        crate::qualification_gfx942_sharded_vecadd_rounds_v1::admit_gfx942_sharded_vecadd_rounds_qualification_v1(count, index)
            .map(KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecaddRounds)
            .map_err(|error| KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                error.to_string(),
            ))
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
    /// Opens two exact changed-content rounds per indexed child in one live backend.
    ///
    /// Both recipe closures and the complete ordered UID roster are validated
    /// before native admission. Each child owns an independent, irreversible
    /// two-authorization gate which latches its original A/B/C allocation IDs.
    /// The caller must settle and release the first batch, then refresh every
    /// padded input byte before the second. Authorization is not completion.
    ///
    /// PUBLIC DeviceLocal allocation and native-peer routing use the existing
    /// opt-in policy, including exact pending producer-aware compute events.
    /// Existing constructors, one-shot gates and general launch authority are
    /// unchanged. No live gate replacement or reset is exposed.
    pub fn open_gfx942_sharded_vecadd_rounds_peer_qualification_v1(
        unique_ids: &[u64],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let gates = admit_sharded_vecadd_rounds_devices_v1(unique_ids)?;
        Self::open_default_with_gate_policy_v1(gates, true)
    }

    /// Opens independent one-shot shards of the finite vecadd qualification workload.
    ///
    /// The ordered two-to-eight-device UID roster and every indexed recipe for
    /// `round` (zero or one) are admitted before any native device is opened.
    /// Each child retains its own exact gate; no authority state is shared.
    /// This grants no general kernel authority and changes no existing fixture.
    ///
    /// DeviceLocal storage and eligible copies use the existing native-peer
    /// opt-in policy. An exact producer-aware compute event can defer native peer
    /// execution until successful completion and owner restoration; an ordinary
    /// launch still requires an explicit join before peer admission. Each round
    /// requires a fresh process because these gates are one-shot and native VM
    /// admission retains device history even after successful shutdown.
    pub fn open_gfx942_sharded_vecadd_peer_qualification_v1(
        unique_ids: &[u64],
        round: usize,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let gates =
            admit_indexed_qualification_devices_v1(unique_ids, round, admit_sharded_vecadd_v1)?;
        Self::open_default_with_gate_policy_v1(gates, true)
    }

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
    /// unchanged. Eligible initialized, equal complete persistent allocations
    /// use the bounded native packet plan after dependency resolution and
    /// endpoint quiescence.
    /// Other copies retain the ordinary bounded host-staging path.
    /// Flush/drain publish a retained native ticket, sample its completion once
    /// per progress step, and retire the queue before restoring both owners.
    /// Both children exclude unrelated native work while the ticket is retained;
    /// disjoint child pairs can progress independently. Poll and wait only observe
    /// stored results. Drain deadlines are checked between steps: native ioctl
    /// and currentness checks remain synchronous, without a hard wall-clock bound.
    pub fn open_gfx942_r57_n3_peer_qualification_v2(
        unique_ids: &[u64],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let gates = admit_qualification_devices_v1(unique_ids, admit_r57_n3_v2)?;
        Self::open_default_with_gate_policy_v1(gates, true)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod sharded_tests;

#[cfg(test)]
mod sharded_rounds_tests;
