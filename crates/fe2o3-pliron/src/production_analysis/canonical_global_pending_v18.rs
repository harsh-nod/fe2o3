//! Snapshot-bound native correspondence, not a completed global memory family.
use super::resources::discard;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirOperationCoordinateV1 as Coordinate,
    Operation, Terminator,
};

/// Borrowed observations of one real imported graph and its immutable owner.
/// Candidate carriers include nonvolatile Global loads/stores, scalar-Global
/// pointer producers and slice lengths of any address space. The composing
/// source join separately requires its exact scalar Global access recipe.
/// Bounds, initialization, read-from, aliasing, concurrency, target eligibility
/// and native stage coverage remain pending. No native context escapes.
///
/// ```compile_fail
/// use fe2o3_pliron::{PendingCanonicalGlobalAccessesV18, CheckedCanonicalRankedPoliciesV18};
/// fn promote(x: PendingCanonicalGlobalAccessesV18<'_, '_>)
///     -> CheckedCanonicalRankedPoliciesV18<'_, '_> { x }
/// ```
pub struct PendingCanonicalGlobalAccessesV18<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    graph: &'s crate::KirPlironGraphV18<'g>,
    epoch: u64,
    guard: &'s Guard,
    refund_denied: &'s Cell<bool>,
}

impl<'s, 'g> PendingCanonicalGlobalAccessesV18<'s, 'g> {
    // The mixed scope has completed both whole-owner replay and the exact
    // native global census. This factory is inaccessible outside that scope's
    // parent module, and does not discharge any global memory requirement.
    pub(super) fn after_mixed_census_v26(
        owner: &'g VerifiedCanonicalKernelIrModuleV18,
        graph: &'s crate::KirPlironGraphV18<'g>,
        epoch: u64,
        guard: &'s Guard,
        refund_denied: &'s Cell<bool>,
    ) -> Self {
        Self {
            owner,
            graph,
            epoch,
            guard,
            refund_denied,
        }
    }
}

impl PendingCanonicalGlobalAccessesV18<'_, '_> {
    /// A composing source owner can report a lost, still-live higher floor.
    /// This can only refuse the scope and suppress its refunds, never admit it.
    pub fn refuse_retained_custody(&self) -> Failure {
        self.refund_denied.set(true);
        self.guard.resource(Resource::Accounting)
    }

    /// Exact owner and native epoch are checked before any query work is charged.
    pub fn check_owner(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        budget: &Budget<'_>,
    ) -> Result<(), Failure> {
        self.guard.check(budget)?;
        if !std::ptr::eq(self.owner, owner) {
            return Err(self.guard.exact_graph());
        }
        self.graph
            .check_ranked_policy_epoch_v18(self.epoch)
            .map_err(|_| self.guard.mutation())
    }

    /// Dense coordinate lookup into the exact snapshot-bound owner. None is an
    /// unsupported carrier, never an empty or completed memory proof.
    pub fn operation(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&::fe2o3_kernel_ir::Operation>, Failure> {
        self.check_owner(owner, budget)?;
        budget
            .charge_work(16)
            .map_err(|error| self.guard.resource(error))?;
        let row = owner
            .module()
            .functions
            .get(coordinate.block.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block.block as usize))
            .and_then(|block| block.operations.get(coordinate.operation as usize))
            .ok_or_else(|| self.guard.exact_graph())?;
        Ok(self
            .graph
            .pending_global_operation_v18(coordinate)
            .map(|actual| {
                debug_assert!(std::ptr::eq(row, actual));
                actual
            }))
    }

    /// Actual snapshot-bound condition, successor order and edge payloads.
    /// This correspondence observation does not itself establish bounds.
    pub fn guard_terminator(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        coordinate: Block,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&::fe2o3_kernel_ir::Terminator>, Failure> {
        self.check_owner(owner, budget)?;
        budget
            .charge_work(12)
            .map_err(|error| self.guard.resource(error))?;
        let terminator = self
            .owner
            .module()
            .functions
            .get(coordinate.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block as usize))
            .and_then(|block| block.terminator.as_ref())
            .ok_or_else(|| self.guard.exact_graph())?;
        Ok(matches!(
            terminator,
            Terminator::ConditionalBranch { .. }
                | Terminator::Switch { .. }
                | Terminator::IntegerSwitch { .. }
        )
        .then_some(terminator))
    }

