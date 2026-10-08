use super::*;

const MAX_GROUP_LOADS: usize = 8;
const MAX_GROUP_OPERATIONS: usize = 256;

/// Experimental scalar-load scheduling for the ordinary gfx950 module route.
///
/// This is explicitly opt-in and does not change the default, anchored, ordered,
/// debug-line, native BF16 or physical lowering routes. It is not a performance
/// or execution-safety certificate; actual ISA and native parity are still required.
pub fn lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_checked_load_grouping_v1(
    module: &Module,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_with_checked_load_context_v1(
        module,
        LoweringTarget::Gfx950XnackMinusV1,
        None,
        None,
        true,
        None,
        None,
        None,
        None,
        true,
    )
}

pub(super) struct CheckedLoadGroupV1 {
    pub(super) end: usize,
    loads: Vec<usize>,
}

impl FunctionLowerer<'_> {
    pub(super) fn checked_load_group_v1(
        &self,
        block: &BasicBlock,
        start: usize,
    ) -> Option<CheckedLoadGroupV1> {
        if !self.checked_load_grouping_v1
            || !matches!(
                self.semantic_anchor_emission,
                SemanticAnchorEmissionV1::Disabled
            )
            || self.ordered_region_v16
            || self.ordered_program_v17
            || self.ordered_composition_v1.is_some()
            || self.ordered_debug_line_v17.is_some()
            || self.native_bf16_helper.is_some()
            || !self.groupable_load_v1(&block.operations[start])
        {
            return None;
        }
        let mut loads = Vec::with_capacity(MAX_GROUP_LOADS);
        let mut loaded_values = BTreeSet::new();
        let mut end = start;
        for (index, operation) in block
            .operations
            .iter()
            .enumerate()
            .skip(start)
            .take(MAX_GROUP_OPERATIONS)
        {
            let mut dependent = false;
            operation.visit_operands(|operand| dependent |= loaded_values.contains(&operand));
            if dependent {
                break;
            }
            if self.groupable_load_v1(operation) {
                loads.push(index);
                loaded_values.extend(operation.result_ids());
                end = index + 1;
                if loads.len() == MAX_GROUP_LOADS {
                    break;
                }
            } else if !self.groupable_address_operation_v1(operation) {
                break;
            }
        }
        (loads.len() >= 2).then_some(CheckedLoadGroupV1 { end, loads })
    }

    fn groupable_load_v1(&self, operation: &Operation) -> bool {
        let OperationKind::GuardedLoad {
            pointer, access, ..
        } = &operation.kind
        else {
            return false;
        };
        !access.volatile
            && access.address_space == KernelAddressSpace::Global
            && matches!(self.value_type(*pointer), Type::Pointer(pointer)
                if pointer.address_space == KernelAddressSpace::Global
                    && pointer.access == AccessMode::ReadOnly
                    && pointer.pointee.as_scalar().is_some())
    }

    fn groupable_address_operation_v1(&self, operation: &Operation) -> bool {
        let integer = |value| {
            self.value_type(value)
                .as_scalar()
                .is_some_and(|scalar| scalar.is_integer() || scalar == ScalarType::Bool)
        };
        match &operation.kind {
            OperationKind::Constant(_)
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            // The ordinary emitter uses non-inbounds GEP, including masked empty slices.
            | OperationKind::GetElementPointer { .. } => true,
            OperationKind::Unary { operand, .. } => integer(*operand),
            OperationKind::Binary { op, lhs, .. } => integer(*lhs) && matches!(op,
                BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply
                    | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
                    | BinaryOp::Checked(_)),
            OperationKind::Compare { lhs, .. } => integer(*lhs),
            OperationKind::Select { true_value, .. } => integer(*true_value)
                || matches!(self.value_type(*true_value), Type::Pointer(_)),
            OperationKind::Cast { kind: CastKind::RestrictPointerAccess, .. } => true,
            OperationKind::Cast { kind, value, to } => integer(*value)
                && to.as_scalar().is_some_and(|scalar| scalar.is_integer() || scalar == ScalarType::Bool)
                && matches!(kind, CastKind::Truncate | CastKind::ZeroExtend | CastKind::SignExtend | CastKind::Bitcast),
            // In particular: no floating arithmetic, divide, shift, call, load,
            // marker, collective, barrier, atomic, allocation or inline assembly.
            _ => false,
        }
    }

    pub(super) fn emit_checked_load_group_v1(
        &self,
        output: &mut dyn fmt::Write,
        block: &BasicBlock,
        group: &CheckedLoadGroupV1,
    ) -> Result<(), LoweringErrors> {
        let first = group.loads[0];
        let last = *group.loads.last().expect("nonempty checked-load group");
        let prefix = format!("checked_load_bb{}_op{first}", block.id.0);
        let fast = format!("{prefix}_fast");
        let slow = format!("{prefix}_slow");
        // Retain the original last-load exit label, including successor/backedge PHIs.
        let merge = guarded_load_merge_label(block.id, last);
        for index in first..group.end {
            if !group.loads.contains(&index) {
                self.emit_operation(output, block.id, index, &block.operations[index])?;
            }
        }
        let mut predicate = String::new();
        for (ordinal, index) in group.loads.iter().enumerate() {
            let OperationKind::GuardedLoad {
                predicate: guard, ..
            } = &block.operations[*index].kind
            else {
                unreachable!("planned guarded load")
            };
            let (guard, _) = self.value(*guard);
            if ordinal == 0 {
                predicate = guard.to_owned();
            } else {
                let combined = format!("%{prefix}.all{ordinal}");
                writeln!(output, "  {combined} = and i1 {predicate}, {guard}").unwrap();
                predicate = combined;
            }
        }
        writeln!(output, "  br i1 {predicate}, label %{fast}, label %{slow}").unwrap();
        writeln!(output, "{fast}:").unwrap();
        for index in &group.loads {
            let operation = &block.operations[*index];
            let OperationKind::GuardedLoad {
                pointer, access, ..
            } = &operation.kind
            else {
                unreachable!("planned guarded load")
            };
            let (result, ty) = self.value(operation.results[0].id);
            let (pointer, _) = self.value(*pointer);
            writeln!(
                output,
                "  {result}.group_fast = load {}, ptr addrspace(1) {pointer}, align {}",
                llvm_type(ty),
                access.alignment
            )
            .unwrap();
        }
        writeln!(output, "  br label %{merge}").unwrap();
        writeln!(output, "{slow}:").unwrap();
        for index in &group.loads {
            let operation = &block.operations[*index];
            let OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access,
            } = &operation.kind
            else {
                unreachable!("planned guarded load")
            };
            let (result, ty) = self.value(operation.results[0].id);
            let (pointer, _) = self.value(*pointer);
            let (predicate, _) = self.value(*predicate);
            let (fallback, _) = self.value(*fallback);
            let label = format!("{prefix}_slow_op{index}");
            writeln!(
                output,
                "  br i1 {predicate}, label %{label}_true, label %{label}_false"
            )
            .unwrap();
            writeln!(output, "{label}_true:").unwrap();
            writeln!(
                output,
                "  {result}.group_loaded = load {}, ptr addrspace(1) {pointer}, align {}",
                llvm_type(ty),
                access.alignment
            )
            .unwrap();
            writeln!(output, "  br label %{label}_merge").unwrap();
            writeln!(output, "{label}_false:").unwrap();
            writeln!(output, "  br label %{label}_merge").unwrap();
            writeln!(output, "{label}_merge:").unwrap();
            writeln!(output, "  {result}.group_slow = phi {} [ {result}.group_loaded, %{label}_true ], [ {fallback}, %{label}_false ]", llvm_type(ty)).unwrap();
        }
        writeln!(output, "  br label %{merge}").unwrap();
        writeln!(output, "{merge}:").unwrap();
        for index in &group.loads {
            let (result, ty) = self.value(block.operations[*index].results[0].id);
            writeln!(output, "  {result} = phi {} [ {result}.group_fast, %{fast} ], [ {result}.group_slow, %{prefix}_slow_op{last}_merge ]", llvm_type(ty)).unwrap();
        }
        Ok(())
    }
}
