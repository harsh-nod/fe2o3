//! Exact diagnostic V18 CPU ingress. Retained layout metadata is not execution authority.
use std::mem::size_of;

use fe2o3_kernel_ir::{
    BorrowedKernelIrVerificationErrorV1, CanonicalKernelIrReplayAdmissionErrorV18 as Canonical,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1,
    KernelIrDecodeError, KernelIrEncodeError, StorageLayoutErrorV1, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_kir_sim::{SimulationViewAdmissionErrorV18, SimulationViewStorageV18};

use super::*;

pub(super) const MAX_WORK: usize = 1 << 27;
pub(super) const MAX_STORAGE: usize = 256 * 1024 * 1024;
pub(super) const LAYOUT_LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 4096,
    edges: 32768,
    containment_depth: 64,
    object_bytes: MAX_STORAGE as u64,
};

struct ScopedFailure(Failure);
impl From<Resource> for ScopedFailure {
    fn from(error: Resource) -> Self {
        Self(resource_failure(error))
    }
}
impl From<Failure> for ScopedFailure {
    fn from(error: Failure) -> Self {
        Self(error)
    }
}

pub(crate) fn load_debug_simulation_input_v18(
    kir: &Path,
    request: &Path,
    budget: &mut Budget<'_>,
) -> Result<
    (crate::AdmittedSimulationInputV1, SimulationViewStorageV18),
    crate::SimulationInputErrorV1,
> {
    load_admitted_kir_v18(kir, request, budget).map_err(input_error)
}

pub(crate) fn load_debug_simulation_input_bytes_v18(
    kir: &[u8],
    request: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    (crate::AdmittedSimulationInputV1, SimulationViewStorageV18),
    crate::SimulationInputErrorV1,
> {
    let payload = kir
        .len()
        .checked_add(request.len())
        .ok_or_else(|| resource_failure(Resource::Arithmetic));
    payload
        .and_then(|payload| load_bytes(kir, request, payload, budget))
        .map_err(input_error)
}

fn load_admitted_kir_v18(
    kir: &Path,
    request: &Path,
    budget: &mut Budget<'_>,
) -> Result<(crate::AdmittedSimulationInputV1, SimulationViewStorageV18), Failure> {
    // File capture has its existing separate bounded allocator/IO contract.
    let kir = secure_read(
        kir,
        MAX_KIR_BYTES,
        InputCode::KirV18,
        "diagnostic canonical KIR V18",
    )?;
    let request = secure_read(
        request,
        MAX_REQUEST_BYTES,
        InputCode::Request,
        "simulation request",
    )?;
    let payload = kir
        .capacity()
        .checked_add(request.capacity())
        .and_then(|bytes| bytes.checked_add(2 * size_of::<Vec<u8>>()))
        .ok_or_else(|| resource_failure(Resource::Arithmetic))?;
    load_bytes(&kir, &request, payload, budget)
}

fn load_bytes(
    kir: &[u8],
    request: &[u8],
    input_payload: usize,
    budget: &mut Budget<'_>,
) -> Result<(crate::AdmittedSimulationInputV1, SimulationViewStorageV18), Failure> {
    let floor = budget.storage();
    budget
        .with_prepaid_scope(floor, 1, 1, 0, |budget| {
            for (bytes, maximum, code) in [
                (kir, MAX_KIR_BYTES, InputCode::KirV18),
                (request, MAX_REQUEST_BYTES, InputCode::Request),
            ] {
                if bytes.len() > maximum {
                    return Err(ScopedFailure(Failure::input(
                        code,
                        ErrorKind::InputTooLarge,
                        format!("diagnostic V18 input exceeds its {maximum}-byte bound"),
                    )));
                }
            }
            if input_payload
                < kir
                    .len()
                    .checked_add(request.len())
                    .ok_or(Resource::Arithmetic)?
            {
                return Err(Resource::Accounting.into());
            }
            budget.reserve_storage(input_payload)?;
            let (canonical, owner_storage) =
                Owner::from_canonical_bytes_with_verification_budget_v18(
                    kir,
                    LAYOUT_LIMITS,
                    budget,
                )
                .map_err(|error| ScopedFailure(canonical_failure(error)))?;
            budget.reserve_storage(owner_storage.retained_storage())?;
            let limits = cli_simulation_limits();
            let (module, storage) = AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
                &canonical, limits, budget,
            )
            .map_err(|error| ScopedFailure(simulation_failure(error)))?;
            budget.reserve_storage(storage.retained_storage())?;
            if module.identity().wire_version() != 18
                || module.identity().digest() != canonical.identity().digest()
                || module.identity().canonical_length() != canonical.identity().canonical_length()
            {
                return Err(ScopedFailure(kir_failure(
                    ErrorKind::KirV18IdentityMismatch,
                    "diagnostic V18 identity changed during CPU admission",
                )));
            }
            // Request parsing and execution retain their existing separate bounds.
            // The receipt covers the CPU view, not a combined request/RSS claim.
            let input = finish_admitted_input(
                module,
                limits,
                request,
                None,
                SimulationTargetV1::amdgpu_64(),
                None,
                None,
            )
            .map_err(ScopedFailure)?;
            drop(canonical);
            budget.release_storage(owner_storage.retained_storage())?;
            Ok((input, storage))
        })
        .map_err(|error: ScopedFailure| error.0)
}

