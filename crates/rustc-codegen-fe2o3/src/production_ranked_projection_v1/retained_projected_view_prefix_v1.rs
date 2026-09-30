//! Retained projected-view prefix; the later writer/query/finish API is not exposed.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::{
    PreparationCustodySnapshotV1 as Snapshot, PreparationResourcesV1 as Prep, resource,
    retained_custody_snapshot_frame_v1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1 as Target;
use std::mem::size_of;

type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Prepared,
}
/// Only DATA/source/custody is retained. A real checked factory still owns
/// source/SSA membership; these immutable source references cannot grant it.
pub(in crate::production_ranked_projection_v1) struct RetainedProjectedViewPrefixV1<'s> {
    phase: Phase,
    source: Option<(&'s SemanticFunctionDeclV1, &'s [SemanticTypeDeclV1], Target)>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    failure: Option<Resource>,
    locals: Vec<Option<ProjectedViewV1>>,
    queried: Vec<QueriedSliceV1>,
    site: Option<ProjectedSemanticAccessSiteV1>,
    source_start: usize,
    guarded_start: usize,
    next_access: u32,
    loan_started: bool,
    loan_returned: bool,
}
impl<'s> RetainedProjectedViewPrefixV1<'s> {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            entry: None,
            held: None,
            failure: None,
            locals: Vec::new(),
            queried: Vec::new(),
            site: None,
            source_start: 0,
            guarded_start: 0,
            next_access: 0,
            loan_started: false,
            loan_returned: false,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        function: &'s SemanticFunctionDeclV1,
        types: &'s [SemanticTypeDeclV1],
        target: Target,
        resources: &mut Prep<'_, '_>,
    ) -> Result<()> {
        if self.phase != Phase::Fresh {
            return Err(self.saved());
        }
        self.phase = Phase::Terminal;
        self.source = Some((function, types, target));
        self.entry = resources.retained_custody_snapshot_v1();
        let result = (|| {
            let entry = self.entry.ok_or_else(|| resource(Resource::Accounting))?;
            if entry.denied_work || entry.denied_storage || entry.owned > entry.storage {
                return Err(resource(Resource::Accounting));
            }
            resources.work(32)?;
            resources.reserve_storage(frame()?)?;
            let count = function.locals().len();
            // The physical destination is attached before admission/allocation.
            // These policy work charges are additive to the unchanged original constructor.
            resources.work(count)?;
            resources.reserve(&mut self.locals, count)?;
            self.locals.resize(count, None);
            self.check(function, types, target, resources)?;
            self.held = resources.retained_custody_snapshot_v1();
            self.phase = Phase::Prepared;
            Ok(())
        })();
        if let Err(Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))) = &result
        {
            self.failure.get_or_insert(*error);
        }
        result
    }
    fn saved(&self) -> Error {
        resource(self.failure.unwrap_or(Resource::Accounting))
    }
    pub(in crate::production_ranked_projection_v1) fn check(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        target: Target,
        resources: &Prep<'_, '_>,
    ) -> Result<()> {
        if let Some(error) = self.failure {
            return Err(resource(error));
        }
        let (bound, bound_types, bound_target) =
            self.source.ok_or_else(|| resource(Resource::Accounting))?;
        if !std::ptr::eq(bound, function)
            || !std::ptr::eq(bound_types, types)
            || bound_target != target
        {
            return Err(resource(Resource::Accounting));
        }
        let entry = self.entry.ok_or_else(|| resource(Resource::Accounting))?;
        let now = resources
            .retained_custody_snapshot_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        let growth = now
            .owned
            .checked_sub(entry.owned)
            .ok_or_else(|| resource(Resource::Accounting))?;
        let expected = entry
            .storage
            .checked_add(growth)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if now.budget_slot != entry.budget_slot
            || now.work_ledger != entry.work_ledger
            || now.owned_slot != entry.owned_slot
            || now.storage != expected
            || now.work < entry.work
            || now.peak < entry.peak
            || now.denied_work
            || now.denied_storage
        {
            return Err(resource(Resource::Accounting));
        }
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(resource(Resource::Accounting));
            }
        }
        Ok(())
    }
    /// The caller checks the original Prep pair immediately before taking this
    /// lexical loan. No mutable Budget or replacement facts implementation escapes.
    pub(in crate::production_ranked_projection_v1) fn loan<'a, F: ProjectedAssertionFactsV1>(
        &'a mut self,
        singletons: &'a [u8],
        shared: &'a fe2o3_pliron::ProductionSemanticSharedReadsV1<'s>,
        borrows: Option<&'a scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'s>>,
        target: Target,
        facts: &'a mut F,
    ) -> Result<ProjectedViewPrefixLoanV1<'a, 's, F>> {
        if self.phase != Phase::Prepared || self.loan_started {
            return Err(self.saved());
        }
        self.loan_started = true;
        self.phase = Phase::Terminal;
        let held = self.held.ok_or_else(|| resource(Resource::Accounting))?;
        let (_, _, expected_target) = self.source.ok_or_else(|| resource(Resource::Accounting))?;
        let identity = facts.helper_value_ledger_v1()?;
        let storage = facts.scalar_private_storage_v1()?;
        if identity != (held.budget_slot, held.work_ledger)
            || storage < held.storage
            || target != expected_target
        {
            self.phase = Phase::Terminal;
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Prepared;
        Ok(ProjectedViewPrefixLoanV1 {
            backing: self,
            scalar_private_singletons: &[],
            shared_value_reads: None,
            scalar_private_borrows: None,
            facts: Some(facts),
        }
        .with_scalar_private_singletons(singletons)
        .with_shared_value_reads(Some(shared))
        .with_scalar_private_borrows(borrows, target))
    }
    /// No writer has run at this boundary: these exact constructor values are
    /// checked rather than replacing a completed query/vector with an empty one.
    pub(in crate::production_ranked_projection_v1) fn before_writers(
        &self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        target: Target,
        resources: &mut Prep<'_, '_>,
    ) -> Result<()> {
        self.check(function, types, target, resources)?;
        resources.work(
            function
                .locals()
                .len()
                .checked_add(8)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        if self.phase != Phase::Prepared
            || !self.loan_started
            || !self.loan_returned
            || self.locals.len() != function.locals().len()
            || self.locals.iter().any(Option::is_some)
            || !self.queried.is_empty()
            || self.site.is_some()
            || self.source_start != 0
            || self.guarded_start != 0
            || self.next_access != 0
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}
/// Narrow pre-writer operations only. There is intentionally no get_mut,
/// begin_site, query, writer or consuming finish method here.
pub(in crate::production_ranked_projection_v1) struct ProjectedViewPrefixLoanV1<
    'a,
    's,
    F: ProjectedAssertionFactsV1 + ?Sized,
> {
    backing: &'a mut RetainedProjectedViewPrefixV1<'s>,
    scalar_private_singletons: &'a [u8],
    shared_value_reads: Option<&'a fe2o3_pliron::ProductionSemanticSharedReadsV1<'s>>,
    scalar_private_borrows: Option<(
        &'a scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'s>,
        Target,
    )>,
    facts: Option<&'a mut F>,
}
impl<'a, 's, F: ProjectedAssertionFactsV1 + ?Sized> ProjectedViewPrefixLoanV1<'a, 's, F> {
    pub(in crate::production_ranked_projection_v1) fn with_scalar_private_singletons(
        mut self,
        census: &'a [u8],
    ) -> Self {
        self.scalar_private_singletons = census;
        self
    }
    pub(in crate::production_ranked_projection_v1) fn with_shared_value_reads(
        mut self,
        reads: Option<&'a fe2o3_pliron::ProductionSemanticSharedReadsV1<'s>>,
    ) -> Self {
        self.shared_value_reads = reads;
        self
    }
    pub(in crate::production_ranked_projection_v1) fn with_scalar_private_borrows(
        mut self,
        census: Option<&'a scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'s>>,
        target: Target,
    ) -> Self {
        self.scalar_private_borrows = census.map(|census| (census, target));
        self
    }
    pub(in crate::production_ranked_projection_v1) fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<()> {
        if self.backing.phase != Phase::Prepared {
            return Err(self.backing.saved());
        }
        self.backing.phase = Phase::Terminal;
        let result = self
            .facts
            .as_deref_mut()
            .ok_or(Error::Incomplete(
                "private array projection requires canonical facts",
            ))?
            .charge_private_array_work(amount);
        self.retain_outcome(result)
    }
    pub(in crate::production_ranked_projection_v1) fn with_assertion_facts_v1(
        &mut self,
        action: impl for<'f> FnOnce(&'f mut F) -> Result<()>,
    ) -> Result<()> {
        if self.backing.phase != Phase::Prepared {
            return Err(self.backing.saved());
        }
        self.backing.phase = Phase::Terminal;
        let facts = self.facts.as_deref_mut().ok_or(Error::Incomplete(
            "multi-entry induction requires live canonical facts",
        ))?;
        let result = action(facts);
        self.retain_outcome(result)
    }
    fn retain_outcome(&mut self, result: Result<()>) -> Result<()> {
        if result.is_ok() {
            self.backing.phase = Phase::Prepared;
        }
        if let Err(Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))) = &result
        {
            self.backing.failure.get_or_insert(*error);
        }
        result
    }
}
impl<F: ProjectedAssertionFactsV1 + ?Sized> Drop for ProjectedViewPrefixLoanV1<'_, '_, F> {
    fn drop(&mut self) {
        self.backing.loan_returned = true;
    }
}

