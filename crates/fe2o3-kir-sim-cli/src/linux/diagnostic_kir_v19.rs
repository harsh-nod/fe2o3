//! Exact diagnostic V19 ingress, shared by the simulator and headless debugger.
//! Canonical accounting and the CPU's post-decode resident check are separate;
//! neither the retained target declarations nor these bytes authenticate source.

use std::mem::size_of;

use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV19, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, CanonicalKernelIrWorkBudgetV1,
    KernelIrDecodeError, KernelIrEncodeError, VerifiedCanonicalKernelIrModuleV19,
};

use super::*;

const MAX_CANONICAL_WORK_V19: usize = 1 << 27;
const MAX_CANONICAL_STORAGE_V19: usize = 256 * 1024 * 1024;

pub(crate) fn load_debug_simulation_input_v19(
    kir: &Path,
    request: &Path,
) -> Result<crate::AdmittedSimulationInputV1, crate::SimulationInputErrorV1> {
    load_admitted_kir_v19(kir, request).map_err(input_error)
}

pub(crate) fn load_debug_simulation_input_bytes_v19(
    kir: &[u8],
    request: &[u8],
) -> Result<crate::AdmittedSimulationInputV1, crate::SimulationInputErrorV1> {
    let result = (|| {
        check_lengths(kir, request)?;
        // Borrowed extents only: allocation capacity belongs to the caller and
        // cannot truthfully be inferred from a slice.
        let payload = kir
            .len()
            .checked_add(request.len())
            .ok_or_else(arithmetic)?;
        load_bytes(kir, request, payload)
    })();
    result.map_err(input_error)
}

pub(super) fn load_admitted_kir_v19(
    kir: &Path,
    request: &Path,
) -> Result<crate::AdmittedSimulationInputV1, Failure> {
    let kir = secure_read(
        kir,
        MAX_KIR_BYTES,
        InputCode::KirV19,
        "diagnostic canonical KIR V19",
    )?;
    let request = secure_read(
        request,
        MAX_REQUEST_BYTES,
        InputCode::Request,
        "simulation request",
    )?;
    // secure_read has its own bounded filesystem/allocator contract. Account
    // actual coexisting capacities before the subsequent canonical traversal.
    let payload = kir
        .capacity()
        .checked_add(request.capacity())
        .and_then(|bytes| bytes.checked_add(2 * size_of::<Vec<u8>>()))
        .ok_or_else(arithmetic)?;
    load_bytes(&kir, &request, payload)
}

fn check_lengths(kir: &[u8], request: &[u8]) -> Result<(), Failure> {
    for (bytes, maximum, code, label) in [
        (
            kir,
            MAX_KIR_BYTES,
            InputCode::KirV19,
            "diagnostic canonical KIR V19",
        ),
        (
            request,
            MAX_REQUEST_BYTES,
            InputCode::Request,
            "simulation request",
        ),
    ] {
        if bytes.len() > maximum {
            return Err(Failure::input(
                code,
                ErrorKind::InputTooLarge,
                format!("{label} exceeds its {maximum}-byte bound"),
            ));
        }
    }
    Ok(())
}

fn load_bytes(
    kir: &[u8],
    request: &[u8],
    input_payload: usize,
) -> Result<crate::AdmittedSimulationInputV1, Failure> {
    check_lengths(kir, request)?;
    let limits = cli_simulation_limits();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MAX_CANONICAL_WORK_V19);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, MAX_CANONICAL_STORAGE_V19);
    let admitted = admit_with_budget(kir, input_payload, limits, &mut budget)?;
    // The temporary canonical owner is now dropped. Strict request parsing and
    // the retained CPU module use their existing separate bounded contracts;
    // there is deliberately no invented combined storage receipt.
    finish_admitted_input(
        admitted,
        limits,
        request,
        None,
        SimulationTargetV1::amdgpu_64(),
        None,
        None,
    )
}

