use super::resource::{Outcome, Scope};
use super::*;
use crate::{check_module_storage_v1, verify_storage_module_ref_with_budget_v1};
use sha2::{Digest, Sha256};

#[cfg(test)]
std::thread_local! { static PANIC_STAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
pub(super) fn set_panic_stage(stage: u8) {
    PANIC_STAGE.with(|value| value.set(stage));
}

#[cfg(test)]
fn checkpoint(stage: u8) {
    PANIC_STAGE.with(|value| {
        if value.get() == stage {
            value.set(0);
            panic!("V18 canonical custody checkpoint {stage}");
        }
    });
}

fn verify(
    module: &Module,
    limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<(), CanonicalKernelIrReplayAdmissionErrorV18> {
    let checked = check_module_storage_v1(module, limits, budget)
        .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Layout)?;
    let _verified = verify_storage_module_ref_with_budget_v1(checked, None, budget)
        .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Verification)?;
    Ok(())
}

fn identity(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<VerifiedCanonicalKernelIrIdentityV18, ResourceError> {
    let domain = VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_DOMAIN_V1;
    let canonical_length = u64::try_from(bytes.len()).map_err(|_| ResourceError::Arithmetic)?;
    let domain_length = u32::try_from(domain.len()).map_err(|_| ResourceError::Arithmetic)?;
    let work = bytes
        .len()
        .checked_add(domain.len())
        .and_then(|n| n.checked_add(4 + 2 + 8))
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(work)?;
    let mut hash = Sha256::new();
    hash.update(domain_length.to_le_bytes());
    hash.update(domain);
    hash.update(VERIFIED_CANONICAL_KERNEL_IR_V18_IDENTITY_POLICY_V1.to_le_bytes());
    hash.update(canonical_length.to_le_bytes());
    hash.update(bytes);
    Ok(VerifiedCanonicalKernelIrIdentityV18 {
        digest: hash.finalize().into(),
        canonical_length,
    })
}

fn finish_owner(module: Module, canonical_bytes: Vec<u8>, scope: &mut Scope<'_, '_>) -> Outcome {
    #[cfg(test)]
    checkpoint(4);
    let identity = identity(&canonical_bytes, scope.budget)?;
    let retained = scope.retained()?;
    Ok((
        VerifiedCanonicalKernelIrModuleV18 {
            module,
            canonical_bytes,
            identity,
        },
        CanonicalKernelIrReplayStorageV18 { retained },
    ))
}

fn transfer(scope: &mut Scope<'_, '_>, result: Outcome) -> Outcome {
    let released = scope.finish();
    match result {
        Err(error) => Err(error),
        Ok(owner) => {
            // Successful custody intentionally transfers together with its paid
            // receipt; on release failure the complete owner is dropped here.
            released?;
            Ok(owner)
        }
    }
}

pub(super) fn from_module(
    module: &Module,
    limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Outcome {
    let mut scope = Scope::enter(budget)?;
    let result = (|| {
        scope
            .budget
            .reserve_storage(std::mem::size_of::<VerifiedCanonicalKernelIrModuleV18>())?;
        let extent = crate::wire::count_module_with_work_v1(
            module,
            crate::KERNEL_IR_VERSION_V18,
            scope.budget.work_budget_v1(),
            false,
        )
        .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode)?;
        scope.budget.reserve_storage(extent.wire_bytes())?;
        scope
            .budget
            .reserve_storage(extent.peak_auxiliary_bytes())?;
        let bytes = crate::wire::encode_counted_module_v18(
            module,
            extent.wire_bytes(),
            scope.budget.work_budget_v1(),
        )
        .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Encode)?;
        scope
            .budget
            .release_storage(extent.peak_auxiliary_bytes())?;
        #[cfg(test)]
        checkpoint(1);
        let decoded =
            crate::wire::decode_module_v18_with_allocation_budget_v1(&bytes, scope.budget)
                .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Decode)?;
        #[cfg(test)]
        checkpoint(2);
        verify(&decoded, limits, scope.budget)?;
        #[cfg(test)]
        checkpoint(3);
        // The shared decoder compared the full explicit-role/table encoding.
        // Every field is lossless; no unmetered derived graph equality is used.
        finish_owner(decoded, bytes, &mut scope)
    })();
    transfer(&mut scope, result)
}

pub(super) fn from_bytes(
    bytes: &[u8],
    limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Outcome {
    let mut scope = Scope::enter(budget)?;
    let result = (|| {
        scope
            .budget
            .reserve_storage(std::mem::size_of::<VerifiedCanonicalKernelIrModuleV18>())?;
        let module = crate::wire::decode_module_v18_with_allocation_budget_v1(bytes, scope.budget)
            .map_err(CanonicalKernelIrReplayAdmissionErrorV18::Decode)?;
        #[cfg(test)]
        checkpoint(2);
        verify(&module, limits, scope.budget)?;
        #[cfg(test)]
        checkpoint(3);
        scope.budget.reserve_storage(bytes.len())?;
        scope.budget.charge_work(bytes.len())?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| ResourceError::Allocation)?;
        if owned.capacity() != bytes.len() {
            return Err(ResourceError::Allocation.into());
        }
        owned.extend_from_slice(bytes);
        #[cfg(test)]
        checkpoint(1);
        finish_owner(module, owned, &mut scope)
    })();
    transfer(&mut scope, result)
}
