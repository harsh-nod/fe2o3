//! Backend resolution for the lowerer's closed private-memory root requests.
//! The native adapter only borrows independently completed lower tokens.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionLifecycleCheckedNativePoliciesV18 as Lifecycle,
    ProductionOptimizedSourceCorrespondenceV18 as Optimized,
    ProductionPrivateMemoryCheckedNativePoliciesV18 as Private,
    ProductionSourceCorrespondenceV18 as Original,
    ProductionSourceNativeLifecycleDiagnosticV18 as Diagnostic,
    ProductionSourceNativeLifecycleErrorV18 as Error,
    ProductionSourceOwnedViewErrorV18 as SourceError,
    ProductionSourcePrivateMemoryRootRequestV18 as Request,
    ProductionSourceScalarLeavesV18 as SourceLeaves,
};

// The production policy envelope prepays this entrance before request.root or
// the outer SourceOnly closure runs. Inner lexical scratch has its own credit.
pub(crate) fn private_root_bridge_entrance_headers_v18() -> Option<usize> {
    use std::mem::{align_of, size_of};
    type Call<'a> = (
        &'a Original<'a>,
        &'a Optimized<'a>,
        &'a Request<'a>,
        &'a mut Budget<'a>,
    );
    type Outer<'a> = (&'a Original<'a>, &'a Request<'a>);
    type Args<'a> = (
        &'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'a>,
        &'a mut Budget<'a>,
    );
    type Inner<'a> = (
        &'a Original<'a>,
        &'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'a>,
        &'a Request<'a>,
    );
    [
        size_of::<Call<'_>>(),
        2 * align_of::<Call<'_>>(),
        size_of::<Outer<'_>>(),
        2 * align_of::<Outer<'_>>(),
        size_of::<Args<'_>>(),
        2 * align_of::<Args<'_>>(),
        size_of::<Inner<'_>>(),
        2 * align_of::<Inner<'_>>(),
        size_of::<usize>(),
        size_of::<Result<usize, SourceError>>(),
        size_of::<&SourceLeaves<'_>>(),
        size_of::<Result<&SourceLeaves<'_>, SourceError>>(),
        3 * size_of::<Result<(), ProductionRankedProjectionErrorV1>>(),
        3 * size_of::<Result<(), SourceError>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
}

pub(crate) fn check_source_private_memory_root_prepaid_v18(
    original: &Original<'_>,
    optimized: &Optimized<'_>,
    request: &Request<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let root = request.root(budget)?;
    original.with_optimized_source_scalar_leaves_v18(optimized, root, budget, |leaves, budget| {
        let source = leaves.original_leaves(budget)?;
        with_private_root_bridge_scratch(original, source, budget, |budget| {
            let semantic = original.source(budget)?.source_semantic(budget)?;
            optimized_source_consumer_v18::with_checked_source_entry_writes_v18(
                semantic,
                leaves,
                budget,
                |entries, budget| Ok(request.check_entry_writes(entries, budget)?),
            )
        })
    })
}

