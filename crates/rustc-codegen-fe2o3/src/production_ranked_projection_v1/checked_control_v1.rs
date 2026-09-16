use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceOutputBlockCoverageV1 as Coverage,
    ProductionSourceOutputBlockV1 as Disposition,
    ProductionSourceOutputSelectedSuccessorV1 as Selection,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    Dormant,
    Branch(usize),
    // Recorder-only continuation conditional on the actual callee returning.
    // Mandatory canonical Call coverage supplies the callee/argument/result join.
    ConditionalCallReturn(usize),
    BoundsAssert {
        next: usize,
        index: ProductionRankedValueV1,
        extent: ProductionRankedValueV1,
    },
    Split(usize, usize),
    Return,
    Trap,
}

#[derive(Clone, Copy, Debug)]
struct Block {
    coverage: Coverage,
    selected: Option<Selection>,
    control: Control,
    reached: bool,
}

#[cfg(test)]
pub(super) const CONTROL_ROW_LAYOUT_BYTES_FOR_TEST: usize = std::mem::size_of::<Block>();

/// The frame covers every source block and retains the exact chosen edge
/// occurrence even when explicit and otherwise target the same block. It grants
/// no authority over live ranked effects, final O operations, or formal evidence.
pub(super) struct CheckedControlFrameV1 {
    blocks: Vec<Block>,
}

fn invalid(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(detail)
}

fn arithmetic() -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
        ),
    )
}

fn allocate<T>(count: usize) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            ),
        )
    })?;
    if values.capacity() != count {
        return Err(invalid(
            "checked control allocation exceeded its prepaid capacity",
        ));
    }
    Ok(values)
}

fn live(coverage: Coverage) -> bool {
    matches!(
        coverage.disposition(),
        Disposition::Materialized {
            executable: true,
            ..
        }
    )
}

fn target(
    function: &SemanticFunctionDeclV1,
    target: SemanticBlockIdV1,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    let index = target.index() as usize;
    if index >= function.blocks().len() {
        return Err(invalid(
            "checked control source edge is outside the function",
        ));
    }
    Ok(index)
}

fn selected_target(
    explicit: SemanticBlockIdV1,
    otherwise: SemanticBlockIdV1,
    ordinal: u32,
    selected: SemanticBlockIdV1,
) -> Result<SemanticBlockIdV1, ProductionRankedProjectionErrorV1> {
    let expected = match ordinal {
        0 => explicit,
        1 => otherwise,
        _ => return Err(invalid("checked control selected edge ordinal is invalid")),
    };
    if selected != expected {
        return Err(invalid(
            "checked control selected edge occurrence differs from source",
        ));
    }
    Ok(expected)
}

fn recorded_call_return(
    call: &SemanticDirectCallV1,
    callables: &[SemanticCallableDeclV1],
) -> Option<SemanticBlockIdV1> {
    let destination = call.destination()?;
    if destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
        || !matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::Defined { .. })
        )
    {
        return None;
    }
    Some(destination.edge().target())
}

#[cfg(test)]
fn admitted_shape(
    terminator: &SemanticTerminatorKindV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    admitted_shape_with_recorded_calls(terminator, types, callables, false)
}

fn admitted_shape_with_recorded_calls(
    terminator: &SemanticTerminatorKindV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    recorded_calls: bool,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::Goto(_) | SemanticTerminatorKindV1::Return => Ok(()),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            if targets.values().len() != 1
                || targets.values()[0].value() > 1
                || !matches!(
                    types
                        .get(discriminant.ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
                )
            {
                return Err(invalid(
                    "checked partial control has an unsupported source switch mapping",
                ));
            }
            Ok(())
        }
        SemanticTerminatorKindV1::Assert { unwind, .. } => {
            if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
                return Err(invalid("checked control assertion has unsupported cleanup"));
            }
            Ok(())
        }
        SemanticTerminatorKindV1::Call(call)
            if recorded_calls && recorded_call_return(call, callables).is_some() =>
        {
            Ok(())
        }
        SemanticTerminatorKindV1::Call(call)
            if call.destination().is_none()
                && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                && matches!(
                    callables.get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Trap,
                        ..
                    })
                ) =>
        {
            Ok(())
        }
        _ => Err(invalid(
            "checked partial control has unsupported generated, split, or call mapping",
        )),
    }
}

