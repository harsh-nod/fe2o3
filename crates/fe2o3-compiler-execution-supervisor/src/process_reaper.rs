//! Compatibility names over the single shared protected-service cleanup pool.

pub(crate) use fe2o3_protected_service_spawn::cleanup_bridge::ReapSlotV1;
pub use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupAdmissionErrorV2 as ProtectedIssuerCleanupAdmissionErrorV2,
    ProtectedServiceCleanupErrorV2 as ProtectedIssuerCleanupErrorV2,
    ProtectedServiceCleanupReportV2 as ProtectedIssuerCleanupReportV2,
    ProtectedServiceCleanupReservationV2 as ProtectedIssuerCleanupReservationV2,
    ProtectedServiceCleanupServiceV2 as ProtectedIssuerCleanupServiceV2,
};

pub(crate) fn reserve_legacy() -> Result<ReapSlotV1<'static>, crate::ProtectedIssuerLaunchErrorV1> {
    use fe2o3_protected_service_spawn::cleanup_bridge::{self, LegacyCleanupReservationErrorV1};
    cleanup_bridge::reserve_legacy().map_err(|error| match error {
        LegacyCleanupReservationErrorV1::Mode => {
            crate::ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                "cleanup pool is not in legacy mode",
            )
        }
        LegacyCleanupReservationErrorV1::Capacity => {
            crate::ProtectedIssuerLaunchErrorV1::ProcessCapacity
        }
    })
}

#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) fn isolated_cleanup(
    account: fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1,
) -> ProtectedIssuerCleanupServiceV2 {
    // SAFETY: private rootless controller fixtures never submit real child records.
    unsafe { fe2o3_protected_service_spawn::cleanup_bridge::isolated_cleanup_for_test(account) }
}
