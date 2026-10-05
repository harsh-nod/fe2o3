//! Original nominal tile calls; no target operation supplies a source value.
use super::super::source_bytes::execution_loans::{self, ExecutionOperand, Role as LoanRole};
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExecutionOperationV29 as Execution, SemanticExecutionRoleV29 as Role,
    SemanticOperandV1 as Operand, SemanticRustTypeKindV1 as RustType, SemanticTypeIdV1 as TypeId,
};

#[derive(Clone, Copy, Debug)]
enum Action {
    ContextIssue,
    WorkgroupDerive {
        context: ExecutionOperand,
    },
    Load {
        workgroup: ExecutionOperand,
        input: usize,
        moved_input: bool,
        base: TypedOperand,
        lanes: u16,
        layout: ExecutionTileLayoutV1,
    },
    Transport {
        input: usize,
        input_type: TypeId,
        parts: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TileCall {
    action: Action,
    destination: usize,
    output_type: TypeId,
    continuation: usize,
    elements: u16,
}

fn geometry(types: &[TypeDecl], ty: TypeId, fragment: bool) -> Result<(u16, u16)> {
    let role = types
        .get(ty.index() as usize)
        .ok_or_else(mismatch)?
        .rust_type_kind();
    match (role, fragment) {
        (RustType::Execution(Role::MaskedTileU32 { lanes, elements }), false)
        | (RustType::Execution(Role::LaneFragmentU32 { lanes, elements }), true) => {
            Ok((lanes, elements))
        }
        _ => Err(mismatch()),
    }
}

fn local(operand: &Operand, move_only: bool) -> Result<(usize, bool)> {
    let (place, moved) = match operand {
        Operand::Move(place) => (place, true),
        Operand::Copy(place) if !move_only => (place, false),
        _ => return Err(unsupported()),
    };
    if !place.projections().is_empty() {
        return Err(unsupported());
    }
    Ok((place.local().index() as usize, moved))
}

impl TileCall {
    /// Policy comes only from the retained source slots' live expansion owner.
    /// A missing selection always refuses an original tile load.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        body: &SourceByteBody<'_, '_, '_>,
        root: usize,
        instance: usize,
        block: usize,
        call: &Call,
        callable: &Callable,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(4)?;
        let Callable::CompilerIntrinsic {
            binding,
            operation: Intrinsic::Execution(operation),
            ..
        } = callable
        else {
            return Ok(None);
        };
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(24)?;
        let row = plan.instance(root, instance, out)?;
        let relation = slots.correspondence(out)?;
        let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let destination_local = destination.place().local().index() as usize;
        let continuation = destination.edge().target().index() as usize;
        let output_type = destination.place().ty();
        if call.unwind() != Unwind::Unreachable
            || !call.variadic_argument_abis().is_empty()
            || !destination.place().projections().is_empty()
            || destination.edge().role() != EdgeRole::CallReturn
            || continuation >= row.blocks.len()
            || function
                .locals()
                .get(destination_local)
                .map(|local| local.ty())
                != Some(output_type)
            || binding.abi().return_type() != output_type
            || binding.abi().source_input_types().len() != call.arguments().len()
            || slots.has_original_object(root, instance, destination_local as u32, out)?
        {
            return Err(mismatch());
        }
        for (argument, ty) in call
            .arguments()
            .iter()
            .zip(binding.abi().source_input_types())
        {
            out.budget.charge_work(1)?;
            if argument.ty() != *ty {
                return Err(mismatch());
            }
        }
        let flat = |index: usize| {
            row.locals
                .start
                .checked_add(index)
                .ok_or(Resource::Arithmetic)
        };
        let (action, elements) = match *operation {
            Execution::ContextIssue { context } => {
                if !call.arguments().is_empty()
                    || context != output_type
                    || semantic
                        .types()
                        .get(context.index() as usize)
                        .map(TypeDecl::rust_type_kind)
                        != Some(RustType::Execution(Role::KernelContext))
                {
                    return Err(mismatch());
                }
                (Action::ContextIssue, 0)
            }
            Execution::MaskedTileLoadU32 { tile, .. } => {
                let [_, input, _] = call.arguments() else {
                    return Err(mismatch());
                };
                if tile != output_type {
                    return Err(mismatch());
                }
                let (layout, selected_lanes) =
                    slots.tile_policy_v162(root, out)?.ok_or_else(unsupported)?;
                let (lanes, elements) = geometry(semantic.types(), tile, false)?;
                if lanes != selected_lanes {
                    return Err(mismatch());
                }
                let workgroup =
                    execution_loans::call_argument(slots, plan, root, instance, block, 0, out)?
                        .ok_or_else(mismatch)?;
                if workgroup.recipe.role != LoanRole::Workgroup || workgroup.recipe.mutable {
                    return Err(mismatch());
                }
                let (input, moved_input) = local(input, false)?;
                let base = body.call_argument(block, 2, out)?;
                if base.scalar()
                    != Some(ScalarV30::Integer {
                        width: 64,
                        signed: false,
                    })
                {
                    return Err(unsupported());
                }
                (
                    Action::Load {
                        workgroup,
                        input: flat(input)?,
                        moved_input,
                        base,
                        lanes,
                        layout,
                    },
                    elements,
                )
            }
            Execution::MaskedTileIntoFragmentU32 { tile, fragment } => {
                let [input] = call.arguments() else {
                    return Err(mismatch());
                };
                let before = geometry(semantic.types(), tile, false)?;
                let after = geometry(semantic.types(), fragment, true)?;
                if fragment != output_type || input.ty() != tile || before != after {
                    return Err(mismatch());
                }
                let (input, _) = local(input, true)?;
                (
                    Action::Transport {
                        input: flat(input)?,
                        input_type: tile,
                        parts: false,
                    },
                    before.1,
                )
            }
            Execution::LaneFragmentIntoPartsU32 { fragment, parts } => {
                let [input] = call.arguments() else {
                    return Err(mismatch());
                };
                let (_, elements) = geometry(semantic.types(), fragment, true)?;
                if parts != output_type || input.ty() != fragment {
                    return Err(mismatch());
                }
                let (input, _) = local(input, true)?;
                (
                    Action::Transport {
                        input: flat(input)?,
                        input_type: fragment,
                        parts: true,
                    },
                    elements,
                )
            }
            Execution::WorkgroupDerive { context, workgroup } => {
                if call.arguments().len() != 1
                    || workgroup != output_type
                    || semantic
                        .types()
                        .get(workgroup.index() as usize)
                        .map(TypeDecl::rust_type_kind)
                        != Some(RustType::Execution(Role::Workgroup))
                {
                    return Err(mismatch());
                }
                let input =
                    execution_loans::call_argument(slots, plan, root, instance, block, 0, out)?
                        .ok_or_else(mismatch)?;
                if input.recipe.role != LoanRole::Context
                    || !input.recipe.mutable
                    || !input.moved
                    || input.recipe.source_type != context.index()
                {
                    return Err(mismatch());
                }
                (Action::WorkgroupDerive { context: input }, 0)
            }
        };
        Ok(Some(Self {
            action,
            destination: flat(destination_local)?,
            output_type,
            continuation: row
                .blocks
                .start
                .checked_add(continuation)
                .ok_or(Resource::Arithmetic)?,
            elements,
        }))
    }

    pub(super) fn emit(
        self,
        root: usize,
        instance: usize,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(4)?;
        write!(out, " let event = InvocationSourceByteEventV36::").map_err(|_| out.error())?;
        match self.action {
            Action::ContextIssue => {
                write!(out, "ContextIssue(InvocationSourceContextIssueV161 {{ destination: {}, source_type: {} }});\n", self.destination, self.output_type.index()).map_err(|_| out.error())?;
            }
            Action::WorkgroupDerive { context } => {
                write!(out, "WorkgroupDerive(InvocationSourceWorkgroupDeriveV168 {{ destination: {}, source_type: {}, context: ", self.destination, self.output_type.index()).map_err(|_| out.error())?;
                context.emit(out)?;
                write!(out, " }});\n").map_err(|_| out.error())?;
            }
            Action::Load {
                workgroup,
                input,
                moved_input,
                base,
                lanes,
                layout,
            } => {
                let layout = match layout {
                    ExecutionTileLayoutV1::Blocked => "Blocked",
                    ExecutionTileLayoutV1::Striped => "Striped",
                };
                write!(
                    out,
                    "TileLoad(InvocationSourceExecutionTileLoadV168 {{ workgroup: "
                )
                .map_err(|_| out.error())?;
                workgroup.emit(out)?;
                write!(out, ", load: InvocationSourceTileLoadV161 {{ destination: {}, source_type: {}, input: {input}, moved_input: {moved_input}, lanes: {lanes}, elements: {}, layout: InvocationSourceTileLayoutV161::{layout}, base: ", self.destination, self.output_type.index(), self.elements).map_err(|_| out.error())?;
                base.emit_scalar_value_v161(out)?;
                write!(out, " }} }});\n").map_err(|_| out.error())?;
            }
            Action::Transport {
                input,
                input_type,
                parts,
            } => {
                write!(out, "TileTransport(InvocationSourceTileTransportV161 {{ destination: {}, output_type: {}, input: {input}, input_type: {}, elements: {}, parts: {parts} }});\n", self.destination, self.output_type.index(), input_type.index(), self.elements).map_err(|_| out.error())?;
            }
        }
        write!(out, " let after = invocation_source_byte_step_v36(cursor.source, event, {root}, {instance}, little_endian);\n let observations = cursor.observations.push(InvocationSourceStatementObservationV36 {{ root: {root}, instance: {instance}, block: {block}, statement: cursor.next_statement, event: Some(event), before: cursor.source, after }});\n let source = invocation_source_byte_pc_v36(after, {});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations, operands: seq![], returned: None }}\n", self.continuation).map_err(|_| out.error())
    }
}

fn headers() -> usize {
    size_of::<TileCall>()
        + 2 * size_of::<Result<Option<TileCall>>>()
        + size_of::<TypedOperand>()
        + execution_loans::headers()
        + 24 * size_of::<usize>()
        + 20 * size_of::<&()>()
}
