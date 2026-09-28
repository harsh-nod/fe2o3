use super::*;
use sha2::Sha256;

pub(super) type Outcome = Result<
    (
        VerifiedCanonicalKernelIrModuleV18,
        CanonicalKernelIrReplayStorageV18,
    ),
    CanonicalKernelIrReplayAdmissionErrorV18,
>;
pub(super) type CopyOutcome = Result<
    (Module, CanonicalKernelIrCandidateStorageV18),
    CanonicalKernelIrReplayAdmissionErrorV18,
>;

pub(super) struct Scope<'a, 'work> {
    pub(super) budget: &'a mut Budget<'work>,
    floor: Option<usize>,
    headers: usize,
}
impl<'a, 'work> Scope<'a, 'work> {
    pub(super) fn enter(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        budget.charge_work(1)?;
        let floor = budget.storage_checkpoint();
        let headers = headers()?;
        budget.reserve_storage(headers)?;
        Ok(Self {
            budget,
            floor: Some(floor),
            headers,
        })
    }
    pub(super) fn enter_copy(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        budget.charge_work(1)?;
        let floor = budget.storage_checkpoint();
        let headers = copy_headers()?;
        budget.reserve_storage(headers)?;
        Ok(Self {
            budget,
            floor: Some(floor),
            headers,
        })
    }
    pub(super) fn retained(&self) -> Result<usize, ResourceError> {
        self.budget
            .storage()
            .checked_sub(self.floor.ok_or(ResourceError::Accounting)?)
            .and_then(|n| n.checked_sub(self.headers))
            .ok_or(ResourceError::Accounting)
    }
    pub(super) fn finish(&mut self) -> Result<(), ResourceError> {
        match self.floor.take() {
            Some(floor) => self.budget.rollback_storage(floor),
            None => Ok(()),
        }
    }
}
impl Drop for Scope<'_, '_> {
    fn drop(&mut self) {
        // All closure-local temporary owners unwind before this scope.
        let _ = self.finish();
    }
}

pub(super) fn copy_headers() -> Result<usize, ResourceError> {
    let slots = [
        (1, std::mem::size_of::<Scope<'_, '_>>()),
        (
            2,
            std::mem::size_of::<Result<Scope<'_, '_>, ResourceError>>(),
        ),
        // Closure, stored result, method return and caller outcome.
        (4, std::mem::size_of::<CopyOutcome>()),
        (
            2,
            std::mem::size_of::<Result<Module, KernelIrDecodeError>>(),
        ),
        (2, std::mem::size_of::<Result<usize, ResourceError>>()),
        (2, std::mem::size_of::<Result<(), ResourceError>>()),
    ];
    slots.into_iter().try_fold(
        crate::wire::storage_codec_headers_v18()?,
        |sum, (count, size)| {
            size.checked_mul(count)
                .and_then(|n| sum.checked_add(n))
                .ok_or(ResourceError::Arithmetic)
        },
    )
}

pub(super) fn headers() -> Result<usize, ResourceError> {
    let slots = [
        (1, std::mem::size_of::<Scope<'_, '_>>()),
        (
            2,
            std::mem::size_of::<Result<Scope<'_, '_>, ResourceError>>(),
        ),
        (8, std::mem::size_of::<Outcome>()),
        (
            1,
            std::mem::size_of::<crate::StructurallyCheckedModuleStorageV1<'_>>(),
        ),
        (
            1,
            std::mem::size_of::<VerifiedStorageKernelIrModuleV1<'_>>(),
        ),
        (
            2,
            std::mem::size_of::<
                Result<crate::StructurallyCheckedModuleStorageV1<'_>, StorageLayoutErrorV1>,
            >(),
        ),
        (
            2,
            std::mem::size_of::<
                Result<VerifiedStorageKernelIrModuleV1<'_>, BorrowedKernelIrVerificationErrorV1>,
            >(),
        ),
        (2, std::mem::size_of::<Result<usize, ResourceError>>()),
        (
            2,
            std::mem::size_of::<Result<Module, KernelIrDecodeError>>(),
        ),
        (
            2,
            std::mem::size_of::<Result<Vec<u8>, KernelIrEncodeError>>(),
        ),
        (
            2,
            std::mem::size_of::<Result<(), CanonicalKernelIrReplayAdmissionErrorV18>>(),
        ),
        (2, std::mem::size_of::<Result<(), ResourceError>>()),
        (
            2,
            std::mem::size_of::<Result<VerifiedCanonicalKernelIrIdentityV18, ResourceError>>(),
        ),
        (2, std::mem::size_of::<Sha256>()),
        (2, 32),
    ];
    slots.into_iter().try_fold(
        crate::wire::storage_codec_headers_v18()?,
        |sum, (count, size)| {
            size.checked_mul(count)
                .and_then(|n| sum.checked_add(n))
                .ok_or(ResourceError::Arithmetic)
        },
    )
}