#[cfg(test)]
pub(super) fn prepare(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<CheckedControlFrameV1>, ProductionRankedProjectionErrorV1> {
    prepare_with_bounds(types, function, callables, &[], facts)
}

pub(super) fn prepare_with_bounds(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    bounds_checks: &[ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<CheckedControlFrameV1>, ProductionRankedProjectionErrorV1> {
    prepare_inner(types, function, callables, bounds_checks, facts, false)
}

/// Only the canonical recorder path may defer ordinary Call continuation to its
/// mandatory exact retained-Call coverage. This does not establish a return.
pub(super) fn prepare_with_recorded_calls_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    bounds_checks: &[ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<Option<CheckedControlFrameV1>, ProductionRankedProjectionErrorV1> {
    prepare_inner(types, function, callables, bounds_checks, facts, true)
}

fn prepare_inner(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    bounds_checks: &[ProjectedBoundsCheckV1],
    facts: &mut impl ProjectedAssertionFactsV1,
    recorded_calls: bool,
) -> Result<Option<CheckedControlFrameV1>, ProductionRankedProjectionErrorV1> {
    if !facts.checked_control_enabled_v1() {
        return Ok(None);
    }
    facts.charge_private_array_work(4)?;
    let count = function.blocks().len();
    // Check the borrowed view's incoming custody floor before our own scratch
    // reservation can make an under-reserved caller appear sufficiently funded.
    let first_coverage = facts.checked_block_coverage_v1(0)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<Block>())
        .and_then(|n| n.checked_add(std::mem::size_of::<CheckedControlFrameV1>()))
        .ok_or_else(arithmetic)?;
    facts.reserve_checked_control_storage_v1(bytes)?;
    let mut blocks = allocate(count)?;
    let mut partial = false;
    for (index, source) in function.blocks().iter().enumerate() {
        facts.charge_private_array_work(4)?;
        let coverage = if index == 0 {
            first_coverage
        } else {
            facts.checked_block_coverage_v1(index)?
        };
        if coverage.source_statements() != source.statements().len()
            || matches!(coverage.disposition(), Disposition::NotMaterialized)
                != coverage.original_operations().is_none()
        {
            return Err(invalid("checked control source block coverage differs"));
        }
        partial |= !live(coverage);
        blocks.push(Block {
            coverage,
            selected: None,
            control: Control::Dormant,
            reached: false,
        });
    }
    // No whole-block omission is needed when every source block is executable.
    // Keep the existing all-live grammar and ranked CFG preparation unchanged.
    if !partial {
        for (index, source) in function.blocks().iter().enumerate() {
            facts.charge_private_array_work(2)?;
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                ..
            } = source.terminator().kind()
            else {
                continue;
            };
            if matches!(facts.condition(index, *expected, target.target())?,
                canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Bool(actual) if actual != *expected)
            {
                return Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                    block: index,
                    kind: semantic_assert_kind_v1(message),
                    expected: *expected,
                    condition_local: simple_operand_local(condition).map(SemanticLocalIdV1::index),
                    source: Box::new(source.terminator().source()),
                });
            }
        }
        return Ok(None);
    }
    for (index, row) in blocks.iter_mut().enumerate() {
        facts.charge_private_array_work(6)?;
        let source = &function.blocks()[index];
        admitted_shape_with_recorded_calls(
            source.terminator().kind(),
            types,
            callables,
            recorded_calls,
        )?;
        if !live(row.coverage) {
            continue;
        }
        row.control = match source.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => {
                Control::Branch(target(function, edge.target())?)
            }
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => {
                facts.charge_private_array_work(6)?;
                let declaration = types
                    .get(discriminant.ty().index() as usize)
                    .ok_or_else(|| invalid("checked control selector type is absent"))?;
                let [explicit] = targets.values() else {
                    return Err(invalid(
                        "checked control requires one explicit Boolean successor",
                    ));
                };
                if !matches!(
                    declaration.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                ) || explicit.value() > 1
                {
                    return Err(invalid(
                        "checked control selector is not an exact Boolean switch",
                    ));
                }
                // Query before any target deduplication. The sealed record keeps
                // original N edge identity as well as source successor ordinal.
                let selected = facts.checked_selected_successor_v1(index)?;
                facts.charge_private_array_work(4)?;
                row.selected = selected;
                if let Some(selected) = selected {
                    let Disposition::Materialized { original, .. } = row.coverage.disposition()
                    else {
                        return Err(invalid(
                            "checked control selected block has no original coordinate",
                        ));
                    };
                    if selected.input().source != original {
                        return Err(invalid("checked control selected input block differs"));
                    }
                    Control::Branch(target(
                        function,
                        selected_target(
                            explicit.edge().target(),
                            targets.otherwise().target(),
                            selected.semantic_ordinal(),
                            selected.semantic_target(),
                        )?,
                    )?)
                } else {
                    Control::Split(
                        target(function, explicit.edge().target())?,
                        target(function, targets.otherwise().target())?,
                    )
                }
            }
            SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target: edge,
                unwind,
            } => {
                facts.charge_private_array_work(3)?;
                if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
                    return Err(invalid("checked control assertion has unsupported cleanup"));
                }
                // Bounds assertions also query their actual asserting block.
                // A dormant access target never discards a live failed guard.
                let condition_value = facts.condition(index, *expected, edge.target())?;
                let retained = match condition_value {
                    canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Bool(_) => None,
                    canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Dynamic
                    | canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Unknown => {
                        if matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                            bounds_cfg_v1::source_guard(bounds_checks, index, edge.target(), facts)?
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let discharged = matches!(condition_value,
                    canonical_assertion_facts_v1::ProjectedAssertionConditionV1::Bool(actual) if actual == *expected)
                    || retained.is_some();
                if !discharged {
                    return Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                        block: index,
                        kind: semantic_assert_kind_v1(message),
                        expected: *expected,
                        condition_local: simple_operand_local(condition)
                            .map(SemanticLocalIdV1::index),
                        source: Box::new(source.terminator().source()),
                    });
                }
                let next = target(function, edge.target())?;
                if let Some(guard) = retained {
                    Control::BoundsAssert {
                        next,
                        index: guard.index,
                        extent: guard.extent,
                    }
                } else {
                    Control::Branch(next)
                }
            }
            SemanticTerminatorKindV1::Return => Control::Return,
            SemanticTerminatorKindV1::Call(call)
                if recorded_calls && call.destination().is_some() =>
            {
                facts.charge_private_array_work(3)?;
                let next = recorded_call_return(call, callables).ok_or_else(|| {
                    invalid("checked recorded Call lost its ordinary continuation")
                })?;
                Control::ConditionalCallReturn(target(function, next)?)
            }
            SemanticTerminatorKindV1::Call(call)
                if call.destination().is_none()
                    && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                    && matches!(
                        callables.get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::Trap,
                            ..
                        })
                    ) =>
            {
                Control::Trap
            }
            _ => {
                return Err(invalid(
                    "checked partial control has unsupported generated, split, or call mapping",
                ));
            }
        };
    }
    let queue_bytes = count
        .checked_mul(std::mem::size_of::<usize>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<usize>>()))
        .ok_or_else(arithmetic)?;
    facts.charge_private_array_work(3)?;
    facts.reserve_checked_control_storage_v1(queue_bytes)?;
    let mut pending = allocate(count)?;
    let entry = function.entry().index() as usize;
    reach(&mut blocks, &mut pending, entry, facts)?;
    let mut cursor = 0;
    while cursor < pending.len() {
        facts.charge_private_array_work(3)?;
        let index = pending[cursor];
        cursor += 1;
        match blocks[index].control {
            Control::Dormant => {
                return Err(invalid("checked source CFG reaches a nonexecuting block"));
            }
            Control::Branch(next)
            | Control::ConditionalCallReturn(next)
            | Control::BoundsAssert { next, .. } => reach(&mut blocks, &mut pending, next, facts)?,
            Control::Split(first, second) => {
                reach(&mut blocks, &mut pending, first, facts)?;
                reach(&mut blocks, &mut pending, second, facts)?;
            }
            Control::Return | Control::Trap => {}
        }
    }
    for row in &blocks {
        facts.charge_private_array_work(2)?;
        if row.reached != live(row.coverage) {
            return Err(invalid(
                "checked source reachability and sealed block disposition disagree",
            ));
        }
    }
    drop(pending);
    Ok(Some(CheckedControlFrameV1 { blocks }))
}