    pub const fn memory_safety_is_complete(&self) -> bool {
        false
    }
    pub const fn native_stage_coverage_is_complete(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn headers(capture: usize, alignment: usize) -> Result<usize, Failure> {
    fn h<T>() -> Result<usize, Failure> {
        checked_add(
            size_of::<T>(),
            size_of::<Result<T, Failure>>()
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
        )
    }
    let mut total = checked_add(
        capture,
        alignment.checked_mul(2).ok_or(Resource::Arithmetic)?,
    )?;
    for amount in [
        h::<PendingCanonicalGlobalAccessesV18<'_, '_>>()?,
        h::<Guard>()?,
        h::<&VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&crate::KirPlironGraphV18<'_>>()?,
        h::<&Guard>()?,
        h::<&mut Budget<'_>>()?,
        h::<Coordinate>()?,
        h::<Block>()?,
        h::<&Operation>()?,
        h::<Option<&Operation>>()?,
        h::<&Terminator>()?,
        h::<Option<&Terminator>>()?,
        h::<fe2o3_kernel_ir::OperationKind>()?,
        h::<fe2o3_kernel_ir::Type>()?,
        h::<fe2o3_kernel_ir::ValueId>()?,
        h::<[usize; 8]>()?,
        h::<u64>()?,
        h::<()>()?,
        h::<Result<(), Failure>>()?,
        h::<std::thread::Result<Result<(), Failure>>>()?,
        h::<Option<Failure>>()?,
        h::<Option<Failure>>()?,
        h::<(Ledger, usize, usize, usize)>()?,
        h::<(
            &PendingCanonicalRankedSourceRolesV18<'_, '_>,
            &mut Option<Failure>,
        )>()?,
        crate::KirPlironGraphV18::pending_global_scan_headers_v18()?,
        drain_header(),
        h::<(&PendingCanonicalGlobalAccessesV18<'_, '_>, &mut Budget<'_>)>()?,
    ] {
        total = checked_add(total, amount)?;
    }
    Ok(total)
}

impl PendingCanonicalRankedSourceRolesV18<'_, '_> {
    fn retain_global_failure_v18(&self, error: Failure) -> Failure {
        match error {
            Failure::Resource(error) => self.guard.resource(error),
            Failure::Mutation => self.guard.mutation(),
            Failure::ExactGraph => self.guard.exact_graph(),
            error => error,
        }
    }

    /// No policy runs here. The exact graph was imported and round-tripped by
    /// the enclosing source-role scope. The unit callback cannot export a view.
    pub fn with_pending_global_accesses_v18<'w>(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        budget: &mut Budget<'w>,
        callback: impl for<'s, 'g> FnOnce(
            &PendingCanonicalGlobalAccessesV18<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        self.guard.check(budget)?;
        if !std::ptr::eq(owner, self.owner) {
            return Err(self.guard.exact_graph());
        }
        self.graph
            .check_ranked_policy_epoch_v18(self.epoch)
            .map_err(|_| self.guard.mutation())?;
        let capture = std::mem::size_of_val(&callback);
        let alignment = std::mem::align_of_val(&callback);
        budget
            .charge_work(2)
            .map_err(|error| self.guard.resource(error))?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&*budget) as usize;
        budget
            .reserve_storage(
                headers(capture, alignment)
                    .map_err(|error| self.retain_global_failure_v18(error))?,
            )
            .map_err(|error| self.guard.resource(error))?;
        let retained = budget.storage();
        let mut construction_error = None;
        let caught = catch_unwind(AssertUnwindSafe(|| {
            if let Err(error) = self
                .graph
                .check_pending_global_carriers_v18(owner, self.epoch, budget)
            {
                // Select before dropping an uninvoked closure: its captures can panic.
                construction_error = Some(self.retain_global_failure_v18(error));
                drop(callback);
                return Ok(());
            }
            let guard = Guard::new(budget);
            let view = PendingCanonicalGlobalAccessesV18 {
                owner: self.owner,
                graph: self.graph,
                epoch: self.epoch,
                guard: &guard,
                refund_denied: self.refund_denied,
            };
            let result = guard.callback(budget, |budget| callback(&view, budget));
            self.graph
                .check_ranked_policy_epoch_v18(self.epoch)
                .map_err(|_| guard.mutation())?;
            result
        }));
        if let Ok(Err(error)) = &caught {
            match error {
                Failure::Resource(error) => {
                    let _ = self.guard.resource(*error);
                }
                Failure::ExactGraph => {
                    let _ = self.guard.exact_graph();
                }
                Failure::Mutation => {
                    let _ = self.guard.mutation();
                }
                _ => {}
            }
        }
        let prior = construction_error.or_else(|| self.guard.check(budget).err());
        // A lost higher scope floor must never be hidden by refunding to the
        // still-present parent floor. Callback owners die before any refund.
        if self.refund_denied.get()
            || ledger != budget.work_ledger_identity_v1()
            || slot != std::ptr::from_ref(&*budget) as usize
            || budget.storage() < retained
        {
            self.refund_denied.set(true);
            discard(caught);
            return Err(self.guard.resource(Resource::Accounting));
        }
        let result = match (prior, caught) {
            (Some(error), caught) => {
                discard(caught);
                Err(error)
            }
            (None, Ok(result)) => result,
            (None, Err(payload)) => {
                discard(payload);
                Err(Failure::Panicked)
            }
        };
        if self.refund_denied.get()
            || ledger != budget.work_ledger_identity_v1()
            || slot != std::ptr::from_ref(&*budget) as usize
            || budget.storage() < retained
        {
            self.refund_denied.set(true);
            discard(result);
            return Err(self.guard.resource(Resource::Accounting));
        }
        budget
            .release_storage(budget.storage() - floor)
            .map_err(|error| self.guard.resource(error))?;
        result.map_err(|error| match error {
            Failure::Resource(error) => self.guard.resource(error),
            Failure::Mutation => self.guard.mutation(),
            Failure::ExactGraph => self.guard.exact_graph(),
            error => error,
        })
    }
}

#[cfg(test)]
#[path = "canonical_global_pending_v18_tests.rs"]
mod tests;
