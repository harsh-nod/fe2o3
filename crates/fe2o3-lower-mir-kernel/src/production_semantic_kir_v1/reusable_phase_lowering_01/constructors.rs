//! Defer only the three original assignments of the checked Owner/Issue
//! constructor. Its normal return emits the capability, not its ZST fields.
use super::runtime::{bind_exact, clone_value, event_value};
use super::*;
use fe2o3_mir_model::{
    SemanticExpandedStatementOriginV1 as Origin, SemanticExpandedTerminatorOriginV1,
};
use fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 as Site;

struct Constructor {
    instance: SemanticCallInstanceIdV1,
    block: SemanticBlockIdV1,
    source_block: SemanticBlockIdV1,
    parameter: SemanticLocalIdV1,
    source_input: SsaValueV1,
    input_type: SemanticTypeIdV1,
    visited: u8,
}

pub(super) struct Constructors {
    values: Vec<Constructor>,
}

fn constructor_count(rows: &[PhaseEmissionRowV1], work: &mut usize) -> PhaseResult<usize> {
    spend(work, rows.len())?;
    Ok(rows
        .iter()
        .filter(|row| {
            matches!(
                row.action,
                PhaseEmissionActionV1::OwnerConvert | PhaseEmissionActionV1::Begin
            )
        })
        .count())
}

impl Constructors {
    pub(super) fn new(checked: &CheckedRows<'_>, work: &mut usize) -> PhaseResult<Self> {
        let count = constructor_count(&checked.input.rows, work)?;
        let mut values = reserve(count, work)?;
        for row in &checked.input.rows {
            spend(work, 1)?;
            let phase = &checked.input.phases[row.phase];
            let (instance, source_input) = match row.action {
                PhaseEmissionActionV1::OwnerConvert => (phase.owner, phase.owner_workgroup),
                PhaseEmissionActionV1::Begin => (phase.issue, phase.owner_reference),
                _ => continue,
            };
            let binding = checked.binding(instance, work)?;
            let Defined::ReusablePhase(record) = binding.contract() else {
                return Err(rejected("phase constructor changed its defined family"));
            };
            let input_type = match record.recipe() {
                Recipe::OwnerConvert { workgroup, .. } => workgroup,
                Recipe::Issue { owner, .. } => owner,
                _ => return Err(rejected("phase constructor changed its exact role")),
            };
            let [parameter] = binding.callee_arguments() else {
                return Err(rejected("phase constructor has no unique actual parameter"));
            };
            let function =
                &checked.owner.source_semantic().functions()[record.function().index() as usize];
            let view = checked
                .owner
                .execution_view_for_root(checked.input.root)
                .unwrap();
            let block = super::calls::mapped_block(view, instance, function.entry(), work)?;
            if function.blocks().len() != 1
                || function.blocks()[0].statements().len() != 3
                || !matches!(
                    function.blocks()[0].terminator().kind(),
                    SemanticTerminatorKindV1::Return
                )
                || view.block_origins()[block.index() as usize].terminator()
                    != (SemanticExpandedTerminatorOriginV1::CallReturn { callee: instance })
            {
                return Err(rejected(
                    "phase constructor lost its exact closed normal-return body",
                ));
            }
            values.push(Constructor {
                instance,
                block,
                source_block: function.entry(),
                parameter: *parameter,
                source_input,
                input_type,
                visited: 0,
            });
        }
        Ok(Self { values })
    }

    pub(super) fn defer(
        &mut self,
        checked: &CheckedRows<'_>,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        site: Site,
        statement: &SemanticStatementKindV1,
        work: &mut usize,
    ) -> PhaseResult<bool> {
        let Some(ordinal) = site.statement() else {
            return Ok(false);
        };
        let view = checked
            .owner
            .execution_view_for_root(checked.input.root)
            .unwrap();
        let origin = &view.block_origins()[site.block().index() as usize];
        let Some(Origin::Source {
            statement: source_ordinal,
        }) = origin.statements().get(ordinal as usize)
        else {
            return Ok(false);
        };
        for selected in &mut self.values {
            spend(work, 1)?;
            if origin.instance() != selected.instance {
                continue;
            }
            if site.block() != selected.block
                || origin.block() != selected.source_block
                || *source_ordinal >= 3
                || *source_ordinal != u32::from(selected.visited)
            {
                return Err(rejected(
                    "phase constructor source assignments changed order or frame",
                ));
            }
            let SemanticStatementKindV1::Assign(assignment) = statement else {
                return Err(rejected("phase constructor changed an original assignment"));
            };
            if !assignment.destination().projections().is_empty() {
                return Err(rejected("phase constructor destination became projected"));
            }
            if selected.visited == 0 {
                let input = lowering
                    .locals
                    .get(selected.parameter.index() as usize)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| {
                        rejected("phase constructor has no actual parameter transfer")
                    })?;
                let expected = lowering
                    .semantic_ssa_bindings
                    .get(&selected.source_input)
                    .ok_or_else(|| {
                        rejected("phase constructor has no original caller SSA input")
                    })?;
                spend(work, 2)?;
                let input = clone_value(input, work)?;
                let expected = clone_value(expected, work)?;
                if !matches!((&input, &expected), (
                    SemanticValueBindingV1::Value {
                        id, ty: Type::ExecutionCapability(cap),
                    },
                    SemanticValueBindingV1::Value {
                        id: expected_id, ty: Type::ExecutionCapability(expected_cap),
                    },
                ) if id == expected_id && cap == expected_cap
                    && cap.source_type == execution_type_identity_v1(lowering.types, selected.input_type)?) {
                    return Err(rejected(
                        "phase constructor substituted its current source parameter value",
                    ));
                }
            }
            let value = event_value(
                checked,
                site,
                assignment.destination().local().index(),
                PhaseBoundaryKindV1::Define,
                work,
            )?;
            bind_exact(
                lowering,
                site,
                assignment.destination(),
                value,
                SemanticValueBindingV1::Unmaterialized,
            )?;
            selected.visited += 1;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn complete(&self, work: &mut usize) -> PhaseResult<()> {
        for value in &self.values {
            spend(work, 1)?;
            if value.visited != 3 {
                return Err(rejected(
                    "phase constructor did not consume its exact source body",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "constructor_census_tests.rs"]
mod census_tests;