fn reach(
    blocks: &mut [Block],
    pending: &mut Vec<usize>,
    next: usize,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    facts.charge_private_array_work(3)?;
    let row = blocks
        .get_mut(next)
        .ok_or_else(|| invalid("checked source CFG target is absent"))?;
    if !live(row.coverage) {
        return Err(invalid(
            "checked source CFG reaches an original-absent or nonexecuting block",
        ));
    }
    if !row.reached {
        row.reached = true;
        pending.push(next);
    }
    Ok(())
}

impl CheckedControlFrameV1 {
    #[cfg(test)]
    pub(super) fn coverage_for_test(&self, index: usize) -> Coverage {
        self.blocks[index].coverage
    }

    #[cfg(test)]
    pub(super) fn selected_for_test(&self, index: usize) -> Option<Selection> {
        self.blocks[index].selected
    }

    pub(super) fn projects_block(&self, index: usize) -> bool {
        self.blocks[index].reached
    }

    pub(super) fn into_cfg(
        self,
        projected: &[ProjectedSemanticBlockV1],
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(Vec<ProjectedCfgTerminatorV1>, Vec<bool>), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(4)?;
        let count = self.blocks.len();
        if projected.len() != count {
            return Err(invalid("checked control frame lost a source block"));
        }
        let bytes = count
            .checked_mul(
                std::mem::size_of::<ProjectedCfgTerminatorV1>() + std::mem::size_of::<bool>(),
            )
            .and_then(|n| {
                n.checked_add(
                    std::mem::size_of::<Vec<ProjectedCfgTerminatorV1>>()
                        + std::mem::size_of::<Vec<bool>>(),
                )
            })
            .ok_or_else(arithmetic)?;
        facts.reserve_checked_control_storage_v1(bytes)?;
        let mut terminators = allocate(count)?;
        let mut reachable = allocate(count)?;
        for (row, projected) in self.blocks.into_iter().zip(projected) {
            facts.charge_private_array_work(3)?;
            if !row.reached && !projected.items.is_empty() {
                return Err(invalid("checked dormant block acquired projected effects"));
            }
            reachable.push(row.reached);
            terminators.push(match row.control {
                Control::Dormant => ProjectedCfgTerminatorV1::AbsentMaterialized,
                Control::Branch(next) => ProjectedCfgTerminatorV1::Branch(next),
                // This analysis edge is conditional on actual Return; the live
                // recorder retains the source Call, and D must check it before
                // a completed analysis escapes. No progress is inferred here.
                Control::ConditionalCallReturn(next) => ProjectedCfgTerminatorV1::Branch(next),
                Control::BoundsAssert {
                    next,
                    index,
                    extent,
                } => ProjectedCfgTerminatorV1::BoundsAssert {
                    index,
                    extent,
                    success: next,
                },
                Control::Split(first_block, second_block) => {
                    ProjectedCfgTerminatorV1::AnalysisSplit {
                        first_block,
                        second_block,
                    }
                }
                Control::Return => ProjectedCfgTerminatorV1::Return,
                Control::Trap => ProjectedCfgTerminatorV1::Trap,
            });
        }
        Ok((terminators, reachable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiIdentityV1, SemanticAbiValueV1, SemanticCallDestinationV1, SemanticCanonAbiV1,
        SemanticCodeObjectVersionV1, SemanticCompilerIntrinsicIdentityV1,
        SemanticConstGenericArgumentsIdentityV1, SemanticControlFlowEdgeV1,
        SemanticDeviceFfiContractIdentityV1, SemanticDeviceFfiEffectsV1,
        SemanticDeviceFfiImportContractV1, SemanticDeviceFfiPhysicalAbiIdentityV1,
        SemanticDeviceFfiSemanticIdentityV1, SemanticDeviceFfiTargetV1, SemanticFunctionAbiV1,
        SemanticGenericTypeArgumentsIdentityV1, SemanticItemDefinitionIdentityV1,
        SemanticLayoutIdentityV1, SemanticLinkSymbolV1, SemanticMonomorphizationIdentityV1,
        SemanticNonBodyCallableBindingV1,
    };

    fn ordinary_call(
        destination: bool,
        role: SemanticEdgeRoleV1,
        unwind: SemanticUnwindActionV1,
    ) -> SemanticTerminatorKindV1 {
        let destination = destination.then(|| {
            SemanticCallDestinationV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(0),
                    vec![],
                    SemanticTypeIdV1::from_index(0),
                )
                .unwrap(),
                SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(1)),
            )
        });
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(0),
                vec![],
                destination,
                unwind,
            )
            .unwrap(),
        )
    }

    // These inert rows exercise grammar dispatch, not source admission or a
    // completed canonical capability. Genuine source regressions live in D/F.
    fn nonbody_binding() -> SemanticNonBodyCallableBindingV1 {
        SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([1; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([2; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([3; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([4; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([5; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([6; 32]),
                SemanticLayoutIdentityV1::from_sha256([7; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![],
                SemanticAbiValueV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticAbiPassModeV1::Ignore,
                ),
            )
            .unwrap(),
        )
    }

    #[test]
    fn ordinary_call_continuation_requires_recorder_mode() {
        let callables = [SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )];
        let call = ordinary_call(
            true,
            SemanticEdgeRoleV1::CallReturn,
            SemanticUnwindActionV1::Unreachable,
        );
        assert!(admitted_shape(&call, &[], &callables).is_err());
        assert!(admitted_shape_with_recorded_calls(&call, &[], &callables, true).is_ok());
        let SemanticTerminatorKindV1::Call(call) = &call else {
            unreachable!()
        };
        assert_eq!(
            recorded_call_return(call, &callables),
            Some(SemanticBlockIdV1::from_index(1))
        );
    }

    #[test]
    fn recorded_call_requires_normal_destination_without_cleanup() {
        let callables = [SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )];
        for call in [
            ordinary_call(
                false,
                SemanticEdgeRoleV1::CallReturn,
                SemanticUnwindActionV1::Unreachable,
            ),
            ordinary_call(
                true,
                SemanticEdgeRoleV1::CallReturn,
                SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallUnwind,
                    SemanticBlockIdV1::from_index(2),
                )),
            ),
            ordinary_call(
                true,
                SemanticEdgeRoleV1::Goto,
                SemanticUnwindActionV1::Unreachable,
            ),
        ] {
            assert!(admitted_shape_with_recorded_calls(&call, &[], &callables, true).is_err());
        }
        let call = ordinary_call(
            true,
            SemanticEdgeRoleV1::CallReturn,
            SemanticUnwindActionV1::Unreachable,
        );
        assert!(admitted_shape_with_recorded_calls(&call, &[], &[], true).is_err());
    }

    #[test]
    fn recorded_call_does_not_admit_external_or_generated_continuations() {
        let call = ordinary_call(
            true,
            SemanticEdgeRoleV1::CallReturn,
            SemanticUnwindActionV1::Unreachable,
        );
        let external = SemanticCallableDeclV1::DeviceFfiImport {
            binding: nonbody_binding(),
            contract: SemanticDeviceFfiImportContractV1::new(
                SemanticDeviceFfiContractIdentityV1::from_sha256([8; 32]),
                SemanticLinkSymbolV1::new(b"untrusted_import".to_vec()).unwrap(),
                SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus,
                SemanticCodeObjectVersionV1::V6,
                SemanticDeviceFfiPhysicalAbiIdentityV1::from_sha256([9; 32]),
                SemanticDeviceFfiEffectsV1::none(),
                SemanticDeviceFfiSemanticIdentityV1::from_sha256([10; 32]),
            ),
        };
        let generated = SemanticCallableDeclV1::CompilerIntrinsic {
            binding: nonbody_binding(),
            operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([11; 32]),
        };
        for callable in [external, generated] {
            assert!(admitted_shape_with_recorded_calls(&call, &[], &[callable], true).is_err());
        }
    }

    #[test]
    fn explicit_trap_grammar_is_unchanged_in_both_modes() {
        let call = ordinary_call(
            false,
            SemanticEdgeRoleV1::CallReturn,
            SemanticUnwindActionV1::Unreachable,
        );
        let callables = [SemanticCallableDeclV1::CompilerIntrinsic {
            binding: nonbody_binding(),
            operation: SemanticCompilerIntrinsicOperationV1::Trap,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([12; 32]),
        }];
        for mode in [false, true] {
            assert!(admitted_shape_with_recorded_calls(&call, &[], &callables, mode).is_ok());
        }
    }

    #[test]
    fn duplicate_targets_still_require_a_valid_selected_occurrence() {
        let target = SemanticBlockIdV1::from_index(1);
        for ordinal in [0, 1] {
            assert_eq!(
                selected_target(target, target, ordinal, target).unwrap(),
                target
            );
        }
        assert!(selected_target(target, target, 2, target).is_err());
        assert!(selected_target(target, target, 0, SemanticBlockIdV1::from_index(2)).is_err());
    }

    #[test]
    fn isolated_grammar_refuses_unmapped_split_and_call_control() {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticControlFlowEdgeV1, SemanticDirectCallV1};
        let edge = SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(0),
        );
        assert!(
            admitted_shape(
                &SemanticTerminatorKindV1::FalseEdge {
                    real_target: edge,
                    imaginary_target: edge,
                },
                &[],
                &[]
            )
            .is_err()
        );
        assert!(admitted_shape(&SemanticTerminatorKindV1::Unreachable, &[], &[]).is_err());
        let call = SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(0),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        assert!(admitted_shape(&SemanticTerminatorKindV1::Call(call), &[], &[]).is_err());
    }
}
