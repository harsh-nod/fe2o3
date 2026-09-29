//! Fixed retained expiry bridge; the old Schedule and Visitor traversal remain.
use super::super::retained_alias_state::RetainedAliasSessionV1;
use super::*;

impl Schedule {
    pub(in super::super) fn completed_retained(
        &self,
        analysis: &mut RetainedAliasSessionV1<'_, '_, '_>,
        site: SemanticTransparentBorrowSiteV1,
        source: &SemanticStatementKindV1,
    ) -> Result<()> {
        let result = (|| {
            analysis.expiry_work(1)?;
            if self.indegrees.get(site.block as usize) != Some(&0) {
                return Ok(());
            }
            statement(
                &mut RetainedExpire {
                    schedule: self,
                    analysis,
                    site,
                },
                source,
            )
        })();
        analysis.expiry_result(result)
    }
}
struct RetainedExpire<'s, 'r, 'f, 'w> {
    schedule: &'s Schedule,
    analysis: &'s mut RetainedAliasSessionV1<'r, 'f, 'w>,
    site: SemanticTransparentBorrowSiteV1,
}
impl Visitor for RetainedExpire<'_, '_, '_, '_> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.analysis.expiry_work(units)
    }
    fn local(&mut self, local: SemanticLocalIdV1) -> Result<()> {
        self.analysis.expiry_work(4)?;
        let row = self
            .schedule
            .locals
            .get(local.index() as usize)
            .ok_or(Error::ReplayMismatch)?;
        if row.pinned
            || row.block != self.site.block as usize
            || row.statement != self.site.statement as usize
        {
            return Ok(());
        }
        self.analysis.expiry_drop_local(local.index())
    }
}
pub(in super::super) fn frame()
-> std::result::Result<usize, fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use std::mem::size_of;
    let rows = [
        size_of::<RetainedExpire<'static, 'static, 'static, 'static>>(),
        size_of::<(
            &Schedule,
            &mut RetainedAliasSessionV1<'static, 'static, 'static>,
            SemanticTransparentBorrowSiteV1,
            &SemanticStatementKindV1,
            SemanticLocalIdV1,
            usize,
            u32,
            Option<&Last>,
            &Last,
            Result<&Last>,
            Error,
        )>(),
        size_of::<(Result<()>, Option<&usize>, &usize, usize, usize, bool)>(),
        size_of::<(
            &mut RetainedExpire<'static, 'static, 'static, 'static>,
            &SemanticStatementKindV1,
            &SemanticPlaceV1,
            SemanticProjectionKindV1,
            std::slice::Iter<'static, SemanticProjectionV1>,
            &SemanticProjectionV1,
            &[SemanticProjectionV1],
            &SemanticRvalueKindV1,
            &SemanticOperandV1,
            &mut dyn FnMut(&SemanticOperandV1) -> Result<()>,
        )>(),
        size_of::<(
            [usize; 5],
            std::array::IntoIter<usize, 5>,
            usize,
            std::result::Result<usize, Resource>,
            Resource,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
