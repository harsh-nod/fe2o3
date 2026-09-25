//! Direct policy6 N-to-I agreement, not final-F or ordinary formal evidence.
use super::{
    ProductionConditionalContinuationErrorV1, ProductionSourceBoundConditionalAggregateRequestV1,
};
use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewErrorV1,
    ConditionalTotalViewFactsV1 as Facts, VerifiedCanonicalKernelIrModuleV12 as Graph,
    derive_conditional_total_view_from_verified_v1,
};
use fe2o3_kernel_opt::{
    CanonicalPolicy6OptimizationErrorV1, CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix,
};
use fe2o3_pliron::ProductionConditionalAggregateErrorV1;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

/// Refusal while joining a live conditional source request to policy6 output.
#[derive(Debug)]
pub enum ProductionConditionalCheckedOutputErrorV1 {
    /// An original phase account could not pay for the check or its scratch.
    Resource(Resource),
    /// The sealed optimization prefix did not replay against the bound input.
    Prefix(CanonicalPolicy6OptimizationErrorV1),
    /// Conditional coverage could not be derived from the actual graph.
    Coverage(ConditionalTotalViewErrorV1),
    /// Source arguments or output correspondence did not match the request.
    Source(ProductionConditionalContinuationErrorV1),
    /// The borrowed ranked graph no longer matched its original owner.
    Graph(ProductionConditionalAggregateErrorV1),
    /// A required identity, occurrence or runtime premise differed.
    Mismatch(&'static str),
}
type Error = ProductionConditionalCheckedOutputErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional N-to-I agreement: {self:?}")
    }
}
impl std::error::Error for Error {}

const SCRATCH: usize = 4 * size_of::<Facts<'static>>() + size_of::<Error>() + 1024;

impl ProductionSourceBoundConditionalAggregateRequestV1<'_> {
    /// Checks the actual sealed B/C/S/O/I against this live source request.
    /// The coordinate receipt must borrow this exact N. The backend separately
    /// checks its exact target metadata, genuine proof and retained contract.
    /// Source/arena/proof remain paid on their ORIGINAL source account; B and
    /// the entire prefix remain paid on their distinct ORIGINAL target account.
    /// Neither account is reconstructed or transferred by this call. Added
    /// scratch is released on return/unwind, never work or refusal history.
    ///
    /// No graph is copied and no evidence owner or authority is returned. I is
    /// not native history F; the ordinary conditional-finalizer gate stays shut.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionSourceBoundConditionalAggregateRequestV1 as Request,
    ///     ProductionFormalMemoryOwnerV1};
    /// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
    /// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn promote(request: &Request<'_>, coordinates: &Coordinates<'_, '_>, prefix: &Prefix,
    ///     target: &mut Budget<'_>, source: &mut Budget<'_>) -> ProductionFormalMemoryOwnerV1 {
    ///     request.check_policy6_output_v1(coordinates, prefix, target, source).unwrap()
    /// }
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn check_policy6_output_v1(
        &self,
        coordinates: &Coordinates<'_, '_>,
        checked: &Prefix,
        target: &mut Budget<'_>,
        source: &mut Budget<'_>,
    ) -> Result<()> {
        scoped(source, |source| {
            scoped(target, |target| {
                source.charge_work(4)?;
                if source.work_ledger_identity_v1() == target.work_ledger_identity_v1()
                    || !std::ptr::eq(self.source().executable(), coordinates.input())
                {
                    return Err(Error::Mismatch(
                        "original source and distinct phase accounts",
                    ));
                }
                if source.storage() < self.source().retained_analysis_storage_v1()
                    || target.storage()
                        < checked
                            .retained_storage()
                            .checked_add(coordinates.output().canonical().canonical_bytes().len())
                            .ok_or(Resource::Arithmetic)?
                {
                    return Err(Resource::Accounting.into());
                }
                source.reserve_storage(SCRATCH)?;
                target.reserve_storage(SCRATCH)?;
                self.pliron_input()
                    .require_current_graph_v1(source)
                    .map_err(Error::Graph)?;
                checked
                    .replay(coordinates.output(), target)
                    .map_err(Error::Prefix)?;
                let kernel = source_kernel(self, source)?;
                let original = facts(coordinates.input(), kernel, source)?;
                let binding = self
                    .source()
                    .bind_conditional_output_v1(original, source)
                    .map_err(|e| {
                        Error::Source(ProductionConditionalContinuationErrorV1::Binding(e))
                    })?;
                let output = facts(checked.owner(), kernel, target)?;
                super::require_conditional_argument_rows_v1(
                    self.arguments(),
                    self.pliron_input(),
                    source,
                )
                .map_err(Error::Source)?;
                occurrences::check(self, binding.coverage(), &output, checked, source)?;
                self.pliron_input()
                    .require_current_graph_v1(source)
                    .map_err(Error::Graph)
            })
        })
    }
}

