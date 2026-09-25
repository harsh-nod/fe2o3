//! CPU bounds implications over the exact checked source argument/read roster.
//! Replay records are descriptive; only the enclosing source join consumes them.
//!
//! The admitted fragment has one canonical/ranked read occurrence per source
//! input, referenced exactly once in the retained output expression. Repeated
//! expression leaves, additional sites for that input, and unused sites are
//! unsupported, not generalized read correspondence. CPU resolver expansions
//! through scalar temporaries may repeat a previously checked input expression.

use super::*;
use crate::portable_reference_v1::{ReferenceTerminatorV1, ResolvedReferenceBoundsCheckV1};
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;

type CpuExpr = ReferenceEffectExpressionV1;

pub use crate::portable_reference_v1::ReplayedCpuEffectsV1;

fn premise(premises: &[Premise], required: Premise, budget: &mut Budget<'_>) -> Result<(), Error> {
    for actual in premises {
        charge(budget, 8)?;
        if *actual == required {
            return Ok(());
        }
    }
    Err(reject("conditional CPU missing exact runtime premise"))
}

fn check_read_occurrence(
    expression: &Expr,
    site: (u32, u32),
    budget: &mut Budget<'_>,
    mut check: impl FnMut(&ProductionSemanticLoadV2, &mut Budget<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut matched = false;
    visit_loads(expression, budget, &mut |load, budget| {
        charge(budget, 4)?;
        if (load.block, load.operation) == site {
            require(
                !matched,
                "conditional CPU input supports only one value-expression load occurrence",
            )?;
            check(load, budget)?;
            matched = true;
        }
        Ok(())
    })?;
    require(
        matched,
        "conditional CPU read occurrence is absent from the matched value expression",
    )
}

pub(super) fn check_source_bound_reads_v1(
    request: &Request<'_>,
    binding: &ConditionalReferenceInputV1<'_>,
    replay: &ReplayedCpuEffectsV1,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let input = request.pliron_input();
    let [output] = input.outputs() else {
        return Err(reject("conditional CPU read output domain"));
    };
    let contract = input
        .effect_contract(output)
        .ok_or_else(|| reject("conditional CPU read effect contract"))?;
    let gpu = semantic_expression(input.kernel(), contract.gpu_value(), budget)?;
    // Cover the complete live read roster, including inputs absent from CPU RHS
    // traversal. Unused sites cannot disappear behind a different raw argument.
    for (index, read) in input.reads().iter().enumerate() {
        charge(budget, 4)?;
        let row = argument(request, read.canonical().parameter(), budget)?;
        for previous in &input.reads()[..index] {
            charge(budget, 4)?;
            let previous = argument(request, previous.canonical().parameter(), budget)?;
            require(
                previous.source_argument() != row.source_argument(),
                "conditional CPU input supports only one checked read occurrence",
            )?;
        }
        check_read_occurrence(
            gpu,
            (read.site().block, read.site().operation),
            budget,
            |_, _| Ok(()),
        )?;
    }
    let premises = input.premises();
    premise(premises, Premise::D1Launch, budget)?;
    premise(
        premises,
        Premise::OutputWithinGlobalX {
            parameter: output.canonical_parameter(),
        },
        budget,
    )?;
    check_replay_v1(&binding.replay.effect_ir, replay, budget, |raw, budget| {
        charge(
            budget,
            binding.replay.signature_preimage.reference_inputs().len(),
        )?;
        charge(
            budget,
            binding.replay.signature_preimage.kernel_inputs().len(),
        )?;
        let relations = binding
            .replay
            .signature_preimage
            .derive_relations_v1()
            .map_err(|_| reject("conditional CPU read premise signature"))?;
        let Some(ReferenceArgumentRelationV1::SharedSliceInput {
            argument: source,
            element,
        }) = relations.relation_at_raw_argument_v1(raw)
        else {
            return Err(reject("conditional CPU read premise ABI role"));
        };
        let mut matched = false;
        for read in request.pliron_input().reads() {
            charge(budget, 16)?;
            let canonical = read.canonical();
            let row = argument(request, canonical.parameter(), budget)?;
            if row.source_argument() != source {
                continue;
            }
            require(
                !matched,
                "conditional CPU input supports only one checked read occurrence",
            )?;
            matched = true;
            check_read_occurrence(
                gpu,
                (read.site().block, read.site().operation),
                budget,
                |load, budget| {
                    charge(budget, 16)?;
                    require(
                        reference_scalar_v2(element) == Some(load.scalar)
                            && load.view == read.view()
                            && load.indices.as_ref() == [read.index()]
                            && load.allocation_origin == u64::from(row.adjusted_argument()) + 1,
                        "conditional CPU read premise source/type/index substitution",
                    )
                },
            )?;
            // Access extent and pointer arithmetic extent are different obligations.
            // In particular, N=0 does not erase a GlobalLaunch address premise.
            premise(
                premises,
                Premise::ReadableInput {
                    parameter: canonical.parameter(),
                    domain: canonical.access_domain(),
                },
                budget,
            )?;
            premise(
                premises,
                Premise::RepresentableAddress {
                    parameter: canonical.parameter(),
                    domain: canonical.address_domain(),
                    element_bytes: canonical.element_bytes(),
                    alignment: canonical.alignment(),
                },
                budget,
            )?;
            premise(
                premises,
                Premise::SeparateInputOutput {
                    input: canonical.parameter(),
                    output: output.canonical_parameter(),
                },
                budget,
            )?;
        }
        require(
            matched,
            "conditional CPU read premise lacks checked source read",
        )
    })
}

fn point(expression: &CpuExpr) -> bool {
    matches!(expression, CpuExpr::PointCoordinate { axis: 0 })
}

fn bound_input(check: &ResolvedReferenceBoundsCheckV1) -> Result<u32, Error> {
    let CpuExpr::InputLength { reference_argument } = &check.length else {
        return Err(reject("conditional CPU bound length is not its input"));
    };
    require(
        check.expected && point(&check.index),
        "conditional CPU bound expected/index",
    )?;
    require(
        matches!(&check.condition,
        CpuExpr::Binary { operation: ReferenceBinaryOpV1::LessThan, lhs, rhs, checked: false }
        if point(lhs) && matches!(rhs.as_ref(), CpuExpr::InputLength { reference_argument: raw }
            if raw == reference_argument)),
        "conditional CPU bound condition substitution",
    )?;
    Ok(*reference_argument)
}

fn next(block: &crate::portable_reference_v1::ReferenceBlockV1) -> Result<Option<u32>, Error> {
    match &block.terminator {
        ReferenceTerminatorV1::Return => Ok(None),
        ReferenceTerminatorV1::Goto { target } => Ok(Some(*target)),
        ReferenceTerminatorV1::Assert {
            success,
            bounds_check: Some(_),
            ..
        } => Ok(Some(*success)),
        _ => Err(reject(
            "conditional CPU control requires an unproved guard or trap",
        )),
    }
}

// The admitted single-successor CFG gives an exact predecessor relation without
// another graph, a dominance cache, or an independently resetting resource meter.
fn precedes(
    ir: &ReferenceEffectIrV1,
    before: u32,
    after: u32,
    budget: &mut Budget<'_>,
) -> Result<bool, Error> {
    let mut current = 0;
    let mut seen = false;
    for _ in 0..ir.blocks.len() {
        charge(budget, 4)?;
        if current == after {
            return Ok(seen);
        }
        seen |= current == before;
        let block = ir
            .blocks
            .get(current as usize)
            .ok_or_else(|| reject("conditional CPU control target"))?;
        let Some(target) = next(block)? else {
            return Ok(false);
        };
        current = target;
    }
    Err(reject("conditional CPU cyclic control"))
}

fn reads<E: From<Error>>(
    expression: &CpuExpr,
    depth: usize,
    budget: &mut Budget<'_>,
    visit: &mut impl FnMut(u32, &mut Budget<'_>) -> Result<(), E>,
) -> Result<(), E> {
    charge(budget, 1)?;
    require(
        depth < fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
        "conditional CPU read expression depth",
    )?;
    match expression {
        CpuExpr::InputLoad {
            reference_argument,
            index,
        } => {
            require(
                point(index),
                "conditional CPU read is not the exact point index",
            )?;
            visit(*reference_argument, budget)
        }
        CpuExpr::Binary { lhs, rhs, .. } => {
            reads(lhs, depth + 1, budget, visit)?;
            reads(rhs, depth + 1, budget, visit)
        }
        CpuExpr::Unary { operand, .. } | CpuExpr::Cast { operand, .. } => {
            reads(operand, depth + 1, budget, visit)
        }
        _ => Ok(()),
    }
}

/// Checks descriptive replay computations and bounds; does not authenticate input.
pub fn check_replay_v1<E: From<Error>>(
    ir: &ReferenceEffectIrV1,
    replay: &ReplayedCpuEffectsV1,
    budget: &mut Budget<'_>,
    mut check_input: impl FnMut(u32, &mut Budget<'_>) -> Result<(), E>,
) -> Result<(), E> {
    let mut current = 0;
    let mut values = 0usize;
    let mut assertions = 0usize;
    for _ in 0..ir.blocks.len() {
        charge(budget, 4)?;
        let block = ir
            .blocks
            .get(current as usize)
            .ok_or_else(|| reject("conditional CPU control target"))?;
        for value in &replay.values {
            charge(budget, 2)?;
            if value.block != current {
                continue;
            }
            values = values
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            charge(budget, block.assignments.len())?;
            require(
                block
                    .assignments
                    .iter()
                    .any(|assignment| assignment.statement == value.statement),
                "conditional CPU replay computation occurrence",
            )?;
            reads::<E>(&value.expression, 0, budget, &mut |raw, budget| {
                check_input(raw, budget)?;
                let mut guarded = false;
                for check in &replay.bounds {
                    charge(budget, 8)?;
                    if bound_input(check)? == raw && precedes(ir, check.block, current, budget)? {
                        guarded = true;
                    }
                }
                require(
                    guarded,
                    "conditional CPU read lacks a preceding exact bounds assertion",
                )
                .map_err(Into::into)
            })?;
        }
        if matches!(block.terminator, ReferenceTerminatorV1::Assert { .. }) {
            let mut selected = None;
            for check in &replay.bounds {
                charge(budget, 4)?;
                if check.block == current {
                    require(
                        selected.replace(check).is_none(),
                        "conditional CPU duplicate bounds assertion",
                    )?;
                }
            }
            let check =
                selected.ok_or_else(|| reject("conditional CPU unproved nonbounds assertion"))?;
            let raw = bound_input(check)?;
            check_input(raw, budget)?;
            assertions = assertions
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let mut used = false;
            for value in &replay.values {
                charge(budget, 2)?;
                if precedes(ir, current, value.block, budget)? {
                    reads::<E>(&value.expression, 0, budget, &mut |input, _| {
                        used |= input == raw;
                        Ok::<(), E>(())
                    })?;
                }
            }
            require(used, "conditional CPU unmatched bounds assertion")?;
        }
        let Some(target) = next(block)? else {
            return require(
                values == replay.values.len() && assertions == replay.bounds.len(),
                "conditional CPU replay contains unchecked control-path computations",
            )
            .map_err(Into::into);
        };
        current = target;
    }
    Err(reject("conditional CPU cyclic control").into())
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
