//! Required request accounting at the Context/backend boundary.

use super::*;
use fe2o3_kfd::{CheckedGfx942XnackMinusDevice, Gfx942RequestAccountV1, Gfx942RetainedRequestV1};
use fe2o3_runtime_model::ModelDeviceAdmissionV1;

/// An immutable backend-device binding to one already-minted request leaf.
/// This is accounting custody, not authority to execute native operations.
#[derive(Clone)]
pub struct RuntimeAllocationDeviceAdmissionV1 {
    backend_device: u64,
    model: ModelDeviceAdmissionV1,
    account: Gfx942RequestAccountV1,
}

impl RuntimeAllocationDeviceAdmissionV1 {
    pub fn for_checked_device_v1(
        backend_device: u64,
        device: &CheckedGfx942XnackMinusDevice,
        account: Gfx942RequestAccountV1,
    ) -> Result<Self, crate::RuntimeResourceCreditErrorV1> {
        if backend_device == 0 || !account.matches_device_v1(device) {
            return Err(crate::RuntimeResourceCreditErrorV1::Invariant);
        }
        Ok(Self {
            backend_device,
            model: device.model_admission(),
            account,
        })
    }

    pub const fn backend_device_v1(&self) -> u64 {
        self.backend_device
    }

    pub(crate) fn account(&self) -> &Gfx942RequestAccountV1 {
        &self.account
    }

    pub(crate) const fn model(&self) -> ModelDeviceAdmissionV1 {
        self.model
    }

    pub(crate) fn is_live(&self) -> bool {
        let usage = self.account.session_usage_v1();
        !usage.poisoned && usage.quarantined_records == 0
    }

    pub(crate) fn matches_checked(
        &self,
        backend_device: u64,
        device: &CheckedGfx942XnackMinusDevice,
    ) -> bool {
        self.backend_device == backend_device
            && self.model == device.model_admission()
            && self.account.matches_device_v1(device)
            && self.is_live()
    }
}

/// Required is a complete roster, not a list of optional overrides.
/// Context rejects missing, extra, duplicate or unhealthy bindings before use.
pub enum RuntimeAllocationAdmissionProfileV1 {
    Legacy,
    Required(Vec<RuntimeAllocationDeviceAdmissionV1>),
}

/// A move-only borrowed request witness. Only Context constructs it; backend
/// implementations may authenticate it but cannot extract or refund its credit.
///
/// ```compile_fail
/// fn duplicate(witness: fe2o3_runtime::RuntimeAllocationRequestWitnessV1<'_>) {
///     let _copy = witness.clone();
/// }
/// ```
pub struct RuntimeAllocationRequestWitnessV1<'a> {
    device: RuntimeDeviceIdV1,
    admission: &'a RuntimeAllocationDeviceAdmissionV1,
    credit: &'a Gfx942RetainedRequestV1,
    bytes: u64,
}

impl<'a> RuntimeAllocationRequestWitnessV1<'a> {
    pub(super) fn new(
        device: RuntimeDeviceIdV1,
        admission: &'a RuntimeAllocationDeviceAdmissionV1,
        credit: &'a Gfx942RetainedRequestV1,
        bytes: u64,
    ) -> Self {
        Self {
            device,
            admission,
            credit,
            bytes,
        }
    }

    /// Inert Context branding, not a request credit or native authority.
    pub const fn device_v1(&self) -> RuntimeDeviceIdV1 {
        self.device
    }

    /// Authenticates against the backend's independently retained binding.
    pub fn matches_v1(&self, expected: &RuntimeAllocationDeviceAdmissionV1, bytes: u64) -> bool {
        self.bytes == bytes
            && self.admission.backend_device == expected.backend_device
            && self.admission.model == expected.model
            && self
                .admission
                .account
                .shares_account_with_v1(&expected.account)
            && expected.is_live()
            && expected
                .account
                .matches_retained_charge_v1(self.credit, bytes)
    }
}

/// Unsupported never dispatches a legacy allocation method. The Outcome arm
/// preserves backend-specific errors and allocation-specific settlement exactly.
pub enum RuntimeRequestAllocationResultV1<E> {
    Unsupported,
    Outcome(Result<RuntimeBackendAllocationOutcomeV1<E>, RuntimeBackendFailureV1<E>>),
}
