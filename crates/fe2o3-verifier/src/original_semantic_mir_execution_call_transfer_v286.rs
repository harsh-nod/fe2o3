//! Exclusive call transport binds both original endpoints; it is not a snapshot.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1 as Callable, SemanticLocalIdV1 as Local,
    SemanticLocalRoleV1 as LocalRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct Transfer {
    pub operand: execution_loans::ExecutionOperand,
    root: usize,
    caller: usize,
    block: usize,
    argument: usize,
    caller_owner: u32,
    child: usize,
    child_owner: u32,
    destination: usize,
    pc: usize,
    entry_pc: usize,
    caller_begin: usize,
    caller_end: usize,
    child_begin: usize,
    child_end: usize,
}

impl Transfer {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn for_call(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        caller: usize,
        block: usize,
        argument: usize,
        child: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(28)?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let semantic = source.source_semantic(out.budget)?;
        let parent = plan.instance(root, caller, out)?;
        let next = plan.instance(root, child, out)?;
        let function = semantic
            .functions()
            .get(parent.function.index() as usize)
            .ok_or_else(mismatch)?;
        let callee = semantic
            .functions()
            .get(next.function.index() as usize)
            .ok_or_else(mismatch)?;
        let Some(Terminator::Call(call)) = function
            .blocks()
            .get(block)
            .map(|body| body.terminator().kind())
        else {
            return Err(mismatch());
        };
        if !parent.active
            || !next.active
            || child <= caller
            || next.incoming.map(|(i, b)| (i, b.index() as usize)) != Some((caller, block))
            || parent.locals.len() != function.locals().len()
            || next.locals.len() != callee.locals().len()
            || parent.blocks.len() != function.blocks().len()
            || next.blocks.len() != callee.blocks().len()
            || callee.entry().index() as usize >= next.blocks.len()
            || call.arguments().len() != callee.abi().source_input_types().len()
            || call.unwind()
                != fe2o3_mir_model::semantic_mir_v1::SemanticUnwindActionV1::Unreachable
            || !call.variadic_argument_abis().is_empty()
            || !(parent.locals.end <= next.locals.start || next.locals.end <= parent.locals.start)
            || !matches!(semantic.callables().get(call.callee().index() as usize),
                Some(Callable::Defined { function }) if *function == next.function)
        {
            return Err(mismatch());
        }
        let original = call.arguments().get(argument).ok_or_else(mismatch)?;
        if callee.abi().source_input_types().get(argument) != Some(&original.ty()) {
            return Err(mismatch());
        }
        let operand =
            execution_loans::call_argument(slots, plan, root, caller, block, argument, out)?
                .ok_or_else(mismatch)?;
        if !operand.recipe.mutable || !operand.moved {
            return Err(unsupported());
        }
        let mut parameter = None;
        for (index, local) in callee.locals().iter().enumerate() {
            out.budget.charge_work(2)?;
            if local.role()
                == LocalRole::Argument(u32::try_from(argument).map_err(|_| Resource::Arithmetic)?)
            {
                if local.ty() != original.ty() || parameter.replace(index).is_some() {
                    return Err(mismatch());
                }
            }
        }
        let parameter = parameter.ok_or_else(mismatch)?;
        let local = Local::from_index(u32::try_from(parameter).map_err(|_| Resource::Arithmetic)?);
        if slots.has_original_object(root, child, local.index(), out)?
            || execution_loans::entry_recipe_exact(slots, plan, root, child, local, out)?
                != Some(operand.recipe)
        {
            return Err(unsupported());
        }
        Ok(Self {
            operand,
            root,
            caller,
            block,
            argument,
            caller_owner: parent.function.index(),
            child,
            child_owner: next.function.index(),
            destination: next
                .locals
                .start
                .checked_add(parameter)
                .ok_or(Resource::Arithmetic)?,
            pc: parent
                .blocks
                .start
                .checked_add(block)
                .ok_or(Resource::Arithmetic)?,
            entry_pc: next
                .blocks
                .start
                .checked_add(callee.entry().index() as usize)
                .ok_or(Resource::Arithmetic)?,
            caller_begin: parent.locals.start,
            caller_end: parent.locals.end,
            child_begin: next.locals.start,
            child_end: next.locals.end,
        })
    }

    pub(in super::super) fn for_entry(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        child: usize,
        argument: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let (caller, block) = plan
            .instance(root, child, out)?
            .incoming
            .ok_or_else(mismatch)?;
        Self::for_call(
            slots,
            plan,
            root,
            caller,
            block.index() as usize,
            argument,
            child,
            out,
        )
    }

    pub(in super::super) fn emit_site(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(16)?;
        write!(out, "InvocationSourceExecutionCallSiteV286 {{ root: {}, caller: {}, block: {}, argument: {}, input: {}, caller_owner: {}, child: {}, child_owner: {}, destination: {}, pc: {}, entry_pc: {}, caller_begin: {}, caller_end: {}, child_begin: {}, child_end: {} }}",
            self.root, self.caller, self.block, self.argument, self.operand.local,
            self.caller_owner, self.child, self.child_owner, self.destination, self.pc, self.entry_pc,
            self.caller_begin, self.caller_end, self.child_begin, self.child_end)
            .map_err(|_| out.error())
    }

    pub(in super::super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        write!(
            out,
            "InvocationSourceOperandV36::ExecutionTransfer {{ operand: "
        )
        .map_err(|_| out.error())?;
        self.operand.emit(out)?;
        write!(out, ", site: ").map_err(|_| out.error())?;
        self.emit_site(out)?;
        write!(out, " }}").map_err(|_| out.error())
    }
}

pub(in super::super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Transfer>()
        + h::<Option<usize>>()
        + h::<[usize; 16]>()
        + h::<[&Function; 2]>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticDirectCallV1>()
        + h::<&Operand>()
        + h::<Local>()
        + h::<(
            usize,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
        )>()
}
