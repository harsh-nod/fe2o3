//! Complete unchanged-or-permuted occurrence rows; no value-equality matching.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockSegmentV1 as Segment, CanonicalKirBlockTransitionV1 as BlockRow,
    CanonicalKirDefinitionDescendantKindV1, CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirDefinitionTransitionV1 as DefinitionRow,
    CanonicalKirEdgeArgumentTransitionV1 as EdgeArgumentRow,
    CanonicalKirEdgeTransitionV1 as EdgeRow, CanonicalKirFunctionTransitionV1 as FunctionRow,
    CanonicalKirOperationOriginV1, CanonicalKirOperationTransitionV1 as OperationRow,
    CanonicalKirTransitionCandidateV1 as Candidate, CanonicalKirTransitionRangeV1 as Range,
    CanonicalKirUseCoordinateV1 as Use, CanonicalKirUseTransitionV1 as UseRow,
};

pub(super) struct Rows {
    functions: Vec<FunctionRow>,
    blocks: Vec<BlockRow>,
    segments: Vec<Segment>,
    operations: Vec<OperationRow>,
    definitions: Vec<DefinitionRow>,
    definition_outputs: Vec<Descendant>,
    uses: Vec<UseRow>,
    edges: Vec<EdgeRow>,
    edge_arguments: Vec<EdgeArgumentRow>,
}
impl Rows {
    pub(super) fn new(
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        region: U32LocalOrderRegionV1,
        permutation: &Permutation,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(8)?;
        if input.functions().len() != output.functions().len()
            || input.blocks().len() != output.blocks().len()
            || input.operations().len() != output.operations().len()
            || input.definitions().len() != output.definitions().len()
            || input.uses().len() != output.uses().len()
            || input.edges().len() != output.edges().len()
            || input.edge_arguments().len() != output.edge_arguments().len()
        {
            return Err(Error::InconsistentOwner);
        }
        budget.reserve_storage(size_of::<Self>())?;
        let mut rows = Self {
            functions: allocate(input.functions().len(), budget)?,
            blocks: allocate(input.blocks().len(), budget)?,
            segments: allocate(input.blocks().len(), budget)?,
            operations: allocate(input.operations().len(), budget)?,
            definitions: allocate(input.definitions().len(), budget)?,
            definition_outputs: allocate(input.definitions().len(), budget)?,
            uses: allocate(input.uses().len(), budget)?,
            edges: allocate(input.edges().len(), budget)?,
            edge_arguments: allocate(input.edge_arguments().len(), budget)?,
        };
        for row in input.functions() {
            budget.charge_work(1)?;
            append(
                &mut rows.functions,
                FunctionRow {
                    input: row.coordinate,
                    output: row.coordinate,
                },
            )?;
        }
        for (index, row) in input.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            append(
                &mut rows.blocks,
                BlockRow {
                    output: row.coordinate,
                    segments: range(index)?,
                },
            )?;
            append(
                &mut rows.segments,
                Segment {
                    input: row.coordinate,
                    connector: None,
                },
            )?;
        }
        for row in output.operations() {
            budget.charge_work(1)?;
            append(
                &mut rows.operations,
                OperationRow {
                    output: row.coordinate,
                    origin: CanonicalKirOperationOriginV1::Retained(map_operation(
                        row.coordinate,
                        region,
                        &permutation.source_for_output,
                    )),
                },
            )?;
        }
        for (index, row) in input.definitions().iter().enumerate() {
            budget.charge_work(2)?;
            let target = match row.coordinate {
                Definition::Result { operation, result } => Definition::Result {
                    operation: map_operation(operation, region, &permutation.output_for_source),
                    result,
                },
                other => other,
            };
            append(
                &mut rows.definitions,
                DefinitionRow {
                    input: row.coordinate,
                    outputs: range(index)?,
                },
            )?;
            append(
                &mut rows.definition_outputs,
                Descendant {
                    output: target,
                    kind: CanonicalKirDefinitionDescendantKindV1::Retained,
                },
            )?;
        }
        for row in output.uses() {
            budget.charge_work(1)?;
            let source = match row.coordinate {
                Use::OperationOperand { operation, operand } => Use::OperationOperand {
                    operation: map_operation(operation, region, &permutation.source_for_output),
                    operand,
                },
                other => other,
            };
            append(
                &mut rows.uses,
                UseRow {
                    output: row.coordinate,
                    input: source,
                },
            )?;
        }
        for row in input.edges() {
            budget.charge_work(1)?;
            append(
                &mut rows.edges,
                EdgeRow {
                    input: row.coordinate,
                    output: row.coordinate,
                },
            )?;
        }
        for row in input.edge_arguments() {
            budget.charge_work(1)?;
            append(
                &mut rows.edge_arguments,
                EdgeArgumentRow {
                    input: row.coordinate,
                    output: row.coordinate,
                },
            )?;
        }
        Ok(rows)
    }
    pub(super) fn candidate(&self) -> Candidate<'_> {
        Candidate {
            functions: &self.functions,
            blocks: &self.blocks,
            segments: &self.segments,
            operations: &self.operations,
            definitions: &self.definitions,
            definition_outputs: &self.definition_outputs,
            uses: &self.uses,
            edges: &self.edges,
            edge_arguments: &self.edge_arguments,
        }
    }
}
fn range(start: usize) -> Result<Range> {
    Ok(Range {
        start: u32::try_from(start).map_err(|_| Resource::Arithmetic)?,
        len: 1,
    })
}
fn allocate<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.charge_work(1)?;
    budget.reserve_storage(
        count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    // Match the workspace's logical-payload convention; physical allocator
    // rounding is not a controlled allocation and is not reported as RSS.
    Ok(result)
}
fn append<T>(rows: &mut Vec<T>, value: T) -> Result<()> {
    if rows.len() == rows.capacity() {
        return Err(Error::InconsistentOwner);
    }
    rows.push(value);
    Ok(())
}
