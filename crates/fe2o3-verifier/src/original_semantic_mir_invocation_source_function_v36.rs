//! Ordered source statement execution and exact archived invocation control.

use super::super::{
    ScalarV30, Terminator,
    invocations::{CallKind, InvocationPlan},
};
use super::source_bytes::{Event, SourceByteBody, TypedOperand};
use super::source_enter::SourceFrameEnter;
use super::source_frames::SourceFrameReturn;
use super::source_scalar::SourceScalarStatements;
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::{
    SsaBlockIdV1 as Block,
    semantic_mir_v1::{
        SemanticCallableDeclV1 as Callable, SemanticCompilerIntrinsicOperationV1 as Intrinsic,
        SemanticDirectCallV1 as Call, SemanticEdgeRoleV1 as EdgeRole,
        SemanticTypeDeclV1 as TypeDecl, SemanticTypeShapeV1 as TypeShape,
        SemanticUnwindActionV1 as Unwind,
    },
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[path = "original_semantic_mir_source_descriptor_calls_v51.rs"]
mod descriptor_calls;
#[path = "original_semantic_mir_source_descriptor_indices_v52.rs"]
mod descriptor_indices;
#[path = "original_semantic_mir_invocation_source_index_v37.rs"]
mod index_calls;
#[path = "original_semantic_mir_source_thread_write_v88.rs"]
mod thread_write;

#[path = "original_semantic_mir_source_assert_control_v40.rs"]
mod assertions;

#[path = "original_semantic_mir_source_conservation_v81.rs"]
mod conservation;

#[path = "original_semantic_mir_source_step_hints_v85.rs"]
mod step_hints;

#[path = "original_semantic_mir_cut_frame_generate_v93.rs"]
mod cut_frames;

const THREAD_WRITE_NORMAL_V94: &str =
    include_str!("original_semantic_mir_thread_write_normal_laws_v94.vrs");

pub(super) struct SourceEntryHintsV85 {
    pub(super) owner: u32,
    pub(super) locals: Range<usize>,
    pub(super) pc: usize,
    pub(super) arguments: Vec<usize>,
}

pub(super) struct SourceCallHintsV85 {
    pub(super) child: usize,
    pub(super) arguments: Vec<(usize, bool, u32)>,
}

pub(super) struct SourceCutHintsV85 {
    pub(super) pc: usize,
    pub(super) instance: usize,
    pub(super) statements: usize,
    pub(super) operands: usize,
    pub(super) call: Option<SourceCallHintsV85>,
    pub(super) frame_preserving: bool,
    normalization: Option<thread_write::ThreadWriteCall>,
}

impl SourceCutHintsV85 {
    pub(super) fn emit_write_normalization(
        &self,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.reserve_storage(
            size_of::<thread_write::ThreadWriteCall>() + size_of::<Result<()>>(),
        )?;
        out.budget.charge_work(1)?;
        if let Some(call) = self.normalization {
            call.emit_normalization(root, self.instance, out)?;
        }
        Ok(())
    }
}

pub(super) struct SourceStepHintsV85 {
    pub(super) conserves_heap: bool,
    pub(super) fuels: Vec<usize>,
    pub(super) entries: Vec<Option<SourceEntryHintsV85>>,
    pub(super) cuts: Vec<SourceCutHintsV85>,
}

enum End {
    Unreachable,
    Abort,
    Goto(usize),
    Switch {
        operand: TypedOperand,
        cases: Vec<(u128, usize)>,
        otherwise: usize,
    },
    Call {
        child: usize,
        arguments: Vec<TypedOperand>,
    },
    Index(index_calls::IndexCall),
    Descriptor(descriptor_calls::DescriptorCall),
    DescriptorIndex(descriptor_indices::DescriptorIndexCall),
    ThreadWrite(thread_write::ThreadWriteCall),
    Assert(assertions::SourceAssertControlV40),
    Return,
}

struct BodyBlock {
    statements: usize,
    end: End,
}

pub(super) struct SourceByteFunction<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    blocks: Range<usize>,
    body: SourceByteBody<'slots, 'view, 'source>,
    scalar: SourceScalarStatements<'slots, 'view, 'source>,
    enter: SourceFrameEnter<'slots, 'view, 'source>,
    returned: SourceFrameReturn<'slots, 'view, 'source>,
    control: Vec<BodyBlock>,
    required: usize,
}

pub(super) struct SourceByteProgram<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    functions: Vec<Option<SourceByteFunction<'slots, 'view, 'source>>>,
    roots: Vec<(Range<usize>, u32, usize)>,
    locals: usize,
    required: usize,
}

