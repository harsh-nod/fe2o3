//! Fixed original-source report observation with live compiler binding custody.
//! No report, coordinate value, or continuation here admits final output.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19 as Launch, CanonicalFormalReportViewV19 as Report,
    ControlFlowLimits, ExplicitLaunchExtent, FormalIndexWidth,
};
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceReportsErrorV19 as ReportsError, ProductionSourceOptimizationErrorV18,
};

pub(super) type ReportOptimizationErrorV19 =
    ProductionSourceOptimizationErrorV18<ReportsError<Error>>;

/// Retains a trusted compiler callback through preparation and header refusals.
/// Its destructor cannot replace an already selected compiler error.
pub(super) struct PendingConsumerV19<F>(Option<F>);

impl<F> PendingConsumerV19<F> {
    pub(super) fn new(consume: F) -> Self {
        Self(Some(consume))
    }
    pub(super) fn take(&mut self) -> F {
        self.0.take().expect("single source-owned consumer")
    }
}

impl<F> Drop for PendingConsumerV19<F> {
    fn drop(&mut self) {
        discard(self.0.take());
    }
}

fn geometry(error: crate::production_geometry_v1::ProductionGeometryErrorV1) -> Error {
    ProductionPipelineError::Geometry(error).into()
}

fn closure() -> Error {
    geometry(crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure)
}

fn check_root_names(
    entry: &fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1,
    kernel: &fe2o3_kernel_ir::Kernel,
    actual_function: &fe2o3_kernel_ir::Function,
    binding: [u8; 32],
    symbol: &str,
) -> Result<(), Error> {
    if entry.kernel_binding_identity().as_bytes() != &binding
        || entry.export_symbol().as_bytes() != symbol.as_bytes()
        || kernel.id.as_str() != symbol
        || kernel.entry.as_str() != symbol
        || actual_function.id != kernel.entry
    {
        return Err(closure());
    }
    Ok(())
}

// Only fixed-size source/descriptor fields and the two retained production
// profiles reach the shared coordinate prefix. Pay its three axis traversals,
// field checks and five bounded target-name passes: recognized processors,
// parsing feature support, validating feature support, ISA-family membership,
// and the remaining matrix/advanced-profile selectors. Each pass is bounded by
// KNOWN_PROCESSORS comparisons; fixed profile constructors get 128 more units.
// The full legacy geometry's reachable graph/resource scans are NOT called.
fn coordinate_work(target: TargetProfile) -> Result<usize, Resource> {
    let target_bytes = target.device_target().len();
    fe2o3_amd_target::KNOWN_PROCESSORS
        .len()
        .checked_mul(target_bytes.checked_add(1).ok_or(Resource::Arithmetic)?)
        .and_then(|work| work.checked_mul(5))
        .and_then(|work| work.checked_add(8 * target_bytes))
        .and_then(|work| work.checked_add(128 + 3 * 32))
        .ok_or(Resource::Arithmetic)
}

impl SourceBindingContextV29<'_> {
    pub(super) fn launches(
        &self,
        source: &Source<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Vec<Launch>, FormalIndexWidth), Error> {
        budget.check_prior_denials_v1()?;
        let semantic = source.source_semantic(budget)?;
        let canonical = source.canonical(budget)?;
        let module = canonical.module();
        let count = source.root_count(budget)?;
        budget.charge_work(4)?;
        if count == 0
            || semantic.roots().len() != count
            || module.kernels.len() != count
            || self.bindings.typed_descriptor_roots.len() != count
        {
            return Err(closure());
        }
        let mut result = paid_vec(count, budget)?;
        let target = self.bindings.rustc_target.profile();
        for (ordinal, ((descriptor, semantic_root), kernel)) in self
            .bindings
            .typed_descriptor_roots
            .iter()
            .zip(semantic.roots())
            .zip(&module.kernels)
            .enumerate()
        {
            budget.charge_work(6)?;
            let function = semantic
                .functions()
                .get(semantic_root.index() as usize)
                .ok_or_else(closure)?;
            let entry = function.kernel_entry().ok_or_else(closure)?;
            let (actual_root, canonical_entry) = source.root(ordinal, budget)?;
            let actual_function = module.functions.get(canonical_entry).ok_or_else(closure)?;
            let symbol = descriptor.entry_symbol();
            let name_work = symbol
                .len()
                .checked_add(entry.export_symbol().as_bytes().len())
                .and_then(|work| work.checked_add(kernel.id.as_str().len()))
                .and_then(|work| work.checked_add(kernel.entry.as_str().len()))
                .and_then(|work| work.checked_add(actual_function.id.as_str().len()))
                .and_then(|work| work.checked_add(32 + 6))
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(name_work)?;
            if actual_root != *semantic_root {
                return Err(closure());
            }
            check_root_names(
                entry,
                kernel,
                actual_function,
                descriptor.kernel_binding_bytes(),
                symbol,
            )?;
            let launch = descriptor.source_launch().ok_or_else(|| {
                geometry(crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup)
            })?;
            budget.charge_work(coordinate_work(target)?)?;
            let coordinates =
                crate::production_geometry_v1::derive_original_coordinate_geometry_v19(
                    kernel, function, launch, target,
                )
                .map_err(geometry)?;
            let extents = coordinates
                .formal_coordinate_envelope_v19()
                .map_err(geometry)?;
            budget.charge_work(1)?;
            result.push(Launch::PhysicalEnvelope(ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            }));
        }
        // The retained target selects this actual AMD lowering implementation.
        // Index width comes from its Index helpers, never its pointer ABI.
        budget.charge_work(3)?;
        let width = fe2o3_amdgcn_model::production_logical_index_width_v19();
        if matches!(width, FormalIndexWidth::Unknown) {
            return Err(Error::Unsupported("AMD logical Index lowering width"));
        }
        Ok((result, width))
    }
}

