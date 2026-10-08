//! Scalar statements use the existing expression interpreter, but each graph
//! reads the current tagged state rather than replaying an earlier block state.

use super::super::{
    ExpressionV30, Function, NodeV30, Operand, Place, Rvalue, ScalarV30, SourceProgramV30,
    Statement, Type, emit_graph_v30, interpreter_headers_v31, invocations::InvocationPlan,
};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use std::{fmt::Write as _, mem::size_of, ops::Range};

type Input = (usize, usize, ScalarV30);

#[derive(Clone, Copy, Debug)]
pub(super) struct ScalarCopyV325 {
    pub(super) destination: usize,
    pub(super) graph: usize,
    pub(super) inputs: [Option<usize>; 2],
    pub(super) unit: bool,
}

pub(super) struct SourceScalarStatements<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    function: usize,
    locals: Range<usize>,
    blocks: Range<usize>,
    program: SourceProgramV30,
    inputs: [Option<Input>; 2],
    moved: [Option<usize>; 2],
    touched: [Option<usize>; 3],
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR scalar byte-step identity differs")
}

fn unsupported() -> Error {
    Error::Statement("original MIR scalar byte-step is not modeled")
}

impl<'slots, 'view, 'source> SourceScalarStatements<'slots, 'view, 'source> {
    pub(super) fn copy_coordinates_v325(
        &self,
        root: usize,
        instance: usize,
        block: usize,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<ScalarCopyV325>> {
        self.check(out)?;
        out.budget.reserve_storage(
            headers()
                + size_of::<ScalarCopyV325>()
                + size_of::<Option<ScalarCopyV325>>()
                + size_of::<Result<Option<ScalarCopyV325>>>()
                + size_of::<[Option<usize>; 2]>()
                + size_of::<std::slice::IterMut<'_, Option<usize>>>()
                + size_of::<Option<&mut Option<usize>>>()
                + size_of::<Result<()>>()
                + 8 * size_of::<usize>()
                + 8 * size_of::<&()>(),
        )?;
        out.budget.charge_work(6)?;
        if self.root != root || self.instance != instance || block >= self.blocks.len() {
            return Err(mismatch());
        }
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(self.function)
            .ok_or_else(mismatch)?;
        let statement = function
            .blocks()
            .get(block)
            .and_then(|body| body.statements().get(ordinal))
            .ok_or_else(mismatch)?;
        let Statement::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        if !matches!(
            assignment.value().kind(),
            Rvalue::Use(_) | Rvalue::Unary { .. } | Rvalue::Binary { .. }
        ) {
            return Ok(None);
        }
        let mut copied = true;
        let mut inputs = [None; 2];
        assignment
            .value()
            .kind()
            .try_visit_operands(|operand| -> Result<()> {
                out.budget.charge_work(1)?;
                copied &= !matches!(operand, Operand::Move(_));
                if let Operand::Copy(place) = operand {
                    let global = self.place(function, place, out)?.1;
                    out.budget.charge_work(2)?;
                    if !inputs.contains(&Some(global)) {
                        *inputs
                            .iter_mut()
                            .find(|value| value.is_none())
                            .ok_or(Resource::Accounting)? = Some(global);
                    }
                }
                Ok(())
            })?;
        if !copied {
            return Ok(None);
        }
        Ok(Some(ScalarCopyV325 {
            destination: self.place(function, assignment.destination(), out)?.1,
            graph: graph_key(
                self.blocks
                    .start
                    .checked_add(block)
                    .ok_or(Resource::Arithmetic)?,
                ordinal,
            )?,
            inputs,
            unit: ScalarV30::from_source(semantic.types(), assignment.destination().ty())?
                == ScalarV30::Unit,
        }))
    }

    pub(super) fn new(
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
        out.budget.charge_work(3)?;
        if !row.active
            || row.locals.len() != function.locals().len()
            || row.blocks.len() != function.blocks().len()
        {
            return Err(mismatch());
        }
        let nodes = vector(3, out)?;
        let assignments = vector(1, out)?;
        let mut locals = vector(function.locals().len(), out)?;
        out.budget.charge_work(function.locals().len())?;
        locals.resize(function.locals().len(), None);
        Ok(Self {
            slots,
            root,
            instance,
            function: row.function.index() as usize,
            locals: row.locals.clone(),
            blocks: row.blocks.clone(),
            program: SourceProgramV30 {
                nodes,
                assignments,
                arguments: 0,
                returned: None,
                statements: 1,
                locals,
            },
            inputs: [None; 2],
            moved: [None; 2],
            touched: [None; 3],
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    fn reset(&mut self, out: &mut Writer<'_, '_>) -> Result<()> {
        // Only the at-most-two operand locals and one destination were touched.
        for local in &mut self.touched {
            out.budget.charge_work(1)?;
            if let Some(local) = local.take() {
                *self.program.locals.get_mut(local).ok_or_else(mismatch)? = None;
            }
        }
        self.program.nodes.clear();
        self.program.assignments.clear();
        self.program.arguments = 0;
        self.inputs = [None; 2];
        self.moved = [None; 2];
        Ok(())
    }

    fn touch(&mut self, local: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        for slot in &mut self.touched {
            out.budget.charge_work(1)?;
            match *slot {
                Some(prior) if prior == local => return Ok(()),
                None => {
                    *slot = Some(local);
                    return Ok(());
                }
                _ => {}
            }
        }
        Err(Resource::Accounting.into())
    }

    fn nonmemory_local(&self, local: usize, out: &mut Writer<'_, '_>) -> Result<usize> {
        out.budget.charge_work(2)?;
        if local >= self.locals.len()
            || self
                .slots
                .legacy_descriptor_by_source(
                    self.root,
                    self.instance,
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                    "source-scalar-nonmemory-local",
                    out,
                )?
                .is_some()
        {
            return Err(unsupported());
        }
        self.locals
            .start
            .checked_add(local)
            .ok_or(Resource::Arithmetic.into())
    }

    fn place(
        &self,
        function: &Function,
        place: &Place,
        out: &mut Writer<'_, '_>,
    ) -> Result<(usize, usize)> {
        out.budget.charge_work(2)?;
        let local = self.program.local(function, place)?;
        Ok((local, self.nonmemory_local(local, out)?))
    }

    fn seed(
        &mut self,
        types: &[Type],
        function: &Function,
        operand: &Operand,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(2)?;
        let scalar = ScalarV30::from_source(types, operand.ty())?;
        let layout = types
            .get(operand.ty().index() as usize)
            .ok_or_else(mismatch)?
            .layout();
        let bytes = match scalar {
            ScalarV30::Unit => 0,
            ScalarV30::Bool => 1,
            ScalarV30::Integer { width, .. } | ScalarV30::Float { width } => u64::from(width / 8),
        };
        if layout.size_bytes() != Some(bytes) {
            return Err(mismatch());
        }
        let place = match operand {
            Operand::Copy(place) | Operand::Move(place) => place,
            Operand::Constant(constant) => {
                if let super::super::Constant::Scalar(value) = constant.value()
                    && u64::from(value.size_bytes()) != bytes
                {
                    return Err(mismatch());
                }
                return Ok(());
            }
        };
        let (local, global) = self.place(function, place, out)?;
        self.touch(local, out)?;
        if self.program.locals[local].is_none() {
            let argument = self.program.arguments;
            if argument >= self.inputs.len() {
                return Err(Resource::Accounting.into());
            }
            let node = self.program.push(
                NodeV30 {
                    scalar,
                    expression: ExpressionV30::Argument(
                        u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
                    ),
                },
                out,
            )?;
            self.program.locals[local] = Some(node);
            self.inputs[argument] = Some((local, global, scalar));
            self.program.arguments += 1;
        }
        if matches!(operand, Operand::Move(_)) {
            let mut inserted = false;
            for slot in &mut self.moved {
                out.budget.charge_work(1)?;
                if slot.is_none() {
                    *slot = Some(global);
                    inserted = true;
                    break;
                }
            }
            if !inserted {
                return Err(Resource::Accounting.into());
            }
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        types: &[Type],
        function: &Function,
        ordinal: usize,
        statement: &Statement,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<usize>> {
        self.reset(out)?;
        out.budget.charge_work(2)?;
        let cleared = match statement {
            Statement::Assign(assignment) => {
                if !matches!(
                    assignment.value().kind(),
                    Rvalue::Use(_) | Rvalue::Unary { .. } | Rvalue::Binary { .. }
                ) {
                    return Err(unsupported());
                }
                let (local, _) = self.place(function, assignment.destination(), out)?;
                self.touch(local, out)?;
                assignment
                    .value()
                    .kind()
                    .try_visit_operands(|operand| self.seed(types, function, operand, out))?;
                None
            }
            Statement::StorageLive(local) | Statement::StorageDead(local) => {
                let local = local.index() as usize;
                let global = self.nonmemory_local(local, out)?;
                self.touch(local, out)?;
                Some(global)
            }
            Statement::Deinitialize(place) => {
                let (local, global) = self.place(function, place, out)?;
                self.touch(local, out)?;
                Some(global)
            }
            Statement::Nop => None,
            _ => return Err(unsupported()),
        };
        // This interpreter checks operand order, including move-then-use.
        self.program
            .statement(types, function, ordinal, statement, out)?;
        Ok(cleared)
    }

    pub(super) fn emit_statement(
        &mut self,
        block: usize,
        ordinal: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let slots = self.slots;
        let source = slots.correspondence(out)?.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        out.budget.charge_work(4)?;
        let function = semantic
            .functions()
            .get(self.function)
            .ok_or_else(mismatch)?;
        let statement = function
            .blocks()
            .get(block)
            .and_then(|body| body.statements().get(ordinal))
            .ok_or_else(mismatch)?;
        let global_block = self
            .blocks
            .start
            .checked_add(block)
            .ok_or(Resource::Arithmetic)?;
        if global_block >= self.blocks.end {
            return Err(mismatch());
        }
        let graph = graph_key(global_block, ordinal)?;
        let cleared = self.prepare(semantic.types(), function, ordinal, statement.kind(), out)?;
        let assignment = self.program.assignments.first().copied();
        if let Some(assignment) = assignment {
            emit_graph_v30(
                &self.program.nodes,
                self.program.arguments,
                graph,
                "invocation_source_scalar",
                std::iter::once(Ok(Some(assignment.value))),
                None,
                out,
            )?;
        }
        write!(out, "spec fn invocation_source_scalar_{}_{}_{}_{}_v36(n: InvocationSourceByteStateV36) -> InvocationSourceByteStateV36 {{\n if !n.machine.valid", self.root, self.instance, block, ordinal).map_err(|_| out.error())?;
        for input in &self.inputs {
            out.budget.charge_work(1)?;
            if let Some((_, global, scalar)) = *input {
                write!(out, " || n.machine.values.len() <= {global} || !invocation_source_byte_value_typed_v36(n.machine.values[{global}], {})", scalar.width()).map_err(|_| out.error())?;
            }
        }
        write!(
            out,
            " {{ invocation_source_byte_refused_v36(n) }} else {{\n"
        )
        .map_err(|_| out.error())?;
        if assignment.is_some() {
            write!(out, " let base = seq![").map_err(|_| out.error())?;
            for input in &self.inputs {
                out.budget.charge_work(1)?;
                if let Some((_, global, _)) = *input {
                    write!(out, "match n.machine.values[{global}] {{ MemoryValueV30::Scalar(v) => v, _ => 0int }},").map_err(|_| out.error())?;
                }
            }
            write!(
                out,
                "];\n let result = original_invocation_source_scalar_trace_{graph}_v30(base)[0];\n"
            )
            .map_err(|_| out.error())?;
        }
        write!(out, " let changed = n;\n").map_err(|_| out.error())?;
        for global in &self.moved {
            out.budget.charge_work(1)?;
            if let Some(global) = *global {
                write!(out, " let changed = invocation_source_byte_put_local_v36(changed, {global}, MemoryValueV30::Undefined);\n").map_err(|_| out.error())?;
            }
        }
        if let Some(assignment) = assignment {
            let global = self
                .locals
                .start
                .checked_add(assignment.destination as usize)
                .ok_or(Resource::Arithmetic)?;
            let scalar = self
                .program
                .nodes
                .get(assignment.value)
                .ok_or_else(mismatch)?
                .scalar;
            let value = if scalar == ScalarV30::Unit {
                "MemoryValueV30::Unit"
            } else {
                "MemoryValueV30::Scalar(result)"
            };
            write!(
                out,
                " invocation_source_byte_put_local_v36(changed, {global}, {value})\n"
            )
            .map_err(|_| out.error())?;
        } else if let Some(global) = cleared {
            write!(out, " invocation_source_byte_put_local_v36(changed, {global}, MemoryValueV30::Undefined)\n").map_err(|_| out.error())?;
        } else {
            write!(out, " changed\n").map_err(|_| out.error())?;
        }
        write!(out, " }}\n}}\n").map_err(|_| out.error())
    }
}

fn graph_key(block: usize, statement: usize) -> Result<usize> {
    let sum = block.checked_add(statement).ok_or(Resource::Arithmetic)?;
    let next = sum.checked_add(1).ok_or(Resource::Arithmetic)?;
    let triangular = if sum % 2 == 0 {
        (sum / 2).checked_mul(next)
    } else {
        sum.checked_mul(next / 2)
    }
    .ok_or(Resource::Arithmetic)?;
    triangular
        .checked_add(statement)
        .ok_or(Resource::Arithmetic.into())
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceScalarStatements<'_, '_, '_>>()
        + h::<SourceProgramV30>()
        + h::<NodeV30>()
        + h::<super::super::AssignmentV30>()
        + h::<std::iter::Once<Result<Option<usize>>>>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<Option<super::super::AssignmentV30>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + interpreter_headers_v31()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const LIMIT: usize = 100_000_000;

    fn run(
        work: usize,
        storage: usize,
        examine: impl FnOnce(&mut SourceScalarStatements<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
    ) -> (Result<()>, usize, usize, usize) {
        super::super::super::invocations::tests::run_variant(work, storage, false, |plan, out| {
            let source = plan.source(out)?;
            let owner = source.canonical(out.budget)?;
            let (inventory, receipt) =
                super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let result = source.with_ranked_correspondence_v18(
                &inventory,
                out.budget,
                |relation, budget| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                    let mut statements =
                        SourceScalarStatements::new(plan, &slots, 0, 1, &mut writer)?;
                    examine(&mut statements, &mut writer)
                },
            );
            drop(inventory);
            if result.is_ok() {
                out.budget.release_storage(receipt.retained_storage())?;
            }
            result
        })
    }

    #[test]
    fn original_mir_byte_scalar_steps_read_current_locals_and_reuse_primitive_graph() {
        run(LIMIT, LIMIT, |statements, out| {
            let start = statements.locals.start;
            statements.emit_statement(0, 0, out)?;
            let split = out.text.len();
            statements.emit_statement(0, 1, out)?;
            let second = &out.text[split..];
            assert!(second.contains(&format!("n.machine.values[{}]", start + 3)));
            assert!(second.contains(&format!("n.machine.values[{}]", start + 1)));
            assert!(second.contains("((m0 as u32) | (m1 as u32)) as int"));
            assert!(!second.contains(" ^ "));
            assert!(second.contains("InvocationSourceByteStateV36"));
            assert_eq!(statements.program.nodes.len(), 3);
            assert_eq!(statements.program.assignments.len(), 1);
            assert_eq!(statements.program.locals.len(), 4);
            assert!(out.text.contains("((m0 as u32) ^ (m1 as u32)) as int"));
            assert!(!out.text.contains("assume("));
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_scalar_steps_refuse_unknown_statement_before_text() {
        run(LIMIT, LIMIT, |statements, out| {
            assert!(statements.emit_statement(99, 0, out).is_err());
            assert!(statements.emit_statement(0, 99, out).is_err());
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }

    fn assign(operation: SemanticBinaryOpV1, moved: bool, right: u32) -> Statement {
        let ty = SemanticTypeIdV1::from_index(0);
        let place =
            |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        Statement::Assign(SemanticAssignmentV1::new(
            place(3),
            SemanticRvalueV1::new(
                ty,
                Rvalue::Binary {
                    operation,
                    left: if moved {
                        Operand::Move(place(1))
                    } else {
                        Operand::Copy(place(1))
                    },
                    right: Operand::Copy(place(right)),
                },
            ),
        ))
    }

    #[test]
    fn original_mir_byte_scalar_steps_preserve_move_order_and_refuse_unmodeled_arithmetic() {
        run(LIMIT, LIMIT, |statements, out| {
            let (types, function) =
                super::super::super::tests::fixture(SemanticBinaryOpV1::BitXor, false);
            let first = assign(SemanticBinaryOpV1::BitXor, true, 2);
            statements.prepare(&types, &function, 0, &first, out)?;
            assert_eq!(statements.moved, [Some(statements.locals.start + 1), None]);
            assert_eq!(statements.program.locals[1], None);
            assert!(statements.program.locals[2].is_some());
            let invalid = assign(SemanticBinaryOpV1::BitXor, true, 1);
            assert!(matches!(
                statements.prepare(&types, &function, 1, &invalid, out),
                Err(Error::Statement("original MIR scalar use is undefined"))
            ));
            let unsupported = assign(SemanticBinaryOpV1::Add, false, 2);
            assert!(matches!(
                statements.prepare(&types, &function, 2, &unsupported, out),
                Err(Error::Statement(
                    "original MIR arithmetic/effect contract is not modeled"
                ))
            ));
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_scalar_steps_clear_dead_locals_and_reset_only_touched_slots() {
        run(LIMIT, LIMIT, |statements, out| {
            let (types, function) =
                super::super::super::tests::fixture(SemanticBinaryOpV1::BitXor, false);
            statements.prepare(
                &types,
                &function,
                0,
                &assign(SemanticBinaryOpV1::BitXor, false, 2),
                out,
            )?;
            let before = (out.budget.work(), out.budget.storage());
            statements.reset(out)?;
            assert_eq!(out.budget.work() - before.0, 3);
            assert_eq!(out.budget.storage(), before.1);
            assert!(statements.program.locals.iter().all(Option::is_none));
            for statement in [
                Statement::StorageLive(SemanticLocalIdV1::from_index(1)),
                Statement::StorageDead(SemanticLocalIdV1::from_index(1)),
                Statement::Deinitialize(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![],
                        SemanticTypeIdV1::from_index(0),
                    )
                    .unwrap(),
                ),
            ] {
                assert_eq!(
                    statements.prepare(&types, &function, 0, &statement, out)?,
                    Some(statements.locals.start + 1)
                );
                assert!(statements.program.assignments.is_empty());
            }
            assert_eq!(
                statements.prepare(&types, &function, 0, &Statement::Nop, out)?,
                None
            );
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_scalar_steps_have_exact_and_one_short_resources() {
        let emit = |statements: &mut SourceScalarStatements<'_, '_, '_>,
                    out: &mut Writer<'_, '_>| {
            statements.emit_statement(0, 0, out)?;
            statements.emit_statement(0, 1, out)
        };
        let measured = run(LIMIT, LIMIT, emit);
        measured.0.unwrap();
        run(measured.1, measured.3, emit).0.unwrap();
        assert!(run(measured.1 - 1, measured.3, emit).0.is_err());
        assert!(run(measured.1, measured.3 - 1, emit).0.is_err());
    }

    #[test]
    fn original_mir_byte_scalar_reset_has_independent_three_work_boundary() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        run(LIMIT, LIMIT, |statements, out| {
            let (types, function) =
                super::super::super::tests::fixture(SemanticBinaryOpV1::BitXor, false);
            for limit in [3, 2] {
                statements.prepare(
                    &types,
                    &function,
                    0,
                    &assign(SemanticBinaryOpV1::BitXor, false, 2),
                    out,
                )?;
                let capacity = (
                    statements.program.nodes.capacity(),
                    statements.program.assignments.capacity(),
                    statements.program.locals.capacity(),
                );
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, SOURCE_LIMIT + 17);
                budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
                let mut writer = Writer::new(&mut budget).unwrap();
                let before = writer.budget.storage();
                let result = statements.reset(&mut writer);
                if limit == 3 {
                    result.unwrap();
                    assert_eq!(writer.budget.work(), 3);
                    assert!(statements.program.locals.iter().all(Option::is_none));
                } else {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                }
                assert_eq!(writer.budget.storage(), before);
                assert!(writer.text.is_empty());
                assert_eq!(
                    capacity,
                    (
                        statements.program.nodes.capacity(),
                        statements.program.assignments.capacity(),
                        statements.program.locals.capacity(),
                    )
                );
            }
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_scalar_graph_keys_are_unique_and_checked() {
        let mut seen = std::collections::BTreeSet::new();
        for block in 0..64 {
            for statement in 0..64 {
                assert!(seen.insert(graph_key(block, statement).unwrap()));
            }
        }
        assert_eq!(graph_key(0, 0).unwrap(), 0);
        assert_eq!(graph_key(1, 0).unwrap(), 1);
        assert_eq!(graph_key(0, 1).unwrap(), 2);
        assert!(matches!(
            graph_key(usize::MAX, 1),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert!(matches!(
            graph_key(usize::MAX / 2, 0),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }

    #[test]
    fn original_mir_byte_scalar_headers_have_independent_field_envelope() {
        type Fields<'a, 'v, 's> = (
            &'a SourceSlots<'v, 's>,
            usize,
            usize,
            usize,
            Range<usize>,
            Range<usize>,
            SourceProgramV30,
            [Option<Input>; 2],
            [Option<usize>; 2],
            [Option<usize>; 3],
            usize,
        );
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(
            size_of::<SourceScalarStatements<'_, '_, '_>>(),
            size_of::<Fields<'_, '_, '_>>()
        );
        assert_eq!(
            headers(),
            size_of::<Fields<'_, '_, '_>>()
                + 2 * size_of::<Result<SourceScalarStatements<'_, '_, '_>>>()
                + h::<SourceProgramV30>()
                + h::<NodeV30>()
                + h::<super::super::super::AssignmentV30>()
                + h::<std::iter::Once<Result<Option<usize>>>>()
                + h::<Range<usize>>()
                + h::<Option<usize>>()
                + h::<Option<super::super::super::AssignmentV30>>()
                + 32 * size_of::<usize>()
                + 24 * size_of::<&()>()
                + interpreter_headers_v31()
        );
    }
}
