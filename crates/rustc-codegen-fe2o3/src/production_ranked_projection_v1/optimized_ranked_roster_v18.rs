//! Lexical root custody for the actual lifecycle-checked V18 output.
//! This is not the legacy ranked owner or a final ranked-equivalence receipt.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirFunctionCoordinateV1 as Function,
};
use fe2o3_lower_mir_kernel::{
    ProductionLifecycleCheckedNativePoliciesV18 as Native,
    ProductionOptimizedSourceCorrespondenceV18 as Optimized,
    ProductionSourceCorrespondenceV18 as Original,
    ProductionSourceNativeLifecycleErrorV18 as Error,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};
use source_ranked_consumer_resources_v18 as resources;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RootRow {
    original: SemanticFunctionIdV1,
    input: Function,
    output: Function,
    input_kernel: usize,
    output_kernel: usize,
    reference: Option<usize>,
}

/// Private attachment to actual native reports, original roots and bindings.
/// No field supplies final ranked equivalence, formal or target authority.
/// Logical names and reference registrations retain the private prepared
/// materialization's custody; this is not a new reference-effect proof.
pub(crate) struct SourceNativeRankedRootRosterV18<'a, 'g, 'scope, 'owner> {
    original: &'a Original<'g>,
    optimized: &'a Optimized<'g>,
    native: &'a Native<'scope, 'owner>,
    inputs: &'a [ProductionRankedRootInputV1],
    references: &'a crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    rows: &'a [RootRow],
    required: usize,
}

fn refusal(detail: &'static str) -> Error {
    Error::Source(SourceError::Binding(detail))
}

fn retain<T>(native: &Native<'_, '_>, result: Result<T, Error>) -> Result<T, Error> {
    result.map_err(|error| match error {
        Error::Source(SourceError::Binding(detail)) => {
            native.retain_source_binding_error_v18(detail)
        }
        other => other,
    })
}

fn resource(original: &Original<'_>, error: Resource) -> Error {
    Error::Source(original.retain_query_resource_error_v18(error))
}

fn projection(original: &Original<'_>, error: ProductionRankedProjectionErrorV1) -> Error {
    match error {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::SourceOwned(SourceError::Resource(error)),
        ) => resource(original, error),
        _ => refusal("native root reference roster is incomplete or substituted"),
    }
}

fn checked_subject(
    original: &Original<'_>,
    optimized: &Optimized<'_>,
    native: &Native<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    native.check_source_subject_v18(original, optimized, budget)?;
    original.check_query_v18(budget)?;
    Ok(())
}