fn frame() -> Result<usize> {
    let rows = [
        size_of::<RetainedProjectedViewPrefixV1<'static>>(),
        size_of::<ProjectedViewPrefixLoanV1<'static, 'static, dyn ProjectedAssertionFactsV1>>(),
        size_of::<(
            Phase,
            Option<Snapshot>,
            Snapshot,
            Option<Resource>,
            Resource,
            Error,
            CanonicalAssertionErrorV1,
            Result<()>,
            Result<usize>,
            Option<usize>,
        )>(),
        size_of::<(
            Option<(&SemanticFunctionDeclV1, &[SemanticTypeDeclV1], Target)>,
            (&SemanticFunctionDeclV1, &[SemanticTypeDeclV1], Target),
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            Target,
            &[fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1],
        )>(),
        size_of::<(
            Vec<Option<ProjectedViewV1>>,
            &mut Vec<Option<ProjectedViewV1>>,
            Option<ProjectedViewV1>,
            Vec<QueriedSliceV1>,
            &Vec<QueriedSliceV1>,
            &Option<ProjectedViewV1>,
            std::slice::Iter<'static, Option<ProjectedViewV1>>,
            Option<ProjectedSemanticAccessSiteV1>,
            usize,
            usize,
            u32,
            bool,
            bool,
        )>(),
        size_of::<(
            &mut RetainedProjectedViewPrefixV1<'static>,
            &RetainedProjectedViewPrefixV1<'static>,
            &mut Prep<'static, 'static>,
            &Prep<'static, 'static>,
            &mut Option<Resource>,
            &mut Resource,
        )>(),
        size_of::<(
            Result<Snapshot>,
            Result<(&SemanticFunctionDeclV1, &[SemanticTypeDeclV1], Target)>,
            Result<ProjectedViewPrefixLoanV1<'static, 'static, dyn ProjectedAssertionFactsV1>>,
            Option<&mut dyn ProjectedAssertionFactsV1>,
            &mut dyn ProjectedAssertionFactsV1,
            Result<&mut dyn ProjectedAssertionFactsV1>,
        )>(),
        size_of::<(
            &[u8],
            Option<&fe2o3_pliron::ProductionSemanticSharedReadsV1<'static>>,
            Option<(
                &scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'static>,
                Target,
            )>,
            Option<&scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'static>>,
            Target,
        )>(),
        size_of::<(
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            &mut dyn FnMut() -> Result<()>,
            &mut dyn FnMut(&Option<ProjectedViewV1>) -> bool,
            usize,
            usize,
            bool,
        )>(),
        size_of::<(
            (
                usize,
                fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            ),
            Result<(
                usize,
                fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            )>,
            usize,
            Target,
            bool,
        )>(),
        size_of::<(
            [usize; 12],
            std::array::IntoIter<usize, 12>,
            usize,
            usize,
            Option<usize>,
        )>(),
        retained_custody_snapshot_frame_v1(),
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}

#[cfg(test)]
impl RetainedProjectedViewPrefixV1<'_> {
    /// Initial DATA only. Original facts/census bindings are tested by the actual
    /// whole-root loan, not invented in this independent old constructor.
    pub(in crate::production_ranked_projection_v1) fn compare_original_initial_v1(
        &self,
        original: &ProjectedViewsV1<'_>,
        resources: &mut Prep<'_, '_>,
    ) -> Result<()> {
        let count = self.locals.len();
        resources.work(
            count
                .checked_mul(2)
                .and_then(|n| n.checked_add(8))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )?;
        let excess = original
            .locals
            .capacity()
            .checked_sub(count)
            .ok_or_else(|| resource(Resource::Accounting))?
            .checked_mul(size_of::<Option<ProjectedViewV1>>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        resources.reserve_storage(excess)?;
        if original.locals.len() != count
            || original.locals.iter().any(Option::is_some)
            || self.locals.iter().any(Option::is_some)
            || original.site.is_some()
            || self.site.is_some()
            || original.source_start != self.source_start
            || original.guarded_start != self.guarded_start
            || original.next_access != self.next_access
            || !original.queried.is_empty()
            || !self.queried.is_empty()
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "retained_projected_view_prefix_v1_tests.rs"]
mod tests;