fn private_root_bridge_headers(callback: usize, alignment: usize) -> Option<usize> {
    use std::mem::size_of;
    [
        callback,
        alignment.checked_mul(2)?,
        2 * size_of::<&Original<'_>>(),
        size_of::<&Optimized<'_>>(),
        size_of::<&Request<'_>>(),
        size_of::<&SourceLeaves<'_>>(),
        size_of::<&mut Budget<'_>>(),
        size_of::<&fe2o3_lower_mir_kernel::ProductionOptimizedSourceScalarLeavesV18<'_>>(),
        size_of::<Result<&SourceLeaves<'_>, SourceError>>(),
        size_of::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>(),
        size_of::<Result<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>, SourceError>>(),
        size_of::<&AdmittedInertSemanticMirV1>(),
        size_of::<Result<&AdmittedInertSemanticMirV1, SourceError>>(),
        size_of::<Result<usize, SourceError>>(),
        4 * size_of::<usize>(),
        size_of::<Option<usize>>(),
        2 * size_of::<Result<(), SourceError>>(),
        4 * size_of::<Result<(), ProductionRankedProjectionErrorV1>>(),
        2 * size_of::<Result<(), fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>>(),
        size_of::<std::thread::Result<Result<(), ProductionRankedProjectionErrorV1>>>(),
        size_of::<&mut Budget<'_>>(),
        3 * size_of::<bool>(),
        2 * size_of::<&()>(),
        2 * std::mem::align_of::<&()>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
}

fn with_private_root_bridge_scratch(
    original: &Original<'_>,
    leaves: &SourceLeaves<'_>,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<(), ProductionRankedProjectionErrorV1>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    leaves.check(budget)?;
    let resource = |error| {
        ProductionRankedProjectionErrorV1::from(original.retain_query_resource_error_v18(error))
    };
    let headers =
        private_root_bridge_headers(std::mem::size_of_val(&run), std::mem::align_of_val(&run))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.reserve_storage(headers).map_err(resource)?;
    let required = budget.storage();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
    let succeeded = matches!(&caught, Ok(Ok(())));
    let query = if succeeded {
        leaves
            .check(budget)
            .map_err(ProductionRankedProjectionErrorV1::from)
    } else {
        Ok(())
    };
    let custody = leaves.observe_custody(budget);
    // Cleanup failure cannot invent an earlier source query than a selected
    // callback error or panic. Refund denial still reaches every outer scope.
    let cleanup_error = |error| {
        if succeeded {
            resource(error)
        } else {
            ProductionRankedProjectionErrorV1::from(SourceError::Resource(error))
        }
    };
    let postflight = if custody.is_err() || budget.storage() < required {
        leaves.deny_refund();
        Err(cleanup_error(Resource::Accounting))
    } else if succeeded && budget.storage() != required {
        Err(cleanup_error(Resource::Accounting))
    } else {
        // The callback and its values have dropped. Release only this fixed
        // frame, including on selected error/unwind; never caller-owned credit.
        budget.release_storage(headers).map_err(cleanup_error)
    };
    match caught {
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(Err(error)) => Err(error),
        Ok(Ok(())) => query.and(postflight),
    }
}

#[cfg(test)]
#[path = "optimized_source_private_consumer_v18_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::inspect_actual_private_bridge_scratch_v18;

/// A closed backend dispatcher, not a lower completion conversion. Each arm
/// retains the exact source subject, native history and custody implementation
/// of its authentic lower token. Pending physical observations have no arm.
pub(crate) enum SourceNativePolicyViewV18<'a, 'scope, 'owner> {
    Lifecycle(&'a Lifecycle<'scope, 'owner>),
    Private(&'a Private<'scope, 'owner>),
}

impl SourceNativePolicyViewV18<'_, '_, '_> {
    pub(crate) const fn is_private_memory(&self) -> bool {
        matches!(self, Self::Private(_))
    }
    pub(crate) fn check_source_subject_v18(
        &self,
        original: &Original<'_>,
        optimized: &Optimized<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        match self {
            Self::Lifecycle(view) => view.check_source_subject_v18(original, optimized, budget),
            Self::Private(view) => view.check_source_subject_v18(original, optimized, budget),
        }
    }

    pub(crate) fn retain_source_binding_error_v18(&self, detail: &'static str) -> Error {
        match self {
            Self::Lifecycle(view) => view.retain_source_binding_error_v18(detail),
            Self::Private(view) => view.retain_source_binding_error_v18(detail),
        }
    }

    pub(crate) fn check_retained_root_storage_v18(
        &self,
        required: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        match self {
            Self::Lifecycle(view) => view.check_retained_root_storage_v18(required, budget),
            Self::Private(view) => view.check_retained_root_storage_v18(required, budget),
        }
    }

    pub(crate) fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Error> {
        match self {
            Self::Lifecycle(view) => view.function_count(budget),
            Self::Private(view) => view.function_count(budget),
        }
    }

    pub(crate) fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&fe2o3_pliron::ProductionPlironPreloweringReportV2>, Error> {
        match self {
            Self::Lifecycle(view) => view.report(function, budget),
            Self::Private(view) => view.report(function, budget),
        }
    }

    pub(crate) fn diagnostic(&self, budget: &mut Budget<'_>) -> Result<Diagnostic, Error> {
        match self {
            Self::Lifecycle(view) => view.diagnostic(budget),
            Self::Private(view) => view.diagnostic(budget),
        }
    }
}
