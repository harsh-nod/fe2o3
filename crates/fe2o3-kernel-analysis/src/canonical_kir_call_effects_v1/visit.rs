use super::*;
use crate::{CanonicalKirEffectRefV1, CanonicalKirOperationRefV1};

/// Original graph payload, not a reconstructed effect or a caller-local access.
#[derive(Clone, Copy, Debug)]
pub enum CanonicalKirCallEffectKindV1<'i, 'g> {
    Physical(&'i CanonicalKirEffectRefV1<'g>),
    CompilerOrdering(&'i CanonicalKirOperationRefV1<'g>),
}

/// One syntactic effect instance at one exact call path. A loop's dynamic
/// iteration count is not expanded or proved here. Each call-path index refers
/// to the same inventory's calls(), retaining actual operands and callee identity.
#[derive(Debug)]
pub struct CanonicalKirCallEffectOccurrenceV1<'p, 'i, 'g> {
    inventory: &'i Inventory<'g>,
    root: Function,
    function: Function,
    call_path: &'p [usize],
    kind: CanonicalKirCallEffectKindV1<'i, 'g>,
}
impl<'p, 'i, 'g> CanonicalKirCallEffectOccurrenceV1<'p, 'i, 'g> {
    pub const fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }
    pub const fn root(&self) -> Function {
        self.root
    }
    pub const fn function(&self) -> Function {
        self.function
    }
    pub const fn call_path(&self) -> &'p [usize] {
        self.call_path
    }
    pub const fn kind(&self) -> CanonicalKirCallEffectKindV1<'i, 'g> {
        self.kind
    }
}

impl<'i, 'g> CanonicalKirCallEffectsV1<'i, 'g> {
    /// Visits every physical and compiler-order occurrence in deterministic
    /// depth-first stored inventory order, retaining repeated calls and shared helpers.
    /// Incomplete roots reject before callbacks. Complete roots may still exhaust
    /// the caller's work budget during path expansion: callbacks are provisional
    /// until Ok, and must not issue authority or irreversible side effects.
    ///
    /// Traversal uses O(functions) scratch, no recursive host stack, and charges
    /// every expanded operation/effect/call before visiting. Success, error and
    /// callback unwind restore the incoming storage floor. Graph, inventory and
    /// report must already be caller-reserved; callback storage/work is separate.
    ///
    /// ```
    /// use fe2o3_kernel_analysis::{CanonicalKirCallEffectsV1, CanonicalKirCallEffectErrorV1 as E};
    /// use fe2o3_kernel_ir::{CanonicalKirFunctionCoordinateV1 as F, CanonicalKernelIrVerificationResourceBudgetV1 as B};
    /// fn inspect(report: &CanonicalKirCallEffectsV1<'_, '_>, budget: &mut B<'_>) -> Result<(), E> {
    ///     report.try_visit(F(0), budget, |effect| {
    ///         assert!(report.belongs_to(effect.inventory()));
    ///         for call in effect.call_path() { let _ = &effect.inventory().calls()[*call]; }
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_analysis::{CanonicalKirCallEffectsV1, CanonicalKirCallEffectErrorV1 as E};
    /// use fe2o3_kernel_ir::{CanonicalKirFunctionCoordinateV1 as F, CanonicalKernelIrVerificationResourceBudgetV1 as B};
    /// fn escape(report: &CanonicalKirCallEffectsV1<'_, '_>, budget: &mut B<'_>) {
    ///     let mut saved = None;
    ///     report.try_visit(F(0), budget, |effect| { saved = Some(effect); Ok::<_, E>(()) }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn try_visit<E: From<Error>>(
        &self,
        root: Function,
        budget: &mut Budget<'_>,
        mut visit: impl for<'p> FnMut(
            CanonicalKirCallEffectOccurrenceV1<'p, 'i, 'g>,
        ) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        if self.decision(root, budget)? == Decision::Incomplete {
            return Err(Error::Incomplete(root).into());
        }
        let floor = budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.visit_inner(root, budget, &mut visit)
        }));
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Error::Resource(Resource::Accounting))?,
            )
            .map_err(Error::from)?;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn visit_inner<E: From<Error>>(
        &self,
        root: Function,
        budget: &mut Budget<'_>,
        visit: &mut impl for<'p> FnMut(
            CanonicalKirCallEffectOccurrenceV1<'p, 'i, 'g>,
        ) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        let inventory = self.inventory;
        let count = inventory.functions().len();
        let mut stack = vector::<Frame>(count, budget)?;
        let mut path = vector::<usize>(count, budget)?;
        stack.push(frame(inventory, root)?);
        while let Some(current) = stack.last_mut() {
            budget.charge_work(1).map_err(Error::from)?;
            let row = &inventory.functions()[current.function.0 as usize];
            if current.operation == row.operations.end {
                stack.pop();
                path.pop();
                continue;
            }
            let operation = inventory
                .operations()
                .get(current.operation)
                .ok_or(Error::InconsistentInventory)?;
            current.operation += 1;
            for effect in operation.effects.clone() {
                budget.charge_work(1).map_err(Error::from)?;
                visit(CanonicalKirCallEffectOccurrenceV1 {
                    inventory,
                    root,
                    function: current.function,
                    call_path: &path,
                    kind: CanonicalKirCallEffectKindV1::Physical(
                        inventory
                            .effects()
                            .get(effect)
                            .ok_or(Error::InconsistentInventory)?,
                    ),
                })?;
            }
            if !operation.compiler_ordering().is_empty() {
                budget.charge_work(1).map_err(Error::from)?;
                visit(CanonicalKirCallEffectOccurrenceV1 {
                    inventory,
                    root,
                    function: current.function,
                    call_path: &path,
                    kind: CanonicalKirCallEffectKindV1::CompilerOrdering(operation),
                })?;
            }
            if !matches!(operation.operation.kind, OperationKind::Call { .. }) {
                continue;
            }
            let call_index = current.call;
            let call = inventory
                .calls()
                .get(call_index)
                .ok_or(Error::InconsistentInventory)?;
            current.call += 1;
            if operation
                .operation
                .has_complete_effect_summary_with_budget_v1(budget)
                .map_err(Error::from)?
            {
                continue;
            }
            let target = call.target.ok_or(Error::InconsistentInventory)?;
            if stack.len() >= count {
                return Err(Error::InconsistentInventory.into());
            }
            path.push(call_index);
            stack.push(frame(inventory, target)?);
        }
        Ok(())
    }
}
