//! Immutable request custody for the original two native XGMI endpoints.

#![forbid(unsafe_code)]

use super::*;
use crate::{
    RuntimeAllocationAdmissionProfileV1, RuntimeAllocationDeviceAdmissionV1,
    RuntimeAllocationRequestWitnessV1,
};
use fe2o3_kfd::{
    Gfx942ComposedBackingDeviceBudgetV1, Gfx942ComposedBackingRootV1,
    Gfx942ComposedBackingSessionBudgetV1,
};
use xgmi_budget::EndpointAdmissionV1;

#[allow(
    clippy::large_enum_variant,
    reason = "one fixed pair per backend avoids an extra pre-VM allocation"
)]
pub(super) enum RequestPolicyV1 {
    Legacy,
    Required([RuntimeAllocationDeviceAdmissionV1; 2]),
}

fn invalid(detail: &'static str) -> KfdRuntimeBackendErrorV1 {
    KfdRuntimeBackendErrorV1::new(KfdRuntimeBackendErrorKindV1::InvalidLaunch, detail)
}

impl RequestPolicyV1 {
    pub(super) fn from_admissions(
        devices: [&CheckedGfx942XnackMinusDevice; 2],
        admissions: &[EndpointAdmissionV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let keys = devices.map(|device| device.observation().unique_id());
        let bindings =
            xgmi_budget::admit_endpoints(devices, admissions.each_ref(), |device, admission| {
                match admission {
                    EndpointAdmissionV1::Composed(admission) => {
                        // Validate the whole request/N1/N2 bundle before either VM,
                        // not only the request handle cloned into Runtime's binding.
                        if !admission.matches_device_v1(device) {
                            return Err(invalid("native XGMI composed endpoint mismatch"));
                        }
                        RuntimeAllocationDeviceAdmissionV1::for_checked_device_v1(
                            device.observation().unique_id(),
                            device,
                            admission.request_account_v1().clone(),
                        )
                        .map(Some)
                        .map_err(native_budget::rooted_host_backing_admission_error_v1)
                    }
                    EndpointAdmissionV1::Local(_) | EndpointAdmissionV1::Native(_) => Ok(None),
                }
            })?;
        Self::from_bindings(keys, bindings)
    }

    fn from_bindings(
        keys: [u64; 2],
        bindings: [Option<RuntimeAllocationDeviceAdmissionV1>; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        let policy = match bindings {
            [None, None] => Self::Legacy,
            [Some(first), Some(second)] => Self::Required([first, second]),
            _ => {
                return Err(invalid(
                    "native XGMI request policy must cover both endpoints",
                ));
            }
        };
        if let Some(entries) = policy.bindings(keys)?
            && entries.iter().any(|entry| !entry.is_live())
        {
            return Err(invalid("unhealthy native XGMI request session"));
        }
        Ok(policy)
    }

    fn bindings(
        &self,
        keys: [u64; 2],
    ) -> Result<Option<&[RuntimeAllocationDeviceAdmissionV1; 2]>, KfdRuntimeBackendErrorV1> {
        if admit_xgmi_unique_id_pair_v1(keys[0], keys[1]).is_err() {
            return Err(invalid(
                "native XGMI request endpoints must be distinct and nonzero",
            ));
        }
        let Self::Required(entries) = self else {
            return Ok(None);
        };
        if entries
            .iter()
            .zip(keys)
            .any(|(entry, key)| entry.backend_device_v1() != key)
            || entries[0]
                .account()
                .shares_account_with_v1(entries[1].account())
        {
            return Err(invalid("native XGMI request roster mismatch"));
        }
        Ok(Some(entries))
    }

    pub(super) fn profile(
        &self,
        keys: [u64; 2],
    ) -> Result<
        RuntimeAllocationAdmissionProfileV1,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let Some(entries) = self
            .bindings(keys)
            .map_err(RuntimeBackendFailureV1::Rejected)?
        else {
            return Ok(RuntimeAllocationAdmissionProfileV1::Legacy);
        };
        if entries.iter().any(|entry| !entry.is_live()) {
            return Err(RuntimeBackendFailureV1::Rejected(invalid(
                "unhealthy native XGMI request session",
            )));
        }
        let mut roster = Vec::new();
        roster
            .try_reserve_exact(2)
            .map_err(|_| KfdRuntimeBackendV1::capacity("native XGMI request roster"))?;
        roster.extend(entries.iter().cloned());
        Ok(RuntimeAllocationAdmissionProfileV1::Required(roster))
    }

    pub(super) fn allocation_endpoint(
        &self,
        keys: [u64; 2],
        device: u64,
        bytes: u64,
        witness: Option<RuntimeAllocationRequestWitnessV1<'_>>,
    ) -> Result<usize, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let index = keys.iter().position(|key| *key == device).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "unknown native XGMI device",
            )
        })?;
        let entries = self
            .bindings(keys)
            .map_err(RuntimeBackendFailureV1::Rejected)?;
        match (entries, witness) {
            (None, None) => Ok(index),
            (Some(entries), Some(witness)) if witness.matches_v1(&entries[index], bytes) => {
                Ok(index)
            }
            (Some(_), None) => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "composed native XGMI allocation requires a Context request witness",
            )),
            _ => Err(RuntimeBackendFailureV1::Rejected(invalid(
                "missing or mismatched native XGMI request witness",
            ))),
        }
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    /// Opens both endpoints with mandatory Context request and native backing
    /// accounts. Both budget arrays use the original endpoint argument order.
    /// Both root admissions and Runtime binding preflight checks precede either
    /// VM acquisition. Lower native account installation remains fallible.
    ///
    /// ```no_run
    /// use fe2o3_runtime::KfdNativeXgmiRuntimeBackendV1;
    /// use fe2o3_kfd::{Gfx942ComposedBackingRootV1, Gfx942ComposedBackingDeviceBudgetV1,
    ///     Gfx942ComposedBackingSessionBudgetV1};
    /// fn open(root: &Gfx942ComposedBackingRootV1, devices: [u64; 2],
    ///     parents: [Gfx942ComposedBackingDeviceBudgetV1; 2],
    ///     sessions: [Gfx942ComposedBackingSessionBudgetV1; 2]) {
    ///     let _backend = KfdNativeXgmiRuntimeBackendV1::open_default_with_composed_backing_root_v1(
    ///         devices[0], devices[1], root, parents, sessions).unwrap();
    /// }
    /// ```
    pub fn open_default_with_composed_backing_root_v1(
        first_unique_id: u64,
        second_unique_id: u64,
        root: &Gfx942ComposedBackingRootV1,
        device_budgets: [Gfx942ComposedBackingDeviceBudgetV1; 2],
        session_budgets: [Gfx942ComposedBackingSessionBudgetV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        if admit_xgmi_unique_id_pair_v1(first_unique_id, second_unique_id).is_err() {
            return Err(invalid(
                "native XGMI requires two distinct nonzero unique IDs",
            ));
        }
        let first = KfdRuntimeBackendV1::open_checked_device_v1(first_unique_id)?;
        let second = KfdRuntimeBackendV1::open_checked_device_v1(second_unique_id)?;
        Self::from_checked_pair_with_composed_backing_root_v1(
            first,
            second,
            root,
            device_budgets,
            session_budgets,
        )
    }

    /// Binds both complete composed admissions before consuming either checked
    /// device into a VM. Second VM acquisition failure remains process-fatal.
    pub fn from_checked_pair_with_composed_backing_root_v1(
        first: CheckedGfx942XnackMinusDevice,
        second: CheckedGfx942XnackMinusDevice,
        root: &Gfx942ComposedBackingRootV1,
        device_budgets: [Gfx942ComposedBackingDeviceBudgetV1; 2],
        session_budgets: [Gfx942ComposedBackingSessionBudgetV1; 2],
    ) -> Result<Self, KfdRuntimeBackendErrorV1> {
        Self::from_checked_pair_with_admissions_v1(first, second, |devices| {
            xgmi_budget::admit_endpoints(
                devices,
                [
                    (device_budgets[0], session_budgets[0]),
                    (device_budgets[1], session_budgets[1]),
                ],
                |device, (parent, session)| {
                    root.admit_session_v1(device, parent, session)
                        .map(EndpointAdmissionV1::Composed)
                        .map_err(native_budget::rooted_host_backing_admission_error_v1)
                },
            )
        })
    }

    pub(super) fn allocate_xgmi_request_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
        witness: Option<RuntimeAllocationRequestWitnessV1<'_>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let index = self.request_policy.allocation_endpoint(
            self.descriptions
                .each_ref()
                .map(|entry| entry.backend_device),
            device,
            byte_len,
            witness,
        )?;
        if kind != RuntimeMemoryKindV1::DeviceLocal {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native XGMI exposes PUBLIC device-local allocations only",
            ));
        }
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "native XGMI allocation geometry",
            ));
        }
        xgmi_budget::allocate_record(
            &mut self.terminal,
            &mut self.next_handle,
            &mut self.allocations,
            || {
                self.sessions[index]
                    .allocate_gfx942_xgmi_device_memory_classified_v1(byte_len, alignment)
                    .map(|lease| XgmiRuntimeAllocationV1 {
                        device: index,
                        byte_len,
                        alignment,
                        authority: Some(XgmiAllocationAuthorityV1::Unmapped(lease)),
                    })
            },
            fe2o3_kfd::Gfx942XgmiAllocationFailureV1::disposition,
        )
    }
}

#[cfg(test)]
mod tests;