fn kernel_index(
    original: &Original<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<Vec<Option<usize>>, Error> {
    let mut rows = resources::rows(inventory.functions().len(), budget)
        .map_err(|error| projection(original, error))?;
    rows.resize(inventory.functions().len(), None);
    for (ordinal, kernel) in inventory.kernels().iter().enumerate() {
        budget
            .charge_work(3)
            .map_err(|error| resource(original, error))?;
        let row = rows
            .get_mut(kernel.entry.0 as usize)
            .ok_or_else(|| refusal("native root kernel has an absent actual function"))?;
        if kernel.ordinal as usize != ordinal || row.replace(ordinal).is_some() {
            return Err(refusal("native root kernel repeats its actual entry"));
        }
    }
    Ok(rows)
}

fn reference_partitions(
    original: &Original<'_>,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut Budget<'_>,
) -> Result<Vec<Vec<usize>>, Error> {
    let result = (|| {
        let mut allocation = resources::SourceBindingAllocationV18::new(Some(budget))?;
        let mut roots = allocation.rows(inputs.len())?;
        for root in inputs {
            allocation.charge(1)?;
            resources::push(&mut roots, root.logical_name.as_str())?;
        }
        let mut names = allocation.rows(references.as_slice().len())?;
        for reference in references.as_slice() {
            allocation.charge(1)?;
            resources::push(&mut names, reference.logical_kernel_name.as_str())?;
        }
        partition_reference_effect_binding_indices_v18(&roots, &names, &mut allocation)
    })();
    result.map_err(|error| projection(original, error))
}

fn prepay_kernel_declaration_comparison(
    before: &fe2o3_kernel_ir::Kernel,
    after: &fe2o3_kernel_ir::Kernel,
    budget: &mut Budget<'_>,
) -> Result<(), Resource> {
    use fe2o3_kernel_ir::TargetCapability as Capability;

    let rows = before
        .required_capabilities
        .len()
        .checked_add(after.required_capabilities.len())
        .ok_or(Resource::Arithmetic)?;
    // Pay the roster walk before inspecting tags or string lengths. As in the
    // source module comparator, 16 covers each capability's fixed fields; both
    // Extension strings are additional work before derived Kernel equality.
    budget.charge_work(rows)?;
    let mut work = before
        .id
        .as_str()
        .len()
        .checked_add(after.id.as_str().len())
        .and_then(|work| work.checked_add(before.entry.as_str().len()))
        .and_then(|work| work.checked_add(after.entry.as_str().len()))
        // The following equality block also joins both entries to function IDs.
        .and_then(|work| work.checked_add(before.entry.as_str().len()))
        .and_then(|work| work.checked_add(after.entry.as_str().len()))
        .ok_or(Resource::Arithmetic)?;
    for capability in before
        .required_capabilities
        .iter()
        .chain(&after.required_capabilities)
    {
        let width = match capability {
            Capability::Extension { namespace, name } => 16usize
                .checked_add(namespace.len())
                .and_then(|width| width.checked_add(name.len())),
            Capability::Float16
            | Capability::BFloat16
            | Capability::Float64
            | Capability::Int64
            | Capability::Subgroups
            | Capability::SubgroupSize(_)
            | Capability::WorkgroupMemory
            | Capability::WorkgroupBarrier
            | Capability::Atomic { .. }
            | Capability::DynamicWorkgroupMemory
            | Capability::WaveWidth(_) => Some(16),
        }
        .ok_or(Resource::Arithmetic)?;
        work = work.checked_add(width).ok_or(Resource::Arithmetic)?;
    }
    budget.charge_work(work)
}

fn build_rows(
    original: &Original<'_>,
    optimized: &Optimized<'_>,
    native: &Native<'_, '_>,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut Budget<'_>,
) -> Result<Vec<RootRow>, Error> {
    checked_subject(original, optimized, native, budget)?;
    let source = original.source(budget)?;
    let semantic = source.source_semantic(budget)?;
    let launch = source.source_launch(budget)?;
    let input = optimized.input_inventory(budget)?;
    let output = optimized.output_inventory(budget)?;
    budget
        .charge_work(7)
        .map_err(|error| resource(original, error))?;
    if inputs.is_empty()
        || inputs.len() != source.root_count(budget)?
        || inputs.len() != launch.roots().len()
        || inputs.len() != input.kernels().len()
        || inputs.len() != output.kernels().len()
        || launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
        || native.function_count(budget)? != output.functions().len()
    {
        return Err(refusal(
            "native root attachment has an incomplete root roster",
        ));
    }
    let references = reference_partitions(original, inputs, references, budget)?;
    let mut input_kernels = kernel_index(original, input, budget)?;
    let mut output_kernels = kernel_index(original, output, budget)?;
    let mut rows =
        resources::rows(inputs.len(), budget).map_err(|error| projection(original, error))?;
    for (ordinal, (root, source_launch)) in inputs.iter().zip(launch.roots()).enumerate() {
        budget
            .charge_work(12)
            .map_err(|error| resource(original, error))?;
        let (identity, input_ordinal) = source.root(ordinal, budget)?;
        let function = semantic
            .functions()
            .get(identity.index() as usize)
            .ok_or_else(|| refusal("native root original declaration is absent"))?;
        let entry = function
            .kernel_entry()
            .ok_or_else(|| refusal("native root original kernel entry is absent"))?;
        if identity != source_launch.selected_root()
            || function.role() != SemanticFunctionRoleV1::KernelRoot
            || function.identity() != source_launch.semantic_root_identity()
            || root.kernel_binding != source_launch.kernel_binding()
            || root.kernel_binding != *entry.kernel_binding_identity().as_bytes()
            || source_launch.source_launch() != source_launch_input_v1(&root.source_launch)
        {
            return Err(refusal(
                "native root attachment changed original root or launch binding",
            ));
        }
        let cfg = optimized.output_root_cfg_v18(ordinal, budget)?;
        let actual = cfg.function();
        if !std::ptr::eq(cfg.inventory(), output)
            || actual.function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            || actual.function.body.is_none()
            || native
                .report(actual.coordinate.0 as usize, budget)?
                .is_none()
        {
            return Err(refusal(
                "native root attachment lacks its actual checked root report",
            ));
        }
        let input_kernel = input_kernels
            .get_mut(input_ordinal)
            .and_then(Option::take)
            .ok_or_else(|| refusal("native root attachment repeats or omits an original kernel"))?;
        let output_kernel = output_kernels
            .get_mut(actual.coordinate.0 as usize)
            .and_then(Option::take)
            .ok_or_else(|| refusal("native root attachment repeats or omits an actual kernel"))?;
        let before = input.kernels()[input_kernel].kernel;
        let after = output.kernels()[output_kernel].kernel;
        prepay_kernel_declaration_comparison(before, after, budget)
            .map_err(|error| resource(original, error))?;
        if before != after
            || before.entry != input.functions()[input_ordinal].function.id
            || after.entry != actual.function.id
        {
            return Err(refusal(
                "native root attachment changed its actual kernel declaration",
            ));
        }
        resources::push(
            &mut rows,
            RootRow {
                original: identity,
                input: input.functions()[input_ordinal].coordinate,
                output: actual.coordinate,
                input_kernel,
                output_kernel,
                reference: references[ordinal].first().copied(),
            },
        )
        .map_err(|error| projection(original, error))?;
    }
    budget
        .charge_work(
            input_kernels
                .len()
                .checked_add(output_kernels.len())
                .ok_or_else(|| resource(original, Resource::Arithmetic))?,
        )
        .map_err(|error| resource(original, error))?;
    if input_kernels
        .iter()
        .chain(&output_kernels)
        .any(Option::is_some)
    {
        return Err(refusal("native root attachment left an unclaimed kernel"));
    }
    checked_subject(original, optimized, native, budget)?;
    Ok(rows)
}

impl SourceNativeRankedRootRosterV18<'_, '_, '_, '_> {
    fn check(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.native
            .check_retained_root_storage_v18(self.required, budget)?;
        checked_subject(self.original, self.optimized, self.native, budget)?;
        Ok(())
    }

    fn check_row(&self, ordinal: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.check(budget)?;
        budget
            .charge_work(10)
            .map_err(|error| resource(self.original, error))?;
        let row = self
            .rows
            .get(ordinal)
            .ok_or_else(|| refusal("native root attachment query is outside its roster"))?;
        let source = self.original.source(budget)?;
        let (identity, input) = source.root(ordinal, budget)?;
        let cfg = self.optimized.output_root_cfg_v18(ordinal, budget)?;
        let original = self.optimized.input_inventory(budget)?;
        let output = self.optimized.output_inventory(budget)?;
        let source_launch = source
            .source_launch(budget)?
            .roots()
            .get(ordinal)
            .ok_or_else(|| refusal("native root attachment lost its source launch row"))?;
        let expected = self
            .inputs
            .get(ordinal)
            .ok_or_else(|| refusal("native root attachment lost its prepared root"))?;
        if identity != row.original
            || row.input.0 as usize != input
            || row.output != cfg.function().coordinate
            || original
                .kernels()
                .get(row.input_kernel)
                .map(|kernel| kernel.entry)
                != Some(row.input)
            || output
                .kernels()
                .get(row.output_kernel)
                .map(|kernel| kernel.entry)
                != Some(row.output)
            || source_launch.selected_root() != row.original
            || source_launch.kernel_binding() != expected.kernel_binding
            || source_launch.source_launch() != source_launch_input_v1(&expected.source_launch)
            || self.native.report(row.output.0 as usize, budget)?.is_none()
        {
            return Err(refusal(
                "native root attachment changed an exact source/output row",
            ));
        }
        if let Some(index) = row.reference {
            let reference =
                self.references.as_slice().get(index).ok_or_else(|| {
                    refusal("native root attachment changed its reference binding")
                })?;
            let work = reference
                .logical_kernel_name
                .len()
                .checked_add(expected.logical_name.len())
                .and_then(|work| work.checked_add(1))
                .ok_or_else(|| resource(self.original, Resource::Arithmetic))?;
            budget
                .charge_work(work)
                .map_err(|error| resource(self.original, error))?;
            if reference.logical_kernel_name != expected.logical_name {
                return Err(refusal(
                    "native root attachment changed its reference binding",
                ));
            }
        }
        Ok(())
    }

    fn check_complete(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.check(budget)?;
        if self.rows.len() != self.inputs.len()
            || self.rows.len() != self.original.source(budget)?.root_count(budget)?
        {
            return Err(refusal(
                "native root attachment changed its complete roster",
            ));
        }
        let mut references = 0usize;
        for ordinal in 0..self.rows.len() {
            self.check_row(ordinal, budget)?;
            references = references
                .checked_add(usize::from(self.rows[ordinal].reference.is_some()))
                .ok_or_else(|| resource(self.original, Resource::Arithmetic))?;
        }
        if references != self.references.as_slice().len() {
            return Err(refusal(
                "native root attachment omitted its reference binding",
            ));
        }
        Ok(())
    }

    pub(crate) fn root_count(&self, budget: &mut Budget<'_>) -> Result<usize, Error> {
        retain(self.native, self.check(budget).map(|()| self.rows.len()))
    }

    pub(crate) fn root(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            SemanticFunctionIdV1,
            Function,
            &ProductionRankedRootInputV1,
            Option<&crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
        ),
        Error,
    > {
        retain(self.native, self.check_row(ordinal, budget))?;
        let row = self
            .rows
            .get(ordinal)
            .ok_or_else(|| refusal("native root attachment query is outside its roster"))?;
        let reference = row
            .reference
            .map(|index| &self.references.as_slice()[index]);
        Ok((row.original, row.output, &self.inputs[ordinal], reference))
    }
}

/// Runs only beneath a successful exact lifecycle-source/native scope. The
/// unit consumer cannot export this borrowed attachment as a production owner.
pub(crate) fn with_source_native_ranked_roots_v18<'g, 'scope, 'owner>(
    original: &Original<'g>,
    optimized: &Optimized<'g>,
    native: &Native<'scope, 'owner>,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut Budget<'_>,
    consume: impl FnOnce(
        &SourceNativeRankedRootRosterV18<'_, 'g, 'scope, 'owner>,
        &mut Budget<'_>,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    retain(
        native,
        with_roots_inner(
            original, optimized, native, inputs, references, budget, consume,
        ),
    )
}