pub(super) fn launch_context_headers_v19() -> Result<usize, Resource> {
    [
        size_of::<Vec<Launch>>(),
        size_of::<crate::production_geometry_v1::ProductionCoordinateGeometryV19>(),
        size_of::<fe2o3_amd_target::AmdTargetCapabilities>(),
        size_of::<ProductionPipelineError>(),
        align_of::<ProductionPipelineError>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

struct OriginalFormalReportsV19;

impl<F> SourceHandoffPolicyV29<(), F> for OriginalFormalReportsV19
where
    F: for<'before, 'input, 'after, 'output, 'work> FnMut(
        &Report<'before, 'input>,
        &Report<'after, 'output>,
        &mut Budget<'work>,
    ) -> Result<(), Error>,
{
    fn entry_headers() -> Result<usize, Resource> {
        entry_headers_for_handoff::<(), F, IntegerHandoff<'static, 'static>>()?
            .checked_add(launch_context_headers_v19()?)
            .and_then(|value| value.checked_add(size_of::<Option<F>>()))
            .and_then(|value| value.checked_add(size_of::<Result<(), Error>>()))
            .and_then(|value| {
                value.checked_add(size_of::<std::thread::Result<Result<(), Error>>>())
            })
            // Reserve the diagnostic Box payload before an optimizer/report
            // refusal; never try to charge a new error carrier after denial.
            .and_then(|value| value.checked_add(size_of::<ReportOptimizationErrorV19>()))
            .and_then(|value| value.checked_add(align_of::<ReportOptimizationErrorV19>()))
            .ok_or(Resource::Arithmetic)
    }

    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<(), Error> {
        let mut pending = Some(consume);
        let mut execute = |budget: &mut Budget<'work>| {
            budget.check_prior_denials_v1()?;
            source.require_kernel_argument_abi_v18(
                ProductionKernelArgumentAbiInputV18 { roots },
                budget,
            )?;
            let (launches, width) = context.launches(source, budget)?;
            let (output, (), receipt) = source
                .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                    original.with_optimized_formal_reports_v19(
                        optimized,
                        &launches,
                        width,
                        ControlFlowLimits::DEFAULT,
                        budget,
                        pending
                            .take()
                            .expect("single original-source report consumer"),
                    )?;
                    Ok::<_, ReportsError<Error>>(((), 0))
                })
                .map_err(|error| Error::FormalReports(Box::new(error)))?;
            // The actual output was consumed only while original/optimized
            // correspondences coexisted. No output or report escapes this call.
            drop((output, receipt, launches));
            source.check_original_source(source.source_ssa(budget)?, budget)?;
            budget.check_prior_denials_v1()?;
            Ok(())
        };
        let headers = std::mem::size_of_val(&execute)
            .checked_mul(2)
            .and_then(|value| value.checked_add(std::mem::align_of_val(&execute)))
            .ok_or(Resource::Arithmetic);
        let result = match headers.and_then(|headers| {
            budget.check_prior_denials_v1()?;
            budget.charge_work(headers)?;
            budget.reserve_storage(headers)
        }) {
            Ok(()) => catch_unwind(AssertUnwindSafe(|| execute(budget))),
            Err(error) => Ok(Err(Error::Resource(error))),
        };
        // Trusted compiler callback/destructor cleanup, not a sandbox for
        // adversarial user callbacks. Preserve selected errors across teardown.
        discard(pending);
        match result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(())) => {
                budget.check_prior_denials_v1()?;
                Ok(())
            }
            Err(payload) => {
                discard(payload);
                Err(Error::Unsupported(
                    "original-source report consumer panicked",
                ))
            }
        }
    }
}

pub(super) fn discard<T>(value: T) {
    if let Err(mut payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
        while let Err(next) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            payload = next;
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Observes complete original/output formal reports under real compiler
    /// bindings. This explicit continuation neither clears report reasons nor
    /// activates final memory, target, publication, or default compilation gates.
    pub(crate) fn with_original_source_formal_reports_v19<F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<()>, Error>
    where
        F: for<'before, 'input, 'after, 'output, 'work> FnMut(
            &Report<'before, 'input>,
            &Report<'after, 'output>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    {
        self.with_source_owned_custody_policy_v29::<OriginalFormalReportsV19, (), F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }
}

#[cfg(test)]
#[path = "production_pipeline_source_formal_context_v19_tests.rs"]
mod tests;
