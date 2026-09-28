//! Retained FIFO scratch only; no source, capability or checkpoint authority.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::resource;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;
type Error = ProductionRankedProjectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Resources<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}

/// Caller owns origins/edges and must hold them alongside this queue until its
/// checked postflight. This component itself grants no authenticated authority.
pub(in crate::production_ranked_projection_v1) struct RetainedExactOriginWorklistV1 {
    phase: Phase,
    ledger: Option<Ledger>,
    worklist: Vec<usize>,
    head: usize,
    work: usize,
}
impl RetainedExactOriginWorklistV1 {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            worklist: Vec::new(),
            head: 0,
            work: 0,
        }
    }
    pub(in crate::production_ranked_projection_v1) fn prepare_into<T: Copy + Eq>(
        &mut self,
        origins: &mut [Option<T>],
        edges: &[Vec<usize>],
        conflict: &'static str,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        let fresh = self.phase == Phase::Fresh && self.ledger.is_none();
        self.phase = Phase::Terminal;
        if !fresh || !resources.is_metered() || resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        let ledger = resources
            .original_ledger_v1()
            .ok_or_else(|| resource(Resource::Accounting))?;
        resources.work(32)?;
        resources.reserve_storage(retained_origin_worklist_frame_v1::<T>()?)?;
        self.ledger = Some(ledger);
        self.propagate_attached(origins, edges, conflict, resources)?;
        if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
            return Err(resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    /// Completion of this scratch operation only, not source/table authenticity.
    pub(in crate::production_ranked_projection_v1) fn completed(
        &self,
        resources: &Resources<'_, '_>,
    ) -> bool {
        self.phase == Phase::Complete
            && self.ledger.is_some()
            && self.ledger == resources.original_ledger_v1()
            && !resources.has_denial()
    }

    fn propagate_attached<T: Copy + Eq>(
        &mut self,
        origins: &mut [Option<T>],
        edges: &[Vec<usize>],
        conflict: &'static str,
        resources: &mut Resources<'_, '_>,
    ) -> Result<()> {
        resources.reserve_storage(std::mem::size_of::<Vec<usize>>() + 4096)?;
        if origins.len() != edges.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "local provenance tables have inconsistent lengths",
            ));
        }
        resources.reserve(&mut self.worklist, origins.len())?;
        for (local, origin) in origins.iter().enumerate() {
            resources.work(1)?;
            if origin.is_some() {
                resources.push(&mut self.worklist, local)?;
            }
        }
        while let Some(&source) = self.worklist.get(self.head) {
            resources.work(1)?;
            self.head += 1;
            let Some(origin) = origins[source] else {
                continue;
            };
            for &destination in &edges[source] {
                resources.work(1)?;
                self.work = self.work.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "local provenance dataflow work accounting overflowed",
                    ),
                )?;
                if self.work > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "local provenance dataflow exceeds the charged projection limit",
                    ));
                }
                match origins[destination] {
                    None => {
                        origins[destination] = Some(origin);
                        resources.push(&mut self.worklist, destination)?;
                    }
                    Some(existing) if existing == origin => {}
                    Some(_) => {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(conflict));
                    }
                }
            }
        }
        Ok(())
    }
}
const FRAME_ROWS: usize = 15;
fn typed_rows<T: Copy + Eq>() -> Result<[usize; FRAME_ROWS]> {
    Ok([
        size_of::<RetainedExactOriginWorklistV1>(),
        size_of::<(Phase, Option<Ledger>, Vec<usize>, usize, usize)>(),
        size_of::<(
            &mut RetainedExactOriginWorklistV1,
            &mut [Option<T>],
            &[Vec<usize>],
            &'static str,
            &mut Resources<'static, 'static>,
            bool,
            Ledger,
            Option<Ledger>,
            Result<()>,
        )>(),
        size_of::<(
            &RetainedExactOriginWorklistV1,
            &Resources<'static, 'static>,
            bool,
            Option<Ledger>,
        )>(),
        size_of::<(
            &mut RetainedExactOriginWorklistV1,
            &mut [Option<T>],
            &[Vec<usize>],
            &'static str,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'_, Option<T>>>,
            usize,
            &Option<T>,
            bool,
        )>(),
        size_of::<(
            &mut Vec<usize>,
            usize,
            usize,
            &mut Resources<'static, 'static>,
            Result<()>,
        )>(),
        size_of::<(Option<&usize>, &usize, usize, Option<T>, T)>(),
        size_of::<(
            std::slice::Iter<'static, usize>,
            &usize,
            usize,
            usize,
            Option<usize>,
            T,
            Option<T>,
            T,
        )>(),
        size_of::<(&mut usize, usize, Option<usize>, Error, Result<()>)>(),
        size_of::<(Error, Resource, Result<()>, Option<Ledger>, bool)>(),
        size_of::<(
            [usize; FRAME_ROWS],
            Result<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
        size_of::<(Result<usize>, usize, Option<usize>, Resource, Error)>(),
        size_of::<(&usize, usize, &mut Vec<usize>, usize, Result<()>)>(),
    ])
}
pub(in crate::production_ranked_projection_v1) fn retained_origin_worklist_frame_v1<
    T: Copy + Eq,
>() -> Result<usize> {
    typed_rows::<T>()?.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
#[cfg(test)]
#[path = "bf16_nominal_retained_origin_worklist_v1_tests.rs"]
mod tests;