fn with_roots_inner<'g, 'scope, 'owner>(
    original: &Original<'g>,
    optimized: &Optimized<'g>,
    native: &Native<'scope, 'owner>,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut Budget<'_>,
    consume: impl FnOnce(
        &SourceNativeRankedRootRosterV18<'_, 'g, 'scope, 'owner>,
        &mut Budget<'_>,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    checked_subject(original, optimized, native, budget)?;
    let floor = budget.storage();
    // All query headers are reused within this closed lexical attachment.
    let headers = root_headers(
        std::mem::size_of_val(&consume),
        std::mem::align_of_val(&consume),
    )
    .ok_or_else(|| resource(original, Resource::Arithmetic))?;
    budget
        .reserve_storage(headers)
        .map_err(|error| resource(original, error))?;
    let rows = build_rows(original, optimized, native, inputs, references, budget)?;
    let required = budget.storage();
    let view = SourceNativeRankedRootRosterV18 {
        original,
        optimized,
        native,
        inputs,
        references,
        rows: &rows,
        required,
    };
    view.check_complete(budget)?;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[cfg(test)]
        controls::check(&view, budget)?;
        consume(&view, budget)
    }));
    // Retain a returned source error before postflight can observe a later
    // custody failure. Panic identity is resumed only after custody inspection.
    let caught = caught.map(|result| retain(native, result));
    let postflight = if matches!(&caught, Ok(Ok(()))) {
        view.check_complete(budget)
    } else {
        native.check_retained_root_storage_v18(required, budget)
    };
    drop(view);
    drop(rows);
    match caught {
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(Err(error)) => return Err(error),
        Ok(Ok(())) => postflight?,
    }
    // On error or unwind the containing source/native cleanup owns settlement.
    // Success releases only this constructor's concrete, already dropped data.
    budget
        .release_storage(
            required
                .checked_sub(floor)
                .ok_or_else(|| resource(original, Resource::Accounting))?,
        )
        .map_err(|error| resource(original, error))
}

