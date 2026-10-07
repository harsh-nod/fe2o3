//! Observations from the actual checked whole-Slice locator, not proof evidence.
use super::{Resource, Result, Writer};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirFunctionCoordinateV1 as Function,
};
use fe2o3_mir_model::SsaValueV1;
use std::mem::size_of;

/// A flat, complete traversal emitted while the same original/target owners live.
/// Definition indices select full types in those inventories; no cloned type,
/// detached digest, or row supplied by a consumer admits a binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpandedSupportForwardingV288 {
    /// Original Product Slice component selected by the frame-demand emitter.
    Begin {
        /// Canonical original root ordinal.
        root: usize,
        /// Root-local original call instance.
        instance: usize,
        /// Exact original Product SSA value.
        value: SsaValueV1,
        /// Owner-derived Product component ordinal.
        atom: usize,
        /// Original semantic component type ordinal.
        source_type: u32,
        /// Dense original definition index and its authenticated coordinate.
        original: usize,
        /// Coordinate selecting the same complete physical type.
        coordinate: Definition,
    },
    /// A current original definition has no retained neutral descendants.
    Erased {
        /// Dense original definition index.
        original: usize,
        /// Exact original coordinate.
        coordinate: Definition,
    },
    /// One authenticated edge into the current erased block argument.
    Incoming {
        /// Current dense original definition.
        original: usize,
        /// Exact original edge-argument coordinate.
        edge: EdgeArgument,
        /// Dense original incoming definition.
        incoming: usize,
        /// Exact original incoming coordinate.
        coordinate: Definition,
    },
    /// Terminal original definition with nonempty retained neutral descendants.
    Retained {
        /// Dense original definition.
        original: usize,
        /// Exact original definition coordinate.
        coordinate: Definition,
        /// Actual retained descendant count.
        descendants: usize,
    },
    /// Chosen actual target, after existing original-root membership admission.
    Target {
        /// Dense actual target definition, selecting its complete physical type.
        actual: usize,
        /// Exact actual target definition coordinate.
        coordinate: Definition,
        /// Actual function authenticated for the original root.
        function: Function,
    },
}

pub(super) type Observer<'a> =
    dyn FnMut(ExpandedSupportForwardingV288, &mut Writer<'_, '_>) -> Result<()> + 'a;

pub(super) fn headers() -> usize {
    4 * size_of::<ExpandedSupportForwardingV288>()
        + 8 * size_of::<&mut Observer<'_>>()
        + 2 * size_of::<Option<&mut Observer<'_>>>()
        + 16 * size_of::<usize>()
        + 8 * size_of::<&()>()
        + 3 * size_of::<Result<()>>()
        + size_of::<std::collections::TryReserveError>()
}

pub(super) fn append(
    rows: &mut Vec<ExpandedSupportForwardingV288>,
    event: ExpandedSupportForwardingV288,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        // The enclosing model scope owns and drops this backing before refund.
        let added = rows.capacity().max(16);
        let planned = rows
            .capacity()
            .checked_add(added)
            .ok_or(Resource::Arithmetic)?;
        let bytes = added
            .checked_mul(size_of::<ExpandedSupportForwardingV288>())
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(bytes)?;
        rows.try_reserve_exact(added)
            .map_err(|_| Resource::Allocation)?;
        if rows.capacity() > planned {
            out.budget.reserve_storage(
                (rows.capacity() - planned)
                    .checked_mul(size_of::<ExpandedSupportForwardingV288>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
    }
    rows.push(event);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::{Error, SOURCE_LIMIT};
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn expanded_forwarding_rows_charge_exact_growth_and_restore_short_and_unwind_scopes() {
        // Synthetic rows exercise only storage ownership, not locator admission.
        let event = ExpandedSupportForwardingV288::Erased {
            original: 0,
            coordinate: Definition::FunctionArgument {
                function: Function(0),
                argument: 0,
            },
        };
        let run = |limit, panic| {
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, limit);
            let ledger = budget.work_ledger_identity_v1();
            let slot = std::ptr::from_ref(&budget) as usize;
            let operation = |budget: &mut Budget<'_>| -> Result<usize> {
                let mut out = Writer::new(budget)?;
                let mut rows = Vec::new();
                for _ in 0..17 {
                    append(&mut rows, event, &mut out)?;
                }
                assert_eq!(rows.as_slice(), &[event; 17]);
                assert!(rows.capacity() >= 32);
                if panic {
                    std::panic::panic_any(288usize);
                }
                Ok(rows.capacity())
            };
            let prepaid = SOURCE_LIMIT
                + headers()
                + size_of::<Writer<'_, '_>>()
                + size_of::<Vec<ExpandedSupportForwardingV288>>()
                + 2 * std::mem::size_of_val(&operation)
                + std::mem::align_of_val(&operation);
            let result = catch_unwind(AssertUnwindSafe(|| {
                budget.with_prepaid_scope(0, 1, 1, prepaid, operation)
            }));
            assert_eq!(budget.storage(), 0);
            assert_eq!(std::ptr::from_ref(&budget) as usize, slot);
            assert!(budget.work_ledger_identity_v1() == ledger);
            (result, budget.peak_storage())
        };
        let (full, peak) = run(8 * SOURCE_LIMIT, false);
        assert!(full.unwrap().is_ok());
        assert!(run(peak, false).0.unwrap().is_ok());
        assert!(matches!(
            run(peak - 1, false).0.unwrap(),
            Err(Error::Resource(_))
        ));
        let panic = run(peak, true).0.unwrap_err();
        assert_eq!(*panic.downcast::<usize>().unwrap(), 288);
    }
}