fn admit_with_budget(
    kir: &[u8],
    input_payload: usize,
    limits: SimulationLimitsV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<AdmittedSimulationModuleV1, Failure> {
    let floor = budget.storage();
    let result =
        (|| {
            if input_payload < kir.len() {
                return Err(resource_failure(
                    CanonicalKernelIrVerificationResourceErrorV1::Accounting,
                ));
            }
            budget
                .reserve_storage(input_payload)
                .map_err(resource_failure)?;
            let (canonical, storage) = VerifiedCanonicalKernelIrModuleV19::
            from_canonical_bytes_with_verification_budget_v19(kir, budget)
            .map_err(canonical_failure)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource_failure)?;
            // The canonical owner remains whole and charged during this borrow.
            // admit_v19 bounds decode by wire limits and checks CPU resident storage
            // afterwards; CPU allocations are NOT charged to this canonical ledger.
            let admitted =
                AdmittedSimulationModuleV1::admit_v19(&canonical, limits).map_err(|error| {
                    Failure::new(
                        Stage::SimulatorAdmission,
                        admission_error_kind(&error),
                        bounded_display(&error),
                    )
                })?;
            if admitted.identity().wire_version() != 19
                || admitted.identity().digest() != canonical.identity().digest()
                || admitted.identity().canonical_length() != canonical.identity().canonical_length()
            {
                return Err(kir_failure(
                    ErrorKind::KirV19IdentityMismatch,
                    "diagnostic V19 identity changed during CPU admission",
                ));
            }
            drop(canonical);
            budget
                .release_storage(storage.retained_storage())
                .map_err(resource_failure)?;
            Ok(admitted)
        })();
    // Every temporary owner in the closure is gone before restoring this floor.
    // Accepted work, peak storage and first failed charge remain cumulative.
    let release = budget.storage().checked_sub(floor).ok_or_else(|| {
        resource_failure(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
    })?;
    budget.release_storage(release).map_err(resource_failure)?;
    result
}

fn input_error(failure: Failure) -> crate::SimulationInputErrorV1 {
    crate::SimulationInputErrorV1 {
        stage: serialized_tag(failure.0.stage),
        code: serialized_tag(failure.0.kind),
        message: failure.0.message.clone(),
    }
}

fn kir_failure(kind: ErrorKind, message: impl Into<String>) -> Failure {
    let mut failure = Failure::new(Stage::KirAdmission, kind, message);
    failure.0.input = Some(InputCode::KirV19);
    failure
}

fn arithmetic() -> Failure {
    resource_failure(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

fn resource_failure(error: CanonicalKernelIrVerificationResourceErrorV1) -> Failure {
    use CanonicalKernelIrVerificationResourceErrorV1 as E;
    let kind = match error {
        E::Work(_) => ErrorKind::KirV19WorkLimit,
        E::Storage(_) => ErrorKind::KirV19StorageLimit,
        E::Allocation => ErrorKind::KirV19AllocationFailed,
        E::Accounting => ErrorKind::KirV19ResourceAccounting,
        E::Arithmetic => ErrorKind::KirV19ResourceArithmetic,
    };
    kir_failure(kind, bounded_display(&error))
}

fn canonical_failure(error: CanonicalKernelIrReplayAdmissionErrorV19) -> Failure {
    use CanonicalKernelIrReplayAdmissionErrorV19 as E;
    let kind = match &error {
        E::Resource(error) | E::Decode(KernelIrDecodeError::Resource(error)) => {
            return resource_failure(*error);
        }
        E::Decode(KernelIrDecodeError::WorkLimit(_)) => ErrorKind::KirV19WorkLimit,
        E::Decode(KernelIrDecodeError::Encode(error)) | E::Encode(error) => match error {
            KernelIrEncodeError::WorkLimit(_) => ErrorKind::KirV19WorkLimit,
            KernelIrEncodeError::Allocation => ErrorKind::KirV19AllocationFailed,
            KernelIrEncodeError::Overflow { .. } => ErrorKind::KirV19ResourceArithmetic,
            KernelIrEncodeError::NonCanonical { .. } => ErrorKind::KirV19RoundTripMismatch,
            _ => ErrorKind::KirV19EncodeFailed,
        },
        E::Decode(KernelIrDecodeError::UnknownVersion(_)) => ErrorKind::KirV19WrongVersion,
        E::Decode(KernelIrDecodeError::NonCanonical) | E::CanonicalMismatch => {
            ErrorKind::KirV19RoundTripMismatch
        }
        E::Decode(_) => ErrorKind::KirV19DecodeFailed,
        E::Verification(_) => ErrorKind::KirV19VerificationFailed,
    };
    kir_failure(kind, bounded_display(&error))
}

#[cfg(test)]
#[path = "diagnostic_kir_v19_tests.rs"]
mod tests;
