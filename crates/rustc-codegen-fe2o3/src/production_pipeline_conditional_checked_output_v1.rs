//! Private target/source phase join; no source, proof or graph reconstructed.
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalCheckedOutputErrorV1,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Debug)]
pub(super) enum Error {
    Resource(Resource),
    Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1),
    Agreement(ProductionConditionalCheckedOutputErrorV1),
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional policy6 target/source join: {self:?}")
    }
}

pub(super) fn check(
    request: &Request<'_>,
    bound: &Graph,
    checked: &Prefix,
    profile: Profile,
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<(), Error> {
    let floor = target.storage();
    let account = target.work_ledger_identity_v1();
    let address = std::ptr::from_mut(target);
    let result = catch_unwind(AssertUnwindSafe(|| {
        target.reserve_storage(size_of::<Error>() + 256)?;
        let (coordinates, storage) =
            dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                request.source().executable(),
                bound,
                profile,
                target,
            )
            .map_err(Error::Target)?;
        target.reserve_storage(storage.retained_storage())?;
        request
            .check_policy6_output_v1(&coordinates, checked, target, source)
            .map_err(Error::Agreement)
    }));
    let cleanup =
        if std::ptr::from_mut(target) != address || target.work_ledger_identity_v1() != account {
            Err(Resource::Accounting)
        } else {
            target
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
                .and_then(|delta| target.release_storage(delta))
        };
    match result {
        Ok(result) => {
            cleanup?;
            #[cfg(test)]
            if result.is_ok() {
                tests::agreement_checked(profile);
            }
            result
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
#[path = "production_pipeline_conditional_checked_output_v1_tests.rs"]
pub(super) mod tests;