pub(super) fn run(kir: &Path, request: &Path, policy: RunPolicy) -> Result<(), Failure> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    run_with_budget(kir, request, policy, &mut budget)
}

fn run_with_budget(
    kir: &Path,
    request: &Path,
    policy: RunPolicy,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    let floor = budget.storage();
    budget
        .with_prepaid_scope(floor, 0, 0, 0, |budget| {
            let (input, storage) =
                load_admitted_kir_v18(kir, request, budget).map_err(ScopedFailure)?;
            budget.reserve_storage(storage.retained_storage())?;
            // The consumed input, output and any execution state drop before the
            // original ledger's scope restores storage, including error and unwind.
            run_with_admitted_input(input, policy).map_err(ScopedFailure)
        })
        .map_err(|error: ScopedFailure| error.0)
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
    failure.0.input = Some(InputCode::KirV18);
    failure
}
pub(super) fn resource_failure(error: Resource) -> Failure {
    let kind = match error {
        Resource::Work(_) => ErrorKind::KirV18WorkLimit,
        Resource::Storage(_) => ErrorKind::KirV18StorageLimit,
        Resource::Allocation => ErrorKind::KirV18AllocationFailed,
        Resource::Accounting => ErrorKind::KirV18ResourceAccounting,
        Resource::Arithmetic => ErrorKind::KirV18ResourceArithmetic,
    };
    kir_failure(
        kind,
        format!("diagnostic V18 admission: {}", bounded_display(&error)),
    )
}
fn simulation_failure(error: SimulationViewAdmissionErrorV18) -> Failure {
    match error {
        SimulationViewAdmissionErrorV18::Resource(error) => resource_failure(error),
        SimulationViewAdmissionErrorV18::CanonicalView(error) => canonical_failure(error),
        SimulationViewAdmissionErrorV18::Admission(error) => Failure::new(
            Stage::SimulatorAdmission,
            admission_error_kind(&error),
            bounded_display(&error),
        ),
    }
}
pub(super) fn canonical_failure(error: Canonical) -> Failure {
    use KernelIrDecodeError as D;
    use KernelIrEncodeError as E;
    let kind = match &error {
        Canonical::Resource(error)
        | Canonical::Decode(D::Resource(error))
        | Canonical::Layout(StorageLayoutErrorV1::Resource(error))
        | Canonical::Verification(BorrowedKernelIrVerificationErrorV1::Resource(error)) => {
            return resource_failure(*error);
        }
        Canonical::Decode(D::WorkLimit(error))
        | Canonical::Encode(E::WorkLimit(error))
        | Canonical::Decode(D::Encode(E::WorkLimit(error))) => {
            return resource_failure(Resource::Work(*error));
        }
        Canonical::Encode(E::Allocation) | Canonical::Decode(D::Encode(E::Allocation)) => {
            return resource_failure(Resource::Allocation);
        }
        Canonical::Encode(E::Overflow { .. })
        | Canonical::Decode(D::Encode(E::Overflow { .. })) => {
            return resource_failure(Resource::Arithmetic);
        }
        Canonical::Decode(D::UnknownVersion(_)) => ErrorKind::KirV18WrongVersion,
        Canonical::Decode(D::NonCanonical)
        | Canonical::Encode(E::NonCanonical { .. })
        | Canonical::Decode(D::Encode(E::NonCanonical { .. })) => {
            ErrorKind::KirV18RoundTripMismatch
        }
        Canonical::Decode(D::Encode(_)) | Canonical::Encode(_) => ErrorKind::KirV18EncodeFailed,
        Canonical::Decode(_) => ErrorKind::KirV18DecodeFailed,
        Canonical::Layout(_) | Canonical::Verification(_) => ErrorKind::KirV18VerificationFailed,
    };
    kir_failure(kind, bounded_display(&error))
}

#[cfg(test)]
#[path = "diagnostic_kir_v18_tests.rs"]
mod tests;
