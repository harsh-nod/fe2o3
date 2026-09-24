//! Private exact-profile reuse of the complete budgeted inverse path.
//! No owner or public caller can select an arbitrary wire version here.

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    KernelIrDecodeError, KernelIrEncodeError, MeteredKernelIrVerificationErrorV1, Module,
    VerificationErrors, verify_exact_decoded_module_with_budget_v1,
};

#[derive(Clone, Copy)]
pub(crate) enum Profile {
    V12,
    V16,
    V17,
    V19,
}

pub(crate) enum InverseError {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    CanonicalMismatch,
}

impl From<CanonicalKernelIrVerificationResourceErrorV1> for InverseError {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

pub(crate) struct VerifiedInverse {
    pub(crate) canonical_bytes: Vec<u8>,
    pub(crate) module: Module,
    pub(crate) storage_floor: usize,
}

pub(crate) fn verified_inverse(
    module: &Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    profile: Profile,
    canonical_owner_header: usize,
    inverse_inline_payload: usize,
) -> Result<VerifiedInverse, InverseError> {
    let version = match profile {
        Profile::V12 => crate::KERNEL_IR_VERSION_V12,
        Profile::V16 => crate::KERNEL_IR_VERSION_V16,
        Profile::V17 => crate::KERNEL_IR_VERSION_V17,
        Profile::V19 => crate::KERNEL_IR_VERSION_V19,
    };
    let extent =
        crate::wire::count_module_with_work_v1(module, version, budget.work_budget_v1(), false)
            .map_err(InverseError::Encode)?;
    let retained = extent
        .wire_bytes()
        .checked_add(canonical_owner_header)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.reserve_storage(retained)?;
    let encoder_scratch =
        crate::wire::decoded_tree_payload_bound_v12::<&crate::FunctionId>(module.kernels.len())
            .map_err(InverseError::Decode)?
            .checked_add(extent.peak_auxiliary_bytes())
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.reserve_storage(encoder_scratch)?;
    let encoded = crate::wire::encode_module_with_work_v1(module, version, budget.work_budget_v1())
        .map_err(InverseError::Encode)?;
    budget.release_storage(encoder_scratch)?;
    if encoded.len() != extent.wire_bytes() || encoded.capacity() != encoded.len() {
        return Err(InverseError::CanonicalMismatch);
    }
    let inverse_floor = budget.storage_checkpoint();
    budget.reserve_storage(inverse_inline_payload)?;
    let decoded = match profile {
        Profile::V12 => crate::wire::decode_module_v12_with_allocation_budget_v1(&encoded, budget),
        Profile::V16 => crate::wire::decode_module_v16_with_allocation_budget_v1(&encoded, budget),
        Profile::V17 => crate::wire::decode_module_v17_with_allocation_budget_v1(&encoded, budget),
        Profile::V19 => crate::wire::decode_module_v19_with_allocation_budget_v1(&encoded, budget),
    }
    .map_err(InverseError::Decode)?;
    verify_exact_decoded_module_with_budget_v1(&decoded, None, budget).map_err(
        |error| match error {
            MeteredKernelIrVerificationErrorV1::Verification(error) => {
                InverseError::Verification(error)
            }
            MeteredKernelIrVerificationErrorV1::Resource(error) => InverseError::Resource(error),
        },
    )?;
    budget.charge_work(encoded.len())?;
    if &decoded != module {
        return Err(InverseError::CanonicalMismatch);
    }
    Ok(VerifiedInverse {
        canonical_bytes: encoded,
        module: decoded,
        storage_floor: inverse_floor,
    })
}
