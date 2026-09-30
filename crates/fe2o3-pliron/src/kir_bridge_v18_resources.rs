//! Versioned bridge admission: opaque upstream logical units plus explicit
//! table-copy payload and fixed Rust headers. Not allocator/RSS measurement.
use super::*;
use fe2o3_kernel_ir::{StorageFieldV1, StorageLayoutKindV1, StorageLayoutV1, StorageVariantV1};
use std::mem::size_of;

pub(crate) const CLEANUP_ATTEMPTS: usize = 4;
type PanicPayload = Box<dyn std::any::Any + Send>;

// This bounds destructor calls, not arbitrary Drop work or elapsed time.
// Exhaustion terminates the worker; it cannot promise a typed return or that
// every Rust/external-resource destructor completed during process reclamation.
pub(crate) fn discard_caught_payload(mut payload: PanicPayload) {
    for _ in 0..CLEANUP_ATTEMPTS {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => return,
            Err(next) => payload = next,
        }
    }
    std::process::abort();
}

pub(super) fn add(a: usize, b: usize) -> Result<usize, ResourceError> {
    a.checked_add(b).ok_or(ResourceError::Arithmetic)
}
pub(super) fn mul(a: usize, b: usize) -> Result<usize, ResourceError> {
    a.checked_mul(b).ok_or(ResourceError::Arithmetic)
}

pub(super) struct Scope<'a, 'work> {
    pub(super) budget: &'a mut Budget<'work>,
    floor: usize,
    active: bool,
}
impl<'a, 'work> Scope<'a, 'work> {
    pub(super) fn enter(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        // No cleanup retry or smaller admission after this atomic refusal.
        budget.charge_work(1 + CLEANUP_ATTEMPTS)?;
        let floor = budget.storage();
        budget.reserve_storage(headers()?)?;
        Ok(Self {
            budget,
            floor,
            active: true,
        })
    }
    pub(super) fn finish(&mut self) -> Result<(), ResourceError> {
        if self.active {
            let release = self
                .budget
                .storage()
                .checked_sub(self.floor)
                .ok_or(ResourceError::Accounting)?;
            self.budget.release_storage(release)?;
            self.active = false;
        }
        Ok(())
    }
}
impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

