//! Construction-only native peer policy, without changing launch authority.

use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Opens the selected devices for admitted Worker V3 invocations and native peer copies.
    ///
    /// Reuses the complete device and directed-route admission of
    /// [`Self::open_default_with_native_peer_copy_v1`], including PUBLIC device storage.
    /// Generic kernel, atomic and collective launch authority remains disabled on
    /// every child. Compiler proof and per-invocation admission are still required;
    /// peer visibility does not discharge either obligation.
    pub fn open_worker_v3_generated_only_with_native_peer_copy_v1(
        devices: Vec<u64>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_default_with_gate_policy_v1(generated_only_gates(devices)?, true)
    }

    /// Opens fresh devices with caller-authorized compute and native peer copies.
    ///
    /// The complete bounded UID roster is validated before native admission, and
    /// every device is admitted before any child can create a VM or queue. All
    /// directed XGMI routes must be admissible; a missing route fails construction.
    /// The supplied launch authorities are retained unchanged. This constructor
    /// grants no additional kernel or atomic/collective authority.
    ///
    /// DeviceLocal allocations use KFD PUBLIC backing, isolated from private
    /// cached buffers. HostVisible storage is unchanged. Eligible initialized,
    /// equal-length checked regions use the bounded native packet plan; other copies
    /// retain host staging. This includes prequeued directed scalar chains and
    /// shared Read sources; shared native endpoints serialize without creating a
    /// success dependency between siblings. Flush/drain explicitly progress transfers, retain both
    /// children through queue retirement and owner restoration, and check deadlines
    /// between steps. Poll/wait only observe stored results. Native ioctl/currentness
    /// calls remain synchronous, so a deadline is not a hard syscall time bound.
    /// Existing constructors retain their private-allocation/staged-copy policy.
    pub fn open_default_with_native_peer_copy_v1(
        devices: Vec<(u64, Box<dyn KfdRuntimeLaunchAuthorityV1>)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_with_authorities_v1(devices, KfdRuntimeLaunchGateV1::Production, true)
    }

    /// The native-peer opt-in with the caller's exact semantic launch authorities.
    ///
    /// Allocation, route, fallback and progress rules are identical to
    /// [`Self::open_default_with_native_peer_copy_v1`]. Atomic and collective
    /// profiles and invocation authorization are not widened by peer visibility.
    pub fn open_default_with_semantic_authorities_and_native_peer_copy_v1(
        devices: Vec<(u64, Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>)>,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_with_authorities_v1(devices, KfdRuntimeLaunchGateV1::Semantic, true)
    }

    pub(super) fn open_with_authorities_v1<A>(
        devices: Vec<(u64, A)>,
        gate: impl Fn(A) -> KfdRuntimeLaunchGateV1,
        native_peer_copy: bool,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut gates = Vec::new();
        gates.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device authority roster allocation failed",
            )
        })?;
        gates.extend(
            devices
                .into_iter()
                .map(|(uid, authority)| (uid, gate(authority))),
        );
        if native_peer_copy {
            Self::open_default_with_gate_policy_v1(gates, true)
        } else {
            Self::open_default_with_gates_v1(gates)
        }
    }

    pub(super) fn open_default_with_gate_policy_v1(
        devices: Vec<(u64, KfdRuntimeLaunchGateV1)>,
        native_peer_copy: bool,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        open_with_v1(
            devices,
            native_peer_copy,
            KfdRuntimeBackendV1::open_checked_device_v1,
            KfdRuntimeBackendV1::from_checked_device_with_gate,
            compute_xgmi::admit_native_route_v1,
        )
    }
}

fn generated_only_gates(
    devices: Vec<u64>,
) -> Result<Vec<(u64, KfdRuntimeLaunchGateV1)>, KfdRuntimeBackendErrorV1> {
    let mut gates = Vec::new();
    gates.try_reserve_exact(devices.len()).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "multi-device generated-only roster allocation failed",
        )
    })?;
    gates.extend(
        devices
            .into_iter()
            .map(|uid| (uid, KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)),
    );
    Ok(gates)
}

// The checked-device and route operations are injected here so CPU tests exercise
// the same admission ordering and policy commit without fabricating native devices.
fn open_with_v1<D>(
    devices: Vec<(u64, KfdRuntimeLaunchGateV1)>,
    native_peer_copy: bool,
    mut admit_device: impl FnMut(u64) -> Result<D, KfdRuntimeBackendErrorV1>,
    mut make_child: impl FnMut(D, KfdRuntimeLaunchGateV1) -> KfdRuntimeBackendV1,
    mut admit_route: impl FnMut(
        &KfdRuntimeBackendV1,
        &KfdRuntimeBackendV1,
    ) -> Result<compute_xgmi::Route, KfdRuntimeBackendErrorV1>,
) -> Result<KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendErrorV1> {
    let mut index = multi_admission::reserve_device_index_v1(devices.len())?;
    for (ordinal, (uid, _)) in devices.iter().enumerate() {
        if *uid == 0 || index.insert(*uid, ordinal).is_some() {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device unique IDs must be nonzero and distinct",
            ));
        }
    }
    index.clear();
    let mut checked = Vec::new();
    checked.try_reserve_exact(devices.len()).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "multi-device checked-device roster allocation failed",
        )
    })?;
    let mut children = Vec::new();
    children.try_reserve_exact(devices.len()).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "multi-device child roster allocation failed",
        )
    })?;
    let mut routes = HashMap::new();
    if native_peer_copy {
        routes
            .try_reserve(devices.len() * (devices.len() - 1))
            .map_err(|_| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "compute-XGMI route roster allocation failed",
                )
            })?;
    }
    for (uid, gate) in devices {
        checked.push((admit_device(uid)?, gate));
    }
    for (device, gate) in checked {
        children.push(make_child(device, gate));
    }
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends_with_index_v1(children, index)?;
    if native_peer_copy {
        for (source_index, source) in backend.children.iter().enumerate() {
            for (destination_index, destination) in backend.children.iter().enumerate() {
                if source_index != destination_index {
                    routes.insert(
                        (source_index, destination_index),
                        admit_route(source, destination)?,
                    );
                }
            }
        }
        backend.compute_xgmi_routes = routes;
        for child in &mut backend.children {
            child.peer_visible_device_allocations = true;
        }
    }
    Ok(backend)
}

#[cfg(test)]
mod tests;