fn source_kernel<'a>(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<&'a fe2o3_kernel_ir::KernelId> {
    let source = request.source();
    let semantic = source.semantic_ssa.source_semantic();
    let name = request.pliron_input().kernel().function_name();
    let mut selected = None;
    for row in source.correspondence.lowered_functions() {
        budget.charge_work(3)?;
        if row.role() != super::SemanticKirFunctionRoleV1::KernelEntry {
            continue;
        }
        let declaration = semantic
            .functions()
            .get(row.correspondence_owner().index() as usize)
            .and_then(|function| function.kernel_entry())
            .ok_or(Error::Mismatch("source kernel declaration"))?;
        let symbol = declaration.export_symbol().as_bytes();
        budget.charge_work(
            symbol
                .len()
                .checked_add(name.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if symbol == name.as_bytes() && selected.replace(row).is_some() {
            return Err(Error::Mismatch("unique source kernel correspondence"));
        }
    }
    let selected = selected.ok_or(Error::Mismatch("source kernel correspondence"))?;
    let mut kernel = None;
    for row in &source.executable().module().kernels {
        budget.charge_work(
            row.entry
                .as_str()
                .len()
                .checked_add(selected.kernel_ir_function().as_str().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if &row.entry == selected.kernel_ir_function() && kernel.replace(&row.id).is_some() {
            return Err(Error::Mismatch("unique canonical source kernel"));
        }
    }
    kernel.ok_or(Error::Mismatch("canonical source kernel"))
}

fn facts<'a>(
    owner: &'a Graph,
    kernel: &fe2o3_kernel_ir::KernelId,
    budget: &mut Budget<'_>,
) -> Result<Facts<'a>> {
    match derive_conditional_total_view_from_verified_v1(
        owner.verified_module_ref_v1(),
        kernel,
        budget,
    )
    .map_err(Error::Coverage)?
    {
        fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Established(facts) => Ok(facts),
        fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Unsupported(_) => {
            Err(Error::Mismatch("conditional canonical coverage"))
        }
    }
}

fn scoped<T>(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    let floor = budget.storage();
    let account = budget.work_ledger_identity_v1();
    let address = std::ptr::from_mut(budget);
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup =
        if std::ptr::from_mut(budget) != address || budget.work_ledger_identity_v1() != account {
            Err(Resource::Accounting)
        } else {
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
                .and_then(|delta| budget.release_storage(delta))
        };
    match result {
        Ok(value) => {
            cleanup?;
            value
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

mod occurrences {
    use super::*;
    use fe2o3_kernel_ir::{
        ConditionalTotalViewReadV1 as Read, FunctionOperationLocation as Location, Module,
        Operation,
    };
    use fe2o3_pliron::{
        KirBridgeCoordinateV1 as Coordinate, KirOptimizationEndpointV12 as Endpoint,
        KirOptimizationRelationV12 as Relation, ProductionConditionalRuntimePremiseV1 as Premise,
    };

    pub(super) fn check(
        request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
        before: &Facts<'_>,
        after: &Facts<'_>,
        prefix: &Prefix,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let input = request.pliron_input();
        let [output] = input.outputs() else {
            return Err(Error::Mismatch("one source output"));
        };
        let original = reads(before, budget)?;
        let final_reads = reads(after, budget)?;
        coverage(before, after, prefix, &original, &final_reads, budget)?;
        let parameter = before.output_parameter_index();
        budget.charge_work(32)?;
        if output.canonical_parameter() != parameter
            || input.canonical_output_store_location_v1() != before.store_location()
            || before.read_count() != input.reads().len()
        {
            return Err(Error::Mismatch("source/output coverage subjects"));
        }
        for (a, claimed) in original.iter().zip(input.reads()) {
            budget.charge_work(size_of::<Read>())?;
            if *a != claimed.canonical() {
                return Err(Error::Mismatch("ordered source read claims"));
            }
        }
        premises(after, &final_reads, input.premises(), budget)
    }

    pub(super) fn coverage(
        before: &Facts<'_>,
        after: &Facts<'_>,
        prefix: &Prefix,
        original: &[Read],
        final_reads: &[Read],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(
            prefix
                .native_input_audit_bytes()
                .len()
                .checked_add(prefix.owner().canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if !std::ptr::eq(after.module(), prefix.owner().module())
            || before.read_count() != after.read_count()
            || original.len() != before.read_count()
            || final_reads.len() != after.read_count()
            || before.output_parameter_index() != after.output_parameter_index()
            || before.address_domain() != after.address_domain()
            || before.element_bytes() != after.element_bytes()
            || before.alignment() != after.alignment()
            || before.function().signature != after.function().signature
        {
            return Err(Error::Mismatch("source/output coverage subjects"));
        }
        same_occurrence(
            before,
            before.store_location(),
            after,
            after.store_location(),
            prefix,
            budget,
        )?;
        for (a, b) in original.iter().zip(final_reads) {
            budget.charge_work(64)?;
            if a.parameter() != b.parameter()
                || a.access_domain() != b.access_domain()
                || a.address_domain() != b.address_domain()
                || a.element_bytes() != b.element_bytes()
                || a.alignment() != b.alignment()
            {
                return Err(Error::Mismatch("ordered read/source/domain agreement"));
            }
            same_occurrence(before, a.location(), after, b.location(), prefix, budget)?;
        }
        Ok(())
    }

    pub(super) fn premises(
        after: &Facts<'_>,
        final_reads: &[Read],
        actual: &[Premise],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let parameter = after.output_parameter_index();
        budget.charge_work(4 * size_of::<Premise>())?;
        let header = [
            Premise::D1Launch,
            Premise::OutputWithinGlobalX { parameter },
            Premise::WritableOutput { parameter },
            Premise::RepresentableAddress {
                parameter,
                domain: after.address_domain(),
                element_bytes: after.element_bytes(),
                alignment: after.alignment(),
            },
        ];
        if actual.len()
            != 4usize
                .checked_add(
                    final_reads
                        .len()
                        .checked_mul(3)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?
            || actual[..4] != header
        {
            return Err(Error::Mismatch("complete output premises"));
        }
        for (index, b) in final_reads.iter().enumerate() {
            budget.charge_work(3 * size_of::<Premise>())?;
            let expected = [
                Premise::ReadableInput {
                    parameter: b.parameter(),
                    domain: b.access_domain(),
                },
                Premise::SeparateInputOutput {
                    input: b.parameter(),
                    output: parameter,
                },
                Premise::RepresentableAddress {
                    parameter: b.parameter(),
                    domain: b.address_domain(),
                    element_bytes: b.element_bytes(),
                    alignment: b.alignment(),
                },
            ];
            if actual[4 + 3 * index..7 + 3 * index] != expected {
                return Err(Error::Mismatch("ordered read premises"));
            }
        }
        Ok(())
    }

    pub(super) fn reads(facts: &Facts<'_>, budget: &mut Budget<'_>) -> Result<Vec<Read>> {
        let bytes = facts
            .read_count()
            .checked_mul(size_of::<Read>())
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(bytes)?;
        budget.reserve_storage(
            bytes
                .checked_add(size_of::<Vec<Read>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut reads = Vec::new();
        reads
            .try_reserve_exact(facts.read_count())
            .map_err(|_| Resource::Allocation)?;
        if reads.capacity() != facts.read_count() {
            return Err(Resource::Accounting.into());
        }
        facts
            .visit_reads_v1(budget, |read| {
                if reads.len() == reads.capacity() {
                    return Err(Resource::Accounting);
                }
                reads.push(read);
                Ok(())
            })
            .map_err(Error::Coverage)?;
        if reads.len() != facts.read_count() {
            return Err(Resource::Accounting.into());
        }
        Ok(reads)
    }

    fn coordinate(
        facts: &Facts<'_>,
        location: Location,
        budget: &mut Budget<'_>,
    ) -> Result<Coordinate> {
        let body = facts
            .function()
            .body
            .as_ref()
            .ok_or(Error::Mismatch("conditional function body"))?;
        for (block, row) in body.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            if row.id == location.block {
                return Ok(Coordinate::Operation {
                    function: u32::try_from(facts.function_ordinal())
                        .map_err(|_| Resource::Arithmetic)?,
                    block: u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
                    operation: u32::try_from(location.operation_index)
                        .map_err(|_| Resource::Arithmetic)?,
                });
            }
        }
        Err(Error::Mismatch("canonical block coordinate"))
    }

    fn operation(module: &Module, coordinate: Coordinate) -> Result<&Operation> {
        let Coordinate::Operation {
            function,
            block,
            operation,
        } = coordinate
        else {
            return Err(Error::Mismatch("operation coordinate"));
        };
        module
            .functions
            .get(function as usize)
            .and_then(|f| f.body.as_ref())
            .and_then(|b| b.blocks.get(block as usize))
            .and_then(|b| b.operations.get(operation as usize))
            .ok_or(Error::Mismatch("operation range"))
    }

    fn mapped<'a>(
        coordinate: Coordinate,
        rows: &'a [Relation],
        targets: impl Fn(&'a Relation) -> Option<&'a [Endpoint]>,
        budget: &mut Budget<'_>,
    ) -> Result<Coordinate> {
        let mut found = None;
        for row in rows {
            budget.charge_work(2)?;
            if row.source() != coordinate {
                continue;
            }
            let Some([Endpoint::Operation(target @ Coordinate::Operation { .. })]) = targets(row)
            else {
                return Err(Error::Mismatch("unique surviving memory occurrence"));
            };
            if !row.identity_survived() || found.replace(*target).is_some() {
                return Err(Error::Mismatch("surviving memory occurrence"));
            }
        }
        found.ok_or(Error::Mismatch("missing memory occurrence"))
    }

    fn same_occurrence(
        before: &Facts<'_>,
        a: Location,
        after: &Facts<'_>,
        b: Location,
        prefix: &Prefix,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let p5 = prefix.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        let p3 = p4.intermediate_policy3();
        let a = coordinate(before, a, budget)?;
        let c = mapped(a, p3.map().relations(), |r| p3.map().targets(r), budget)?;
        // C/S/O forwarding preserves coordinates. Compare the actual memory
        // operation too: a replaced private load is not a surviving read.
        budget.charge_work(
            p3.owner()
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(p4.owner().canonical().canonical_bytes().len())
                .and_then(|n| n.checked_add(p5.owner().canonical().canonical_bytes().len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if operation(p3.owner().module(), c)? != operation(p4.owner().module(), c)?
            || operation(p4.owner().module(), c)? != operation(p5.owner().module(), c)?
        {
            return Err(Error::Mismatch("forwarded memory occurrence changed"));
        }
        let map = prefix.continuation().map();
        let i = mapped(c, map.relations(), |r| map.targets(r), budget)?;
        if i != coordinate(after, b, budget)? {
            return Err(Error::Mismatch("final memory occurrence"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "production_conditional_checked_output_v1_tests.rs"]
mod tests;