pub(super) fn headers() -> Result<usize, ResourceError> {
    let slots = [
        (1, size_of::<KirBridgeErrorV18>()),
        // Catch-arm binding, helper parameter, closure capture, successor binding.
        (4, size_of::<PanicPayload>()),
        (1, size_of::<AssertUnwindSafe<PanicPayload>>()),
        (2, size_of::<std::thread::Result<()>>()),
        (1, size_of::<std::ops::Range<usize>>()),
        (1, size_of::<usize>()),
        (1, size_of::<Scope<'_, '_>>()),
        (2, size_of::<Result<Scope<'_, '_>, ResourceError>>()),
        (2, size_of::<KirPlironGraphV18<'_>>()),
        // Facade, inner, closure, catch, stored outcome, match and caller slots.
        (8, size_of::<ImportResult<'_>>()),
        (8, size_of::<ExportResult>()),
        (2, size_of::<Module>()),
        (2, size_of::<Vec<KirBridgeCorrespondenceV1>>()),
        (
            2,
            size_of::<Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<
                std::thread::Result<
                    Result<(Module, Vec<KirBridgeCorrespondenceV1>), KirBridgeErrorV18>,
                >,
            >(),
        ),
        (
            2,
            size_of::<
                Result<
                    (
                        VerifiedCanonicalKernelIrModuleV18,
                        fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV18,
                    ),
                    CanonicalKernelIrReplayAdmissionErrorV18,
                >,
            >(),
        ),
        (
            2,
            size_of::<
                Result<CanonicalStorageTableIdentityV18, CanonicalKernelIrReplayAdmissionErrorV18>,
            >(),
        ),
        (2, size_of::<Result<(), KirBridgeErrorV18>>()),
        (1, size_of::<crate::OperationGraphSnapshotV1>()),
        (
            2,
            size_of::<Result<crate::OperationGraphSnapshotV1, OperationHandleError>>(),
        ),
        (2, size_of::<Result<(), ResourceError>>()),
        (2, size_of::<Result<usize, ResourceError>>()),
        // Coordinate envelope/result and import totals are constructed before
        // that envelope's payload reservation, so this earlier scope owns them.
        (2, size_of::<Envelope>()),
        (2, size_of::<Result<Envelope, ResourceError>>()),
        (2, size_of::<usize>()),
        (
            2,
            size_of::<
                Result<
                    storage_v18::ProfileV18<'_>,
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18,
                >,
            >(),
        ),
        (2, size_of::<std::thread::Result<ImportResult<'_>>>()),
        (2, size_of::<std::thread::Result<ExportResult>>()),
        (2, size_of::<Vec<StorageLayoutV1>>()),
        (2, size_of::<Vec<StorageFieldV1>>()),
        (2, size_of::<Vec<StorageVariantV1>>()),
        (
            2,
            size_of::<Result<Vec<StorageLayoutV1>, KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<Result<Vec<StorageFieldV1>, KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<Result<Vec<StorageVariantV1>, KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<Result<Box<[StorageFieldV1]>, KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<Result<Box<[StorageVariantV1]>, KirBridgeErrorV18>>(),
        ),
        (
            2,
            size_of::<Result<(), std::collections::TryReserveError>>(),
        ),
    ];
    slots
        .into_iter()
        .try_fold(0, |sum, (count, size)| add(sum, mul(count, size)?))
}

pub(super) struct Envelope {
    pub(super) work: usize,
    pub(super) storage: usize,
}
pub(super) fn envelope(bytes: usize, slots: usize) -> Result<Envelope, ResourceError> {
    let volume = add(add(bytes, slots)?, 1)?;
    Ok(Envelope {
        work: add(mul(mul(volume, volume)?, 4)?, mul(volume, 8)?)?,
        storage: add(mul(volume, 64)?, 4096)?,
    })
}

// V18 alone retains exact ordinary-operation and terminator coordinates. The
// already-counted tree contains at least two slots for each such operation.
// Reserve once to avoid growth during construction. Capacity/control bytes are
// a logical upper envelope, not a promise about the allocator's physical RSS.
pub(super) fn coordinate_envelope(slots: usize) -> Result<Envelope, ResourceError> {
    let capacity = mul(slots, 2)?;
    let payload = add(
        mul(
            capacity,
            add(size_of::<(Ptr<Operation>, KirBridgeCoordinateV1)>(), 1)?,
        )?,
        16,
    )?;
    let headers = add(
        size_of::<HashMap<Ptr<Operation>, KirBridgeCoordinateV1>>(),
        add(
            size_of::<Result<(), std::collections::TryReserveError>>(),
            add(
                size_of::<Option<KirBridgeCoordinateV1>>(),
                size_of::<(Ptr<Operation>, KirBridgeCoordinateV1, usize, usize)>(),
            )?,
        )?,
    )?;
    Ok(Envelope {
        // Worst-case hash insertion/comparison debit, not an assertion that
        // HashMap has deterministic constant-time or logarithmic lookup.
        work: add(mul(mul(slots, slots)?, 2)?, mul(slots, 8)?)?,
        storage: add(payload, headers)?,
    })
}

pub(super) fn table_bytes(
    source: &Module,
    budget: &mut Budget<'_>,
) -> Result<usize, ResourceError> {
    let mut bytes = mul(source.storage_layouts.len(), size_of::<StorageLayoutV1>())?;
    for row in &source.storage_layouts {
        budget.charge_work(1)?;
        let dynamic = match &row.kind {
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
                budget.charge_work(fields.len())?;
                mul(fields.len(), size_of::<StorageFieldV1>())?
            }
            StorageLayoutKindV1::Variants { variants, .. } => {
                budget.charge_work(variants.len())?;
                mul(variants.len(), size_of::<StorageVariantV1>())?
            }
            StorageLayoutKindV1::Scalar(_)
            | StorageLayoutKindV1::Vector(_)
            | StorageLayoutKindV1::Pointer(_)
            | StorageLayoutKindV1::Array { .. }
            | StorageLayoutKindV1::Slice { .. } => 0,
        };
        bytes = add(bytes, dynamic)?;
    }
    Ok(bytes)
}

fn exact_vec<T>(length: usize) -> Result<Vec<T>, KirBridgeErrorV18> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| KirBridgeErrorV18::Allocation)?;
    if result.capacity() != length {
        return Err(KirBridgeErrorV18::Allocation);
    }
    Ok(result)
}

fn boxed<T: Clone>(source: &[T]) -> Result<Box<[T]>, KirBridgeErrorV18> {
    let mut result = exact_vec(source.len())?;
    result.extend_from_slice(source);
    Ok(result.into_boxed_slice())
}

pub(super) fn copy_table(source: &Module) -> Result<Vec<StorageLayoutV1>, KirBridgeErrorV18> {
    let mut result = exact_vec(source.storage_layouts.len())?;
    for row in &source.storage_layouts {
        let kind = match &row.kind {
            StorageLayoutKindV1::Record(fields) => StorageLayoutKindV1::Record(boxed(fields)?),
            StorageLayoutKindV1::Union(fields) => StorageLayoutKindV1::Union(boxed(fields)?),
            StorageLayoutKindV1::Variants { encoding, variants } => StorageLayoutKindV1::Variants {
                encoding: *encoding,
                variants: boxed(variants)?,
            },
            other => other.clone(),
        };
        result.push(StorageLayoutV1 {
            size: row.size,
            alignment: row.alignment,
            kind,
        });
    }
    Ok(result)
}
