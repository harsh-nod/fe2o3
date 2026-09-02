//! Private custody for optional bounded MIR-to-KIR CFG refinement evidence.

/// Move-only status derived from the exact live production semantic/KIR owner.
///
/// This is intentionally crate-private and grants no publication, artifact, or
/// launch authority. It distinguishes programs outside the bounded language
/// from programs whose complete relation was actually verified.
pub(crate) struct AuthenticatedMirKirCfgRefinementStatusV2 {
    status: fe2o3_lower_mir_kernel::MirKirCfgRefinementStatusV2,
}

impl AuthenticatedMirKirCfgRefinementStatusV2 {
    pub(crate) fn try_derive(
        owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    ) -> Result<Self, fe2o3_lower_mir_kernel::MirKirCfgRefinementErrorV2> {
        let status = fe2o3_lower_mir_kernel::MirKirCfgRefinementStatusV2::from_live_owner(owner)?;
        debug_assert!(!status.grants_authority());
        let custody = Self { status };
        custody.revalidate(owner)?;
        Ok(custody)
    }

    pub(crate) fn revalidate(
        &self,
        owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    ) -> Result<(), fe2o3_lower_mir_kernel::MirKirCfgRefinementErrorV2> {
        self.status.revalidate_against(owner)
    }

    pub(crate) const fn is_verified(&self) -> bool {
        self.status.evidence().is_some()
    }

    pub(crate) const fn status_name(&self) -> &'static str {
        if self.is_verified() {
            "verified"
        } else {
            "not-eligible"
        }
    }

    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }
}