fn root_headers(callback: usize, alignment: usize) -> Option<usize> {
    [
        callback,
        alignment.checked_mul(2)?,
        size_of::<[usize; 2]>(),
        size_of::<SourceNativeRankedRootRosterV18<'_, '_, '_, '_>>(),
        size_of::<Result<Vec<RootRow>, Error>>(),
        size_of::<Result<Vec<RootRow>, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<Vec<Vec<usize>>, Error>>(),
        size_of::<Result<Vec<Vec<usize>>, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<Vec<usize>, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<Vec<&str>, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<Vec<(&str, usize)>, ProductionRankedProjectionErrorV1>>(),
        size_of::<
            Result<
                resources::SourceBindingAllocationV18<'_, '_>,
                ProductionRankedProjectionErrorV1,
            >,
        >(),
        size_of::<Result<Vec<Option<usize>>, Error>>(),
        size_of::<Result<Vec<Option<usize>>, ProductionRankedProjectionErrorV1>>(),
        size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>(),
        size_of::<
            Result<
                fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>,
                SourceError,
            >,
        >(),
        size_of::<
            Result<
                (
                    SemanticFunctionIdV1,
                    Function,
                    &ProductionRankedRootInputV1,
                    Option<&crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
                ),
                Error,
            >,
        >(),
        size_of::<Result<usize, Error>>(),
        size_of::<Result<Option<&fe2o3_pliron::ProductionPlironPreloweringReportV2>, Error>>(),
        6 * size_of::<Result<(), Error>>(),
        size_of::<std::thread::Result<Result<(), Error>>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
}

#[cfg(test)]
#[path = "optimized_ranked_roster_v18_tests.rs"]
mod controls;
#[cfg(test)]
pub(crate) use controls::with_actual_root_controls_v18;
