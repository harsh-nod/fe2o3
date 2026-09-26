//! Private borrowed occurrence core; neither adapter constructs authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirOperationTransitionV1 as Transition,
    ConditionalTotalViewReadV1 as Read, FunctionOperationLocation as Location, Module, Operation,
};
use fe2o3_kernel_opt::ReplayedPolicy6SemanticRelationV1 as ReplayedPrefix;
use fe2o3_pliron::{
    KirBridgeCoordinateV1 as Coordinate, KirOptimizationEndpointV12 as Endpoint,
    KirOptimizationRelationV12 as Relation, ProductionConditionalRuntimePremiseV1 as Premise,
};

// Both implementations borrow sealed checks. This trait is private to lower;
// there is no public route for supplying an unchecked relation implementation.
pub(super) trait CheckedPrefix {
    fn input_bytes(&self) -> &[u8];
    fn output(&self) -> &Graph;
    fn forwarding_stages(&self) -> (&Graph, &Graph, &Graph);
    fn map_input(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate>;
    fn map_output(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate>;
}

impl CheckedPrefix for Prefix {
    fn input_bytes(&self) -> &[u8] {
        self.native_input_audit_bytes()
    }
    fn output(&self) -> &Graph {
        self.owner()
    }
    fn forwarding_stages(&self) -> (&Graph, &Graph, &Graph) {
        let p5 = self.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        (p4.intermediate_policy3().owner(), p4.owner(), p5.owner())
    }
    fn map_input(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate> {
        let p3 = self
            .intermediate_policy5()
            .intermediate_policy4()
            .intermediate_policy3();
        mapped(input, p3.map().relations(), |r| p3.map().targets(r), budget)
    }
    fn map_output(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate> {
        let map = self.continuation().map();
        mapped(input, map.relations(), |r| map.targets(r), budget)
    }
}

impl CheckedPrefix for ReplayedPrefix<'_> {
    fn input_bytes(&self) -> &[u8] {
        self.policy5_relation()
            .policy4_relation()
            .input()
            .canonical()
            .canonical_bytes()
    }
    fn output(&self) -> &Graph {
        self.continuation().output()
    }
    fn forwarding_stages(&self) -> (&Graph, &Graph, &Graph) {
        let p5 = self.policy5_relation();
        let p4 = p5.policy4_relation();
        (p4.intermediate(), p4.output(), p5.output())
    }
    fn map_input(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate> {
        retained(
            input,
            self.policy5_relation()
                .policy4_relation()
                .policy3_relation()
                .semantic_receipt()
                .receipt()
                .candidate()
                .operations,
            budget,
        )
    }
    fn map_output(&self, input: Coordinate, budget: &mut Budget<'_>) -> Result<Coordinate> {
        retained(
            input,
            self.continuation().receipt().candidate().operations,
            budget,
        )
    }
}

fn retained(input: Coordinate, rows: &[Transition], budget: &mut Budget<'_>) -> Result<Coordinate> {
    let mut found = None;
    for row in rows {
        budget.charge_work(4)?;
        let Origin::Retained(from) = row.origin else {
            continue;
        };
        let from = Coordinate::Operation {
            function: from.block.function.0,
            block: from.block.block,
            operation: from.operation,
        };
        if from != input {
            continue;
        }
        let to = Coordinate::Operation {
            function: row.output.block.function.0,
            block: row.output.block.block,
            operation: row.output.operation,
        };
        if found.replace(to).is_some() {
            return Err(Error::Mismatch("unique surviving memory occurrence"));
        }
    }
    found.ok_or(Error::Mismatch("missing memory occurrence"))
}

pub(super) fn check(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    before: &Facts<'_>,
    after: &Facts<'_>,
    prefix: &impl CheckedPrefix,
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
    prefix: &impl CheckedPrefix,
    original: &[Read],
    final_reads: &[Read],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(
        prefix
            .input_bytes()
            .len()
            .checked_add(prefix.output().canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !std::ptr::eq(after.module(), prefix.output().module()) {
        return Err(Error::Mismatch("source/output coverage subjects"));
    }
    coverage_subjects(before, after, original, final_reads)?;
    same_occurrence(
        before,
        before.store_location(),
        after,
        after.store_location(),
        prefix,
        budget,
    )?;
    for (a, b) in original.iter().zip(final_reads) {
        read_premises(*a, *b, budget)?;
        same_occurrence(before, a.location(), after, b.location(), prefix, budget)?;
    }
    Ok(())
}

// Callers prepay structural comparisons against the two actual graph sizes.
pub(super) fn coverage_subjects(
    before: &Facts<'_>,
    after: &Facts<'_>,
    original: &[Read],
    final_reads: &[Read],
) -> Result<()> {
    if before.read_count() != after.read_count()
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
    Ok(())
}

pub(super) fn read_premises(a: Read, b: Read, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(64)?;
    if a.parameter() != b.parameter()
        || a.access_domain() != b.access_domain()
        || a.address_domain() != b.address_domain()
        || a.element_bytes() != b.element_bytes()
        || a.alignment() != b.alignment()
    {
        return Err(Error::Mismatch("ordered read/source/domain agreement"));
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

pub(super) fn coordinate(
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

pub(super) fn operation(module: &Module, coordinate: Coordinate) -> Result<&Operation> {
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
    prefix: &impl CheckedPrefix,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let (c_graph, s_graph, o_graph) = prefix.forwarding_stages();
    let a = coordinate(before, a, budget)?;
    let c = prefix.map_input(a, budget)?;
    // C/S/O forwarding preserves coordinates. Compare the actual memory
    // operation too: a replaced private load is not a surviving read.
    budget.charge_work(
        c_graph
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(s_graph.canonical().canonical_bytes().len())
            .and_then(|n| n.checked_add(o_graph.canonical().canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if operation(c_graph.module(), c)? != operation(s_graph.module(), c)?
        || operation(s_graph.module(), c)? != operation(o_graph.module(), c)?
    {
        return Err(Error::Mismatch("forwarded memory occurrence changed"));
    }
    let i = prefix.map_output(c, budget)?;
    if i != coordinate(after, b, budget)? {
        return Err(Error::Mismatch("final memory occurrence"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_conditional_checked_relations_v1_tests.rs"]
mod tests;
