//! Complete, immutable request policy for the multi-device router.

use super::*;
use fe2o3_kfd::{
    Gfx942ComposedBackingDeviceBudgetV1, Gfx942ComposedBackingRootV1,
    Gfx942ComposedBackingSessionBudgetV1,
};

type ComposedDeviceV1<A> = (
    u64,
    A,
    Gfx942ComposedBackingDeviceBudgetV1,
    Gfx942ComposedBackingSessionBudgetV1,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MultiRequestPolicyV1 {
    Legacy,
    Required,
}

fn invalid(detail: &'static str) -> KfdRuntimeBackendErrorV1 {
    KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail)
}

pub(super) fn reserve_device_index_v1(
    count: usize,
) -> Result<HashMap<u64, usize>, KfdRuntimeBackendErrorV1> {
    if !(2..=crate::MAX_RUNTIME_DEVICES_V1).contains(&count) {
        return Err(invalid(
            "multi-device KFD device count is outside runtime bounds",
        ));
    }
    let mut index = HashMap::new();
    index.try_reserve(count).map_err(|_| {
        KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "multi-device routing-table allocation failed",
        )
    })?;
    Ok(index)
}

pub(super) fn classify_children_v1(
    children: &[KfdRuntimeBackendV1],
) -> Result<MultiRequestPolicyV1, KfdRuntimeBackendErrorV1> {
    if !(2..=crate::MAX_RUNTIME_DEVICES_V1).contains(&children.len()) {
        return Err(invalid(
            "multi-device KFD device count is outside runtime bounds",
        ));
    }
    let mut required = 0;
    for (index, child) in children.iter().enumerate() {
        let binding = child
            .request_binding_v1()
            .map_err(|failure| match failure {
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error)
                | RuntimeBackendFailureV1::Terminal(error) => error,
            })?;
        if let Some(binding) = binding {
            required += 1;
            // Admission is bounded by MAX_RUNTIME_DEVICES, never live allocations.
            if children[..index].iter().any(|other| {
                other
                    .composed_request_binding
                    .as_ref()
                    .is_some_and(|other| binding.account().shares_account_with_v1(other.account()))
            }) {
                return Err(invalid("multi-device request accounts must be distinct"));
            }
        }
    }
    match required {
        0 => Ok(MultiRequestPolicyV1::Legacy),
        count if count == children.len() => Ok(MultiRequestPolicyV1::Required),
        _ => Err(invalid(
            "multi-device request policy must cover every child",
        )),
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Admits a complete device roster into one composed request/N1/N2 root.
    /// Each tuple is `(unique_id, authority, device_budget, session_budget)`.
    ///
    /// ```no_run
    /// use fe2o3_runtime::{KfdMultiDeviceRuntimeBackendV1, KfdRuntimeLaunchAuthorityV1};
    /// use fe2o3_kfd::{Gfx942ComposedBackingRootV1, Gfx942ComposedBackingDeviceBudgetV1,
    ///     Gfx942ComposedBackingSessionBudgetV1};
    /// fn open(root: &Gfx942ComposedBackingRootV1, devices: Vec<(u64,
    ///     Box<dyn KfdRuntimeLaunchAuthorityV1>, Gfx942ComposedBackingDeviceBudgetV1,
    ///     Gfx942ComposedBackingSessionBudgetV1)>) {
    ///     let _backend = KfdMultiDeviceRuntimeBackendV1::open_default_with_composed_backing_root_v1(devices, root).unwrap();
    /// }
    /// ```
    pub fn open_default_with_composed_backing_root_v1(
        devices: Vec<ComposedDeviceV1<Box<dyn KfdRuntimeLaunchAuthorityV1>>>,
        root: &Gfx942ComposedBackingRootV1,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_composed_v1(devices, root, KfdRuntimeLaunchGateV1::Production)
    }

    /// The composed profile with exact atomic/collective semantic authorities.
    /// Each tuple is `(unique_id, authority, device_budget, session_budget)`.
    ///
    /// ```no_run
    /// use fe2o3_runtime::{KfdMultiDeviceRuntimeBackendV1, KfdRuntimeSemanticLaunchAuthorityV1};
    /// use fe2o3_kfd::{Gfx942ComposedBackingRootV1, Gfx942ComposedBackingDeviceBudgetV1,
    ///     Gfx942ComposedBackingSessionBudgetV1};
    /// fn open(root: &Gfx942ComposedBackingRootV1, devices: Vec<(u64,
    ///     Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>, Gfx942ComposedBackingDeviceBudgetV1,
    ///     Gfx942ComposedBackingSessionBudgetV1)>) {
    ///     let _backend = KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_and_composed_backing_root_v1(devices, root).unwrap();
    /// }
    /// ```
    pub fn open_default_with_semantic_authorities_and_composed_backing_root_v1(
        devices: Vec<ComposedDeviceV1<Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>>>,
        root: &Gfx942ComposedBackingRootV1,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::open_composed_v1(devices, root, KfdRuntimeLaunchGateV1::Semantic)
    }

    fn open_composed_v1<A>(
        devices: Vec<ComposedDeviceV1<A>>,
        root: &Gfx942ComposedBackingRootV1,
        gate: impl Fn(A) -> KfdRuntimeLaunchGateV1,
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let mut index = reserve_device_index_v1(devices.len())?;
        for (ordinal, (uid, _, _, _)) in devices.iter().enumerate() {
            if *uid == 0 || index.insert(*uid, ordinal).is_some() {
                return Err(invalid(
                    "multi-device unique IDs must be nonzero and distinct",
                ));
            }
        }
        index.clear();
        let mut checked = Vec::new();
        let mut children = Vec::new();
        checked.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device checked-device roster allocation failed",
            )
        })?;
        children.try_reserve_exact(devices.len()).map_err(|_| {
            KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Capacity,
                "multi-device child roster allocation failed",
            )
        })?;
        // Complete the process-wide checked-device admission before minting
        // any root session. No child can lazily create a VM or queue here.
        for (uid, authority, device_budget, session_budget) in devices {
            checked.push((
                KfdRuntimeBackendV1::open_checked_device_v1(uid)?,
                gate(authority),
                device_budget,
                session_budget,
            ));
        }
        for (device, gate, device_budget, session_budget) in checked {
            children.push(
                KfdRuntimeBackendV1::from_checked_device_with_composed_gate_v1(
                    device,
                    gate,
                    root,
                    device_budget,
                    session_budget,
                )?,
            );
        }
        Self::from_backends_with_index_v1(children, index)
    }

    pub(super) fn request_profile_v1(
        &self,
    ) -> Result<
        crate::RuntimeAllocationAdmissionProfileV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        self.require_live()?;
        let policy = classify_children_v1(&self.children).map_err(|error| {
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal {
                RuntimeBackendFailureV1::Terminal(error)
            } else {
                RuntimeBackendFailureV1::Rejected(error)
            }
        })?;
        if policy != self.request_policy
            || self.device_children.len() != self.children.len()
            || self.children.iter().enumerate().any(|(index, child)| {
                child.description.backend_device == 0
                    || self.device_children.get(&child.description.backend_device) != Some(&index)
            })
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "multi-device request roster changed",
            ));
        }
        if policy == MultiRequestPolicyV1::Legacy {
            return Ok(crate::RuntimeAllocationAdmissionProfileV1::Legacy);
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.children.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("multi-device request admission roster"))?;
        for child in &self.children {
            entries.push(
                child
                    .composed_request_binding
                    .as_ref()
                    .expect("validated required roster")
                    .clone(),
            );
        }
        Ok(crate::RuntimeAllocationAdmissionProfileV1::Required(
            entries,
        ))
    }
}

#[cfg(test)]
mod tests;
