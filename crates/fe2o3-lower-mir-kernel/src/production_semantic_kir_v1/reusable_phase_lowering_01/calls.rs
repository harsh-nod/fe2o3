//! Exact call coordinates selected from a checked phase recipe. An intrinsic
//! terminal has no expanded callee and must never acquire a fabricated one.
use super::*;
use fe2o3_kernel_ir::ExecutionCapabilitySourceOccurrenceV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionAbiV1, SemanticPhaseCallableV1};
use fe2o3_mir_model::{SemanticExpandedRootV1, SemanticExpandedTerminatorOriginV1 as Origin};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallKind {
    Terminal,
    Expanded(SemanticCallInstanceIdV1),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Call {
    pub(super) source: ExecutionCapabilitySourceV1,
    pub(super) original_normal_target: SemanticBlockIdV1,
    pub(super) expanded_normal_target: SemanticBlockIdV1,
    pub(super) kind: CallKind,
    pub(super) signature: ExecutionCapabilitySignatureV1,
}

impl Call {
    pub(super) fn defined(self) -> PhaseResult<fe2o3_kernel_ir::PhaseCallOccurrenceV1> {
        let CallKind::Expanded(callee) = self.kind else {
            return Err(rejected(
                "terminal phase call has no expanded callee instance",
            ));
        };
        Ok(fe2o3_kernel_ir::PhaseCallOccurrenceV1 {
            source: self.source,
            callee_instance: callee.index(),
            original_normal_target: self.original_normal_target.index(),
            expanded_normal_target: self.expanded_normal_target.index(),
        })
    }

    pub(super) fn terminal(self) -> PhaseResult<fe2o3_kernel_ir::PhaseTerminalCallOccurrenceV1> {
        if self.kind != CallKind::Terminal {
            return Err(rejected(
                "phase terminal unexpectedly has an expanded child",
            ));
        }
        Ok(fe2o3_kernel_ir::PhaseTerminalCallOccurrenceV1 {
            source: self.source,
            original_normal_target: self.original_normal_target.index(),
            expanded_normal_target: self.expanded_normal_target.index(),
        })
    }

    pub(super) fn drop_occurrence(self) -> PhaseResult<fe2o3_kernel_ir::PhaseDropOccurrenceV1> {
        match self.kind {
            CallKind::Terminal => self
                .terminal()
                .map(fe2o3_kernel_ir::PhaseDropOccurrenceV1::Source),
            CallKind::Expanded(_) => self
                .defined()
                .map(fe2o3_kernel_ir::PhaseDropOccurrenceV1::CallEntry),
        }
    }
}

pub(super) fn mapped_block(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    block: SemanticBlockIdV1,
    work: &mut usize,
) -> PhaseResult<SemanticBlockIdV1> {
    let frame = view
        .instances()
        .get(instance.index() as usize)
        .ok_or_else(|| rejected("phase call instance is absent"))?;
    let mut selected = None;
    for (index, origin) in view.block_origins().iter().enumerate() {
        spend(work, 1)?;
        if origin.instance() == instance && origin.block() == block {
            if origin.function() != frame.function() || selected.replace(index).is_some() {
                return Err(rejected(
                    "phase call has ambiguous original block attribution",
                ));
            }
        }
    }
    selected
        .and_then(|index| u32::try_from(index).ok())
        .map(SemanticBlockIdV1::from_index)
        .ok_or_else(|| rejected("phase original block is absent from the retained view"))
}

/// `expected` and `original_block` come from the selected WithPhase/Finish
/// recipe, or the already checked defined-capability binding, not a raw ID API.
pub(super) fn selected_call(
    checked: &CheckedRows<'_>,
    caller: SemanticCallInstanceIdV1,
    original_block: SemanticBlockIdV1,
    expected: SemanticPhaseCallableV1,
    work: &mut usize,
) -> PhaseResult<Call> {
    let owner = checked.owner;
    let semantic = owner.source_semantic();
    let view = owner
        .execution_view_for_root(checked.input.root)
        .ok_or_else(|| rejected("phase call root is absent"))?;
    let frame = view
        .instances()
        .get(caller.index() as usize)
        .ok_or_else(|| rejected("phase caller instance is absent"))?;
    let function = semantic
        .functions()
        .get(frame.function().index() as usize)
        .ok_or_else(|| rejected("phase caller function is absent"))?;
    if frame.function_identity() != function.identity() {
        return Err(rejected("phase caller identity changed"));
    }
    let Some(SemanticTerminatorKindV1::Call(original)) = function
        .blocks()
        .get(original_block.index() as usize)
        .map(|block| block.terminator().kind())
    else {
        return Err(rejected("phase recipe no longer selects an original call"));
    };
    if original.callee() != expected.callable {
        return Err(rejected("phase recipe callee changed"));
    }
    let (abi, identity, defined) =
        match semantic.callables().get(original.callee().index() as usize) {
            Some(SemanticCallableDeclV1::Defined { function }) => {
                let body = semantic
                    .functions()
                    .get(function.index() as usize)
                    .ok_or_else(|| rejected("phase defined callee is absent"))?;
                (body.abi(), body.identity(), Some(*function))
            }
            Some(SemanticCallableDeclV1::CompilerIntrinsic { binding, .. }) => {
                (binding.abi(), binding.identity(), None)
            }
            _ => {
                return Err(rejected(
                    "phase call is not a closed defined or intrinsic call",
                ));
            }
        };
    if identity != expected.identity || abi.identity() != expected.abi {
        return Err(rejected("phase call identity or exact ABI changed"));
    }
    check_call_abi(original, abi, work)?;
    let original_normal_target = original.destination().unwrap().edge().target();
    let expanded_normal_target = mapped_block(view, caller, original_normal_target, work)?;
    let expanded = mapped_block(view, caller, original_block, work)?;
    let origin = &view.block_origins()[expanded.index() as usize];
    let kind = match origin.terminator() {
        Origin::Source => {
            let SemanticTerminatorKindV1::Call(actual) = view.body().blocks()
                [expanded.index() as usize]
                .terminator()
                .kind()
            else {
                return Err(rejected("phase source call is absent"));
            };
            check_call_abi(actual, abi, work)?;
            if actual.callee() != expected.callable
                || actual.destination().unwrap().edge().target() != expanded_normal_target
            {
                return Err(rejected("phase terminal changed its callee or normal edge"));
            }
            CallKind::Terminal
        }
        Origin::CallEntry { callee } => {
            let child = view
                .instances()
                .get(callee.index() as usize)
                .ok_or_else(|| rejected("phase expanded child is absent"))?;
            if child.parent() != Some(caller)
                || child.call_block() != Some(original_block)
                || Some(child.function()) != defined
                || child.function_identity() != identity
            {
                return Err(rejected(
                    "phase expanded call changed its exact child instance",
                ));
            }
            CallKind::Expanded(callee)
        }
        Origin::CallReturn { .. } => {
            return Err(rejected(
                "phase recipe selected a return instead of its call",
            ));
        }
    };
    let mut arguments = reserve(abi.source_input_types().len(), work)?;
    for ty in abi.source_input_types() {
        spend(work, 1)?;
        arguments.push(execution_type_identity_v1(semantic.types(), *ty)?);
    }
    let signature = ExecutionCapabilitySignatureV1::new(
        &arguments,
        execution_type_identity_v1(semantic.types(), abi.source_output_type())?,
    )
    .ok_or_else(|| rejected("phase signature exceeds existing execution arity"))?;
    let source = ExecutionCapabilitySourceV1 {
        function: *function.identity().as_bytes(),
        operation: *identity.as_bytes(),
        block: original_block.index(),
        occurrence: Some(
            ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                *semantic.functions()[checked.input.root.index() as usize]
                    .identity()
                    .as_bytes(),
                checked.input.expansion,
                checked.input.expanded_root,
                caller.index(),
                expanded.index(),
            )
            .ok_or_else(|| rejected("phase source occurrence is incomplete"))?,
        ),
    };
    Ok(Call {
        source,
        original_normal_target,
        expanded_normal_target,
        kind,
        signature,
    })
}

fn check_call_abi(
    call: &SemanticDirectCallV1,
    abi: &SemanticFunctionAbiV1,
    work: &mut usize,
) -> PhaseResult<()> {
    spend(work, call.arguments().len())?;
    if abi.c_variadic()
        || !call.variadic_argument_abis().is_empty()
        || call.unwind() != fe2o3_mir_model::semantic_mir_v1::SemanticUnwindActionV1::Unreachable
        || call
            .arguments()
            .iter()
            .map(semantic_operand_type)
            .ne(abi.source_input_types().iter().copied())
        || call.destination().is_none_or(|result| {
            result.place().ty() != abi.source_output_type()
                || !result.place().projections().is_empty()
                || result.edge().role()
                    != fe2o3_mir_model::semantic_mir_v1::SemanticEdgeRoleV1::CallReturn
        })
    {
        return Err(rejected(
            "phase call changed its retained ABI or normal-return contract",
        ));
    }
    Ok(())
}