impl<'slots, 'view, 'source> SourceByteProgram<'slots, 'view, 'source> {
    pub(super) fn emit_thread_write_normal_proofs_v94(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.source_slots(out)?;
        out.budget.reserve_storage(
            4 * size_of::<usize>()
                + 4 * size_of::<&()>()
                + size_of::<Option<SourceStepHintsV85>>()
                + size_of::<Result<Option<SourceStepHintsV85>>>()
                + size_of::<std::slice::Iter<'_, SourceCutHintsV85>>()
                + size_of::<Result<()>>(),
        )?;
        for root in 0..self.roots.len() {
            out.budget.charge_work(1)?;
            let Some(hints) = step_hints::derive(self, root, out)? else {
                continue;
            };
            for hint in &hints.cuts {
                out.budget.charge_work(1)?;
                if hint.normalization.is_some() {
                    write!(out, "{THREAD_WRITE_NORMAL_V94}").map_err(|_| out.error())?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub(super) fn emit_cut_frame_proofs_v93(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        cut_frames::emit(self, out)
    }

    pub(super) fn step_hints(
        &self,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<SourceStepHintsV85>> {
        self.source_slots(out)?;
        step_hints::derive(self, root, out)
    }

    pub(super) fn conservation_fuels(
        &self,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Vec<usize>>> {
        self.source_slots(out)?;
        conservation::derive(self, root, out)
    }

    pub(super) fn in_place_call(
        &self,
        root: usize,
        instance: usize,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.source_slots(out)?;
        out.budget.charge_work(4)?;
        let range = &self.roots.get(root).ok_or_else(mismatch)?.0;
        if instance >= range.len() {
            return Err(mismatch());
        }
        let function = self.functions[range.start + instance]
            .as_ref()
            .ok_or_else(mismatch)?;
        let row = function.control.get(block).ok_or_else(mismatch)?;
        Ok(matches!(
            row.end,
            End::Index(_)
                | End::Descriptor(_)
                | End::DescriptorIndex(_)
                | End::ThreadWrite(_)
                | End::Abort
        ))
    }

    pub(super) fn source_slots(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<&'slots SourceSlots<'view, 'source>> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(self.slots)
    }

    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        slots: &'slots SourceSlots<'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(program_headers())?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let count = source.root_count(out.budget)?;
        let mut total = 0usize;
        for root in 0..count {
            out.budget.charge_work(1)?;
            total = total
                .checked_add(plan.root(root, out)?.instances.len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut functions = vector(total, out)?;
        let mut roots = vector(count, out)?;
        let mut locals = 0usize;
        for root in 0..count {
            let scope = plan.root(root, out)?;
            if scope.instances.start != functions.len() {
                return Err(mismatch());
            }
            let mut entry = None;
            for instance in 0..scope.instances.len() {
                out.budget.charge_work(3)?;
                let row = plan.instance(root, instance, out)?;
                if row.locals.start != locals {
                    return Err(mismatch());
                }
                locals = row.locals.end;
                if row.active {
                    let function = SourceByteFunction::derive(plan, slots, root, instance, out)?;
                    if instance == 0 {
                        entry = Some(function.enter.entry());
                    }
                    functions.push(Some(function));
                } else {
                    functions.push(None);
                }
            }
            roots.push((
                scope.instances.clone(),
                scope.function.index(),
                entry.ok_or_else(mismatch)?,
            ));
        }
        if functions.len() != total {
            return Err(mismatch());
        }
        Ok(Self {
            slots,
            functions,
            roots,
            locals,
            required: out.budget.storage(),
        })
    }

    pub(super) fn emit(&mut self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        write!(
            out,
            "spec fn invocation_source_abort_site_v50(pc: int) -> bool {{ false"
        )
        .map_err(|_| out.error())?;
        for function in &self.functions {
            out.budget.charge_work(1)?;
            if let Some(function) = function {
                for (block, row) in function.control.iter().enumerate() {
                    out.budget.charge_work(1)?;
                    if matches!(row.end, End::Abort) {
                        let pc = function
                            .blocks
                            .start
                            .checked_add(block)
                            .ok_or(Resource::Arithmetic)?;
                        write!(out, " || pc == {pc}int").map_err(|_| out.error())?;
                    }
                }
            }
        }
        write!(out, " }}\n").map_err(|_| out.error())?;
        for function in &mut self.functions {
            out.budget.charge_work(1)?;
            if let Some(function) = function {
                function.emit(out)?;
            }
        }
        for (root, (range, owner, entry)) in self.roots.iter().enumerate() {
            out.budget.charge_work(1)?;
            let locals = self.locals;
            write!(out, "spec fn invocation_source_byte_initial_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37, little_endian: bool) -> InvocationSourceByteStateV36 {{\n let native = invocation_runtime_execution_{root}_v37(execution) && byte_memory_well_formed_v30(external) && byte_native_view_inputs_v38(external, arguments) && invocation_native_provenance_v39(external, arguments) && (forall|argument: int| 0 <= argument < arguments.len() ==> invocation_source_external_argument_v36(arguments[argument]));\n let entered_memory = if native {{ ByteMemoryV30 {{ view_contracts: invocation_source_view_contracts_0_v39(little_endian), ..external }} }} else {{ external }};\n let source = InvocationSourceByteStateV36 {{ machine: MemoryStateV30 {{ pc: {entry}, values: Seq::new({locals}nat, |i: int| MemoryValueV30::Undefined), memory: entered_memory, generations: Map::empty(), frames: byte_root_frame_with_execution_v37({owner}, execution), valid: native }}, slots: Map::empty(), objects: Map::empty(), logical: invocation_source_logical_initial_v38({locals}nat) }};\n let source_arguments = Seq::new(arguments.len(), |i: int| InvocationSourceValueV42::Carrier(arguments[i]));\n invocation_source_enter_{root}_0_v36(source, source_arguments, little_endian)\n}}\nspec fn invocation_source_byte_block_{root}_v36(source: InvocationSourceByteStateV36, little_endian: bool) -> InvocationSourceBlockResultV36 {{\n").map_err(|_| out.error())?;
            for (instance, function) in self.functions[range.clone()].iter().enumerate() {
                out.budget.charge_work(1)?;
                if let Some(function) = function {
                    write!(out, " if {} <= source.machine.pc < {} {{ invocation_source_block_{root}_{instance}_v36(source, little_endian) }} else", function.blocks.start, function.blocks.end).map_err(|_| out.error())?;
                }
            }
            write!(out, " {{ invocation_source_block_refused_v36(InvocationSourceMicroStateV36 {{ source, next_statement: 0, observations: seq![] }}) }}\n}}\nspec fn invocation_source_byte_trace_{root}_v36(source: InvocationSourceByteStateV36, fuel: nat, little_endian: bool) -> Seq<InvocationSourceBlockResultV36>\n decreases fuel\n{{ if fuel == 0 || !source.machine.valid || source.machine.pc < 0 {{ seq![] }} else {{ let next = invocation_source_byte_block_{root}_v36(source, little_endian); seq![next] + invocation_source_byte_trace_{root}_v36(next.source, (fuel - 1) as nat, little_endian) }} }}\n").map_err(|_| out.error())?;
        }
        Ok(())
    }
}

fn program_headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceByteProgram<'_, '_, '_>>()
        + h::<Vec<Option<SourceByteFunction<'_, '_, '_>>>>()
        + h::<Vec<(Range<usize>, u32, usize)>>()
        + h::<Option<usize>>()
        + h::<Range<usize>>()
        + 14 * size_of::<usize>()
        + 12 * size_of::<&()>()
        + abort_headers()
}

fn abort_headers() -> usize {
    size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, BodyBlock>>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>()
        + 2 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_abort_control_v50_tests.rs"]
mod abort_tests;

#[cfg(test)]
#[path = "original_semantic_mir_source_trap_control_v55_tests.rs"]
mod trap_tests;

fn source_trap_call_v55(
    call: &Call,
    callables: &[Callable],
    types: &[TypeDecl],
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(18)?;
    let Some(callable) = callables.get(call.callee().index() as usize) else {
        return Err(mismatch());
    };
    let Callable::CompilerIntrinsic {
        binding,
        operation: Intrinsic::Trap,
        ..
    } = callable
    else {
        return Ok(false);
    };
    // The admitted original callable owns the no-return semantics. There is no
    // invented destination, return edge, helper frame, or operand observation.
    if !call.arguments().is_empty()
        || !call.variadic_argument_abis().is_empty()
        || call.destination().is_some()
        || call.unwind() != Unwind::Unreachable
        || !binding.abi().source_input_types().is_empty()
        || !binding.abi().arguments().is_empty()
        || binding.abi().c_variadic()
        || !matches!(
            types
                .get(binding.abi().return_type().index() as usize)
                .map(TypeDecl::shape),
            Some(TypeShape::Never)
        )
    {
        return Err(mismatch());
    }
    Ok(true)
}

fn trap_headers_v55() -> usize {
    size_of::<(
        &Call,
        &[Callable],
        &[TypeDecl],
        &fe2o3_mir_model::semantic_mir_v1::SemanticNonBodyCallableBindingV1,
    )>() + size_of::<Option<&Callable>>()
        + size_of::<Option<&TypeDecl>>()
        + size_of::<Option<&TypeShape>>()
        + size_of::<Result<bool>>()
        + size_of::<bool>()
}

fn mismatch() -> Error {
    Error::Statement("original MIR byte control differs from its exact invocation")
}
fn unsupported() -> Error {
    Error::Statement("original MIR byte invocation control is not modeled")
}

impl<'slots, 'view, 'source> SourceByteFunction<'slots, 'view, 'source> {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        slots: &'slots SourceSlots<'view, 'source>,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let row = plan.instance(root, instance, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let ssa = source
            .source_ssa(out.budget)?
            .plan_for_function(row.function)
            .ok_or_else(mismatch)?
            .plan();
        out.budget.charge_work(4)?;
        if !row.active
            || row.blocks.len() != function.blocks().len()
            || ssa.resources().input_blocks() != function.blocks().len()
        {
            return Err(mismatch());
        }
        let body = SourceByteBody::derive(plan, slots, root, instance, out)?;
        let scalar = SourceScalarStatements::new(plan, slots, root, instance, out)?;
        let enter = SourceFrameEnter::derive(plan, slots, root, instance, out)?;
        let returned = SourceFrameReturn::derive(plan, slots, root, instance, out)?;
        let mut control = vector(function.blocks().len(), out)?;
        let calls = plan.calls(root, instance, out)?;
        let mut call_at = 0usize;
        for (block, declaration) in function.blocks().iter().enumerate() {
            out.budget.charge_work(4)?;
            let call_row = if matches!(
                declaration.terminator().kind(),
                Terminator::Call(_) | Terminator::TailCall(_) | Terminator::Drop { .. }
            ) {
                let call = calls.get(call_at).ok_or_else(mismatch)?;
                call_at = call_at.checked_add(1).ok_or(Resource::Arithmetic)?;
                if call.caller != instance || call.block.index() as usize != block {
                    return Err(mismatch());
                }
                Some(call)
            } else {
                None
            };
            let reachable = ssa
                .live_in(Block::new(
                    u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
                ))
                .is_some();
            let target = |local: u32| -> Result<usize> {
                if local as usize >= row.blocks.len() {
                    return Err(mismatch());
                }
                row.blocks
                    .start
                    .checked_add(local as usize)
                    .ok_or_else(|| Resource::Arithmetic.into())
            };
            let end = if !reachable {
                End::Unreachable
            } else {
                match declaration.terminator().kind() {
                    Terminator::Goto(edge) => End::Goto(target(edge.target().index())?),
                    Terminator::SwitchInt { targets, .. } => {
                        let operand = body.switch_operand(block, out)?;
                        let scalar = operand
                            .scalar()
                            .filter(|scalar| *scalar != ScalarV30::Unit)
                            .ok_or_else(unsupported)?;
                        let mut cases = vector(targets.values().len(), out)?;
                        for case in targets.values() {
                            out.budget.charge_work(2)?;
                            if case.value() >= 1u128 << scalar.width() {
                                return Err(mismatch());
                            }
                            cases.push((case.value(), target(case.edge().target().index())?));
                        }
                        End::Switch {
                            operand,
                            cases,
                            otherwise: target(targets.otherwise().target().index())?,
                        }
                    }
                    Terminator::Assert { target: edge, .. } => {
                        End::Assert(assertions::SourceAssertControlV40::derive(
                            &body,
                            block,
                            declaration.terminator().kind(),
                            target(edge.target().index())?,
                            out,
                        )?)
                    }
                    Terminator::Call(call)
                        if matches!(
                            semantic.callables().get(call.callee().index() as usize),
                            Some(Callable::CompilerIntrinsic { .. })
                        ) =>
                    {
                        let call_row = call_row.ok_or_else(mismatch)?;
                        if call_row.kind != CallKind::Direct
                            || !call_row.ssa_reachable
                            || call_row.child.is_some()
                            || call_row.callable != call.callee()
                        {
                            return Err(mismatch());
                        }
                        if source_trap_call_v55(call, semantic.callables(), semantic.types(), out)?
                        {
                            End::Abort
                        } else if let Some(write) = thread_write::ThreadWriteCall::derive(
                            slots,
                            plan,
                            &body,
                            root,
                            instance,
                            block,
                            call,
                            &semantic.callables()[call.callee().index() as usize],
                            out,
                        )? {
                            End::ThreadWrite(write)
                        } else if let Some(descriptor) = descriptor_calls::DescriptorCall::derive(
                            slots,
                            plan,
                            root,
                            instance,
                            block,
                            call,
                            &semantic.callables()[call.callee().index() as usize],
                            out,
                        )? {
                            End::Descriptor(descriptor)
                        } else if let Some(index) = descriptor_indices::DescriptorIndexCall::derive(
                            slots,
                            plan,
                            root,
                            instance,
                            block,
                            call,
                            &semantic.callables()[call.callee().index() as usize],
                            out,
                        )? {
                            End::DescriptorIndex(index)
                        } else {
                            End::Index(index_calls::IndexCall::derive(
                                slots,
                                plan,
                                root,
                                instance,
                                block,
                                call,
                                &semantic.callables()[call.callee().index() as usize],
                                out,
                            )?)
                        }
                    }
                    Terminator::Call(call) => {
                        let call_row = call_row.ok_or_else(mismatch)?;
                        let child = call_row.child.ok_or_else(unsupported)?;
                        let child_row = plan.instance(root, child, out)?;
                        let callee = semantic
                            .functions()
                            .get(child_row.function.index() as usize)
                            .ok_or_else(mismatch)?;
                        out.budget.charge_work(9)?;
                        let destination = call.destination().ok_or_else(unsupported)?;
                        if call_row.kind != CallKind::Direct
                            || !call_row.ssa_reachable
                            || !child_row.active
                            || child_row.incoming != Some((instance, call_row.block))
                            || call_row.callable != call.callee()
                            || !matches!(semantic.callables().get(call.callee().index() as usize), Some(Callable::Defined { function }) if *function == child_row.function)
                            || call.unwind() != Unwind::Unreachable
                            || !call.variadic_argument_abis().is_empty()
                            || call.arguments().len() != callee.abi().source_input_types().len()
                            || destination.edge().role() != EdgeRole::CallReturn
                            || destination.place().ty() != callee.abi().return_type()
                        {
                            return Err(unsupported());
                        }
                        target(destination.edge().target().index())?;
                        let mut arguments = vector(call.arguments().len(), out)?;
                        for (argument, ty) in callee.abi().source_input_types().iter().enumerate() {
                            out.budget.charge_work(2)?;
                            let operand = body.invocation_argument(plan, block, argument, out)?;
                            if operand.ty() != *ty {
                                return Err(mismatch());
                            }
                            arguments.push(operand);
                        }
                        End::Call { child, arguments }
                    }
                    Terminator::Return => End::Return,
                    Terminator::Abort => End::Abort,
                    _ => return Err(unsupported()),
                }
            };
            control.push(BodyBlock {
                statements: declaration.statements().len(),
                end,
            });
        }
        if call_at != calls.len() || control.len() != row.blocks.len() {
            return Err(mismatch());
        }
        Ok(Self {
            slots,
            root,
            instance,
            blocks: row.blocks.clone(),
            body,
            scalar,
            enter,
            returned,
            control,
            required: out.budget.storage(),
        })
    }

    pub(super) fn emit(&mut self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        self.enter.emit(out)?;
        self.returned.emit(out)?;
        self.body.emit(out)?;
        for (block, row) in self.control.iter().enumerate() {
            out.budget.charge_work(1)?;
            for statement in 0..row.statements {
                out.budget.charge_work(1)?;
                if self.body.event_at(block, statement, out)? == Event::Scalar {
                    self.scalar.emit_statement(block, statement, out)?;
                }
            }
        }
        let (r, i) = (self.root, self.instance);
        write!(
            out,
            "spec fn invocation_source_statement_count_{r}_{i}_v36(pc: int) -> int {{\n"
        )
        .map_err(|_| out.error())?;
        for (block, row) in self.control.iter().enumerate() {
            out.budget.charge_work(1)?;
            if !matches!(row.end, End::Unreachable) {
                write!(
                    out,
                    " if pc == {} {{ {}int }} else",
                    self.blocks.start + block,
                    row.statements
                )
                .map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ -1int }}\n}}\nspec fn invocation_source_micro_begin_{r}_{i}_v36(source: InvocationSourceByteStateV36) -> InvocationSourceMicroStateV36 {{ InvocationSourceMicroStateV36 {{ source: if invocation_source_active_{r}_{i}_v36(source) && invocation_source_statement_count_{r}_{i}_v36(source.machine.pc) >= 0 {{ source }} else {{ invocation_source_byte_refused_v36(source) }}, next_statement: 0, observations: seq![] }} }}\nspec fn invocation_source_micro_step_{r}_{i}_v36(cursor: InvocationSourceMicroStateV36, little_endian: bool) -> InvocationSourceMicroStateV36 {{\n if !invocation_source_active_{r}_{i}_v36(cursor.source) || cursor.next_statement < 0 || cursor.next_statement != cursor.observations.len() {{ invocation_source_micro_refused_v36(cursor) }} else {{\n").map_err(|_| out.error())?;
        for (block, row) in self.control.iter().enumerate() {
            out.budget.charge_work(1)?;
            if matches!(row.end, End::Unreachable) {
                continue;
            }
            for statement in 0..row.statements {
                out.budget.charge_work(1)?;
                write!(out, " if cursor.source.machine.pc == {} && cursor.next_statement == {statement} {{\n let after = ", self.blocks.start + block).map_err(|_| out.error())?;
                if self.body.event_at(block, statement, out)? == Event::Scalar {
                    write!(
                        out,
                        "invocation_source_scalar_{r}_{i}_{block}_{statement}_v36(cursor.source)"
                    )
                    .map_err(|_| out.error())?;
                } else {
                    write!(out, "match invocation_source_byte_event_{r}_{i}_v36({block}, {statement}) {{ Some(event) => invocation_source_byte_step_v36(cursor.source, event, {r}, {i}, little_endian), None => invocation_source_byte_refused_v36(cursor.source) }}").map_err(|_| out.error())?;
                }
                write!(out, ";\n invocation_source_micro_record_v36(cursor, after, {r}, {i}, {block}, {statement}, invocation_source_byte_event_{r}_{i}_v36({block}, {statement}))\n }} else").map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ invocation_source_micro_refused_v36(cursor) }}\n }}\n}}\nspec fn invocation_source_micro_finish_{r}_{i}_v36(cursor: InvocationSourceMicroStateV36, little_endian: bool) -> InvocationSourceBlockResultV36 {{\n if !invocation_source_active_{r}_{i}_v36(cursor.source) || cursor.next_statement != invocation_source_statement_count_{r}_{i}_v36(cursor.source.machine.pc) || cursor.next_statement != cursor.observations.len() {{ invocation_source_block_refused_v36(cursor) }} else {{\n").map_err(|_| out.error())?;
        for (block, row) in self.control.iter().enumerate() {
            out.budget.charge_work(1)?;
            if matches!(row.end, End::Unreachable) {
                continue;
            }
            write!(
                out,
                " if cursor.source.machine.pc == {} {{\n",
                self.blocks.start + block
            )
            .map_err(|_| out.error())?;
            match &row.end {
                End::Index(call) => call.emit(out)?,
                End::Descriptor(call) => call.emit(out)?,
                End::DescriptorIndex(call) => call.emit(out)?,
                End::ThreadWrite(call) => call.emit(r, i, block, out)?,
                End::Assert(assertion) => assertion.emit(r, i, block, out)?,
                End::Abort => {
                    write!(out, " let source = invocation_source_byte_trap_v40(cursor.source);\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n").map_err(|_| out.error())?;
                }
                End::Goto(target) => {
                    write!(out, " let source = invocation_source_byte_pc_v36(cursor.source, {target});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n").map_err(|_| out.error())?;
                }
                End::Switch {
                    operand,
                    cases,
                    otherwise,
                } => {
                    write!(
                        out,
                        " let evaluated = invocation_source_operand_evaluate_v36(cursor.source, "
                    )
                    .map_err(|_| out.error())?;
                    operand.emit(out)?;
                    write!(out, ", {r}, {i}, little_endian);\n let source = match evaluated.value {{ MemoryValueV30::Scalar(value) => invocation_source_byte_pc_v36(evaluated.source, ").map_err(|_| out.error())?;
                    for (value, target) in cases {
                        out.budget.charge_work(1)?;
                        write!(out, "if value == {value} {{ {target}int }} else ")
                            .map_err(|_| out.error())?;
                    }
                    write!(out, "{{ {otherwise}int }}), _ => invocation_source_byte_refused_v36(evaluated.source) }};\n let operands = seq![InvocationSourceOperandObservationV36 {{ root: {r}, instance: {i}, block: {block}, role: InvocationSourceOperandRoleV36::Switch, operand: ").map_err(|_| out.error())?;
                    operand.emit(out)?;
                    write!(out, ", before: cursor.source, after: evaluated.source, value: InvocationSourceValueV42::Carrier(evaluated.value) }}];\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands, returned: None }}\n").map_err(|_| out.error())?;
                }
                End::Call { child, arguments } => {
                    write!(out, " let source = cursor.source;\n").map_err(|_| out.error())?;
                    for (argument, operand) in arguments.iter().enumerate() {
                        out.budget.charge_work(1)?;
                        write!(out, " let evaluated_{argument} = invocation_source_value_evaluate_v42(source, ").map_err(|_| out.error())?;
                        operand.emit(out)?;
                        write!(out, ", {r}, {i}, little_endian);\n let source = evaluated_{argument}.source;\n").map_err(|_| out.error())?;
                    }
                    write!(out, " let arguments = seq![").map_err(|_| out.error())?;
                    for argument in 0..arguments.len() {
                        out.budget.charge_work(1)?;
                        if argument != 0 {
                            write!(out, ", ").map_err(|_| out.error())?;
                        }
                        write!(out, "evaluated_{argument}.value").map_err(|_| out.error())?;
                    }
                    write!(out, "];\n let operands = seq![").map_err(|_| out.error())?;
                    for (argument, operand) in arguments.iter().enumerate() {
                        out.budget.charge_work(1)?;
                        write!(out, "InvocationSourceOperandObservationV36 {{ root: {r}, instance: {i}, block: {block}, role: InvocationSourceOperandRoleV36::CallArgument({argument}), operand: ").map_err(|_| out.error())?;
                        operand.emit(out)?;
                        write!(out, ", before: ").map_err(|_| out.error())?;
                        if argument == 0 {
                            write!(out, "cursor.source").map_err(|_| out.error())?;
                        } else {
                            write!(out, "evaluated_{}.source", argument - 1)
                                .map_err(|_| out.error())?;
                        }
                        write!(out, ", after: evaluated_{argument}.source, value: evaluated_{argument}.value }},").map_err(|_| out.error())?;
                    }
                    write!(out, "];\n let source = invocation_source_enter_{r}_{child}_v36(source, arguments, little_endian);\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands, returned: None }}\n").map_err(|_| out.error())?;
                }
                End::Return => {
                    write!(out, " let result = invocation_source_return_{r}_{i}_v36(cursor.source, little_endian);\n InvocationSourceBlockResultV36 {{ source: result.source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: Some(result.returned) }}\n").map_err(|_| out.error())?;
                }
                End::Unreachable => unreachable!(),
            }
            write!(out, " }} else").map_err(|_| out.error())?;
        }
        write!(out, " {{ invocation_source_block_refused_v36(cursor) }}\n }}\n}}\nspec fn invocation_source_micro_run_{r}_{i}_v36(cursor: InvocationSourceMicroStateV36, fuel: nat, little_endian: bool) -> InvocationSourceMicroStateV36\n decreases fuel\n{{ if fuel == 0 || !cursor.source.machine.valid {{ cursor }} else {{ invocation_source_micro_run_{r}_{i}_v36(invocation_source_micro_step_{r}_{i}_v36(cursor, little_endian), (fuel - 1) as nat, little_endian) }} }}\nspec fn invocation_source_block_{r}_{i}_v36(source: InvocationSourceByteStateV36, little_endian: bool) -> InvocationSourceBlockResultV36 {{ let cursor = invocation_source_micro_begin_{r}_{i}_v36(source); let count = invocation_source_statement_count_{r}_{i}_v36(source.machine.pc); if count < 0 {{ invocation_source_block_refused_v36(cursor) }} else {{ invocation_source_micro_finish_{r}_{i}_v36(invocation_source_micro_run_{r}_{i}_v36(cursor, count as nat, little_endian), little_endian) }} }}\n").map_err(|_| out.error())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceByteFunction<'_, '_, '_>>()
        + h::<End>()
        + h::<BodyBlock>()
        + h::<Vec<BodyBlock>>()
        + h::<Vec<(u128, usize)>>()
        + h::<Vec<TypedOperand>>()
        + h::<TypedOperand>()
        + assertions::headers()
        + trap_headers_v55()
        + h::<Range<usize>>()
        + 32 * size_of::<usize>()
        + 28 * size_of::<&()>()
}

pub(super) const SOURCE_FUNCTION_V36: &str = r#"
spec fn invocation_source_external_argument_v36(value: MemoryValueV30) -> bool {
    match value {
        MemoryValueV30::Pointer(pointer) => !invocation_private_allocation_v36(pointer.allocation),
        MemoryValueV30::Slice(slice) => !invocation_private_allocation_v36(slice.pointer.allocation),
        MemoryValueV30::Undefined => false,
        _ => true,
    }
}
struct InvocationSourceStatementObservationV36 {
    root: int,
    instance: int,
    block: int,
    statement: int,
    event: Option<InvocationSourceByteEventV36>,
    before: InvocationSourceByteStateV36,
    after: InvocationSourceByteStateV36,
}
struct InvocationSourceMicroStateV36 {
    source: InvocationSourceByteStateV36,
    next_statement: int,
    observations: Seq<InvocationSourceStatementObservationV36>,
}
enum InvocationSourceOperandRoleV36 {
    Switch,
    CallArgument(int),
    AssertCondition,
    AssertMessage(int),
}
struct InvocationSourceOperandObservationV36 {
    root: int,
    instance: int,
    block: int,
    role: InvocationSourceOperandRoleV36,
    operand: InvocationSourceOperandV36,
    before: InvocationSourceByteStateV36,
    after: InvocationSourceByteStateV36,
    value: InvocationSourceValueV42,
}
struct InvocationSourceBlockResultV36 {
    source: InvocationSourceByteStateV36,
    before_control: InvocationSourceByteStateV36,
    observations: Seq<InvocationSourceStatementObservationV36>,
    operands: Seq<InvocationSourceOperandObservationV36>,
    returned: Option<InvocationSourceValueV42>,
}
spec fn invocation_source_byte_pc_v36(source: InvocationSourceByteStateV36, pc: int) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || source.machine.pc < 0
        || !invocation_source_byte_state_well_formed_v36(source) || pc < 0 {
        invocation_source_byte_refused_v36(source)
    } else { InvocationSourceByteStateV36 { machine: MemoryStateV30 { pc,
        values: source.machine.values, memory: source.machine.memory, generations: source.machine.generations,
        frames: source.machine.frames, valid: true }, ..source } }
}
spec fn invocation_source_byte_trap_v40(source: InvocationSourceByteStateV36) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || source.machine.pc < 0
        || !invocation_source_byte_state_well_formed_v36(source) {
        invocation_source_byte_refused_v36(source)
    } else { InvocationSourceByteStateV36 {
        machine: MemoryStateV30 { pc: -2, ..source.machine }, ..source } }
}
spec fn invocation_source_micro_refused_v36(cursor: InvocationSourceMicroStateV36) -> InvocationSourceMicroStateV36 {
    InvocationSourceMicroStateV36 { source: invocation_source_byte_refused_v36(cursor.source),
        next_statement: cursor.next_statement, observations: cursor.observations }
}
spec fn invocation_source_micro_record_v36(cursor: InvocationSourceMicroStateV36,
    after: InvocationSourceByteStateV36, root: int, instance: int, block: int, statement: int,
    event: Option<InvocationSourceByteEventV36>,
) -> InvocationSourceMicroStateV36 {
    InvocationSourceMicroStateV36 { source: after, next_statement: cursor.next_statement + 1,
        observations: cursor.observations.push(InvocationSourceStatementObservationV36 {
            root, instance, block, statement, event, before: cursor.source, after }) }
}
spec fn invocation_source_block_refused_v36(cursor: InvocationSourceMicroStateV36) -> InvocationSourceBlockResultV36 {
    InvocationSourceBlockResultV36 { source: invocation_source_byte_refused_v36(cursor.source),
        before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }
}
"#;

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    const LIMIT: usize = 100_000_000;

    pub(in super::super) fn with_slots(
        plan: &InvocationPlan<'_, '_>,
        out: &mut Writer<'_, '_>,
        examine: impl FnOnce(&SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
    ) -> Result<()> {
        let source = plan.source(out)?;
        let owner = source.canonical(out.budget)?;
        let (inventory, receipt) =
            super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
        out.budget.reserve_storage(receipt.retained_storage())?;
        let result =
            source.with_ranked_correspondence_v18(&inventory, out.budget, |relation, budget| {
                let mut writer = Writer::new(budget)?;
                let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                examine(&slots, &mut writer)
            });
        drop(inventory);
        if result.is_ok() {
            out.budget.release_storage(receipt.retained_storage())?;
        }
        result
    }

    #[test]
    fn original_mir_byte_program_consumes_every_active_instance_and_ordered_call_argument() {
        for unit_return in [false, true] {
            super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, unit_return, |plan, out| {
            with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                assert_eq!(program.roots.len(), 2);
                assert_eq!(program.functions.iter().filter(|row| row.is_some()).count(), 6);
                program.emit(out)?;
                assert!(out.text.contains("let source = evaluated_0.source;"));
                assert!(out.text.contains("let source = evaluated_1.source;"));
                let first = out.text.find("let evaluated_0 = invocation_source_value_evaluate_v42(source,").unwrap();
                let second = out.text[first..].find("let evaluated_1 = invocation_source_value_evaluate_v42(source,").unwrap() + first;
                let enter = out.text[second..].find("let source = invocation_source_enter_0_1_v36(source, arguments, little_endian);").unwrap() + second;
                assert!(first < second && second < enter);
                assert!(out.text.contains("let arguments = seq![evaluated_0.value, evaluated_1.value]"));
                assert!(out.text.contains("role: InvocationSourceOperandRoleV36::CallArgument(0)"));
                assert!(out.text.contains("before: evaluated_0.source, after: evaluated_1.source, value: evaluated_1.value"));
                // Root blocks are empty. Arithmetic uses the scalar graph;
                // assigning Unit uses the typed byte-transfer dispatcher.
                for root in 0..2 {
                    assert!(!out.text.contains(&format!(
                        "invocation_source_scalar_{root}_0_0_0_v36(cursor.source)"
                    )));
                    for instance in 1..=2 {
                        assert_eq!(out.text.matches(&format!(
                            "invocation_source_scalar_{root}_{instance}_0_0_v36(cursor.source)"
                        )).count(), usize::from(!unit_return));
                        assert_eq!(out.text.matches(&format!(
                            "match invocation_source_byte_event_{root}_{instance}_v36(0, 0) {{ Some(event) => invocation_source_byte_step_v36(cursor.source, event, {root}, {instance}, little_endian)"
                        )).count(), usize::from(unit_return));
                    }
                }
                assert!(out.text.contains("cursor.next_statement != cursor.observations.len()"));
                assert!(out.text.contains("let result = invocation_source_return_0_1_v36(cursor.source, little_endian)"));
                assert!(!out.text.contains("assume("));
                assert!(!out.text.contains("target"));
                Ok(())
            })
        }).0.unwrap();
        }
    }

    #[test]
    fn original_mir_standalone_byte_initial_reuses_native_provenance_without_readiness_assumptions()
    {
        super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
            with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                program.emit(out)?;
                assert_eq!(program.roots.len(), 2);
                for root in 0..program.roots.len() {
                    let initial = out
                        .text
                        .split_once(&format!(
                            "spec fn invocation_source_byte_initial_{root}_v36"
                        ))
                        .unwrap()
                        .1
                        .split("spec fn ")
                        .next()
                        .unwrap();
                    assert!(
                        initial.contains("invocation_native_provenance_v39(external, arguments)")
                    );
                    assert!(initial.contains("byte_memory_well_formed_v30(external)"));
                    assert!(initial.contains("byte_native_view_inputs_v38(external, arguments)"));
                    assert!(
                        initial.contains(
                            "invocation_source_external_argument_v36(arguments[argument])"
                        )
                    );
                    assert!(!initial.contains("source_ready"));
                    assert!(!initial.contains("valid: true"));
                    assert!(!initial.contains("target"));
                }
                Ok(())
            })
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_program_executes_genuine_scalar_storage_between_current_state_operations()
    {
        super::super::super::invocations::tests::run_scalar_allocation_variant(
            LIMIT,
            LIMIT,
            |plan, out| {
                with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let bindings =
                        super::super::byte_bindings::SourceByteBindings::derive(slots, out)?;
                    program.emit(out)?;
                    bindings.emit(out)?;
                    assert_eq!(
                        out.text
                            .matches("let entered = invocation_source_byte_activate_v36(")
                            .count(),
                        2
                    );
                    assert!(out.text.contains("InvocationSourceByteValueV36::Read"));
                    assert!(
                        out.text
                            .contains("InvocationSourceByteDestinationV36::Memory")
                    );
                    assert!(out.text.contains(
                        "invocation_source_byte_step_v36(cursor.source, event, 0, 0, little_endian)"
                    ));
                    assert!(
                        out.text
                            .contains("invocation_source_byte_storage_related_0_v36")
                    );
                    assert!(
                        out.text
                            .contains("invocation_source_byte_storage_related_1_v36")
                    );
                    assert!(
                        out.text
                            .contains("invocation == 0 && site == slot.site && generation >= 0")
                    );
                    assert!(!out.text.contains("maximum_generation"));
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_program_exact_and_one_short_resource_replay() {
        let run = |work, storage| {
            super::super::super::invocations::tests::run_variant(
                work,
                storage,
                true,
                |plan, out| {
                    with_slots(plan, out, |slots, out| {
                        SourceByteProgram::derive(plan, slots, out)?.emit(out)
                    })
                },
            )
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        run(measured.1, measured.3).0.unwrap();
        assert!(run(measured.1 - 1, measured.3).0.is_err());
        assert!(run(measured.1, measured.3 - 1).0.is_err());
    }

    #[test]
    fn original_mir_byte_program_retains_exact_before_after_statement_observations() {
        assert!(SOURCE_FUNCTION_V36.contains("before: cursor.source, after"));
        assert!(SOURCE_FUNCTION_V36.contains("next_statement: cursor.next_statement + 1"));
        assert!(SOURCE_FUNCTION_V36.contains("observations: cursor.observations.push("));
        assert!(SOURCE_FUNCTION_V36.contains("before_control: cursor.source"));
        assert!(!SOURCE_FUNCTION_V36.contains("target"));
    }
}
