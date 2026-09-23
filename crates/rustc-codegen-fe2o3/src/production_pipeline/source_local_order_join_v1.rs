//! Exact live-HIR -> semantic locals/statements -> retained KIR correspondence.
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::local_order::CapturedLocalOrder;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::U32LocalOrderRegionV1 as Region;

pub(super) struct JoinedLocalOrder<'a> {
    kir: &'a Function,
    block: &'a BasicBlock,
    operations: [u32; 3],
    #[cfg(test)]
    pub(super) results: [ValueId; 3],
}

pub(super) fn exact_join<'a>(
    captured: &mut CapturedLocalOrder<'_>,
    owner: &'a ProductionSemanticKirOwnerV1,
) -> Result<JoinedLocalOrder<'a>, String> {
    // Existing owner replay retains its own unchanged production limits.
    owner
        .verify_equivalence()
        .map_err(|e| format!("local-order source replay: {e}"))?;
    let semantic = owner.semantic().semantic();
    let [root] = semantic.roots() else {
        return Err("local-order one semantic root required".into());
    };
    let selected = semantic
        .select_kernel_body_v1()
        .ok_or("local-order source body unavailable")?;
    if selected.root() != *root
        || selected.body() != *root
        || semantic.functions().len() != 1
        || owner.module().functions.len() != 1
    {
        return Err("local-order direct single body required".into());
    }
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or("local-order semantic function unavailable")?;
    let identities = captured.identities;
    if function.identity() != identities.function()
        || function.item_definition_identity() != identities.item_definition()
        || function.monomorphization_identity() != identities.monomorphization()
        || function.generic_type_arguments_identity() != identities.generic_type_arguments()
        || function.const_generic_arguments_identity() != identities.const_generic_arguments()
        || function.role() != SemanticFunctionRoleV1::KernelRoot
    {
        return Err("local-order sealed Instance/semantic identity mismatch".into());
    }
    let correspondence = owner.correspondence();
    let mapped = unique(
        correspondence.lowered_functions(),
        &mut captured.meter,
        |row| row.correspondence_owner() == *root && row.semantic_function() == *root,
        "local-order function correspondence",
    )?;
    let kir = unique(
        &owner.module().functions,
        &mut captured.meter,
        |row| &row.id == mapped.kernel_ir_function(),
        "local-order KIR function",
    )?;
    let body = kir.body.as_ref().ok_or("local-order KIR body absent")?;
    let [kernel] = owner.module().kernels.as_slice() else {
        return Err("local-order kernel roster changed".into());
    };
    if kernel.entry != kir.id || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1)) {
        return Err("local-order exact launch mismatch".into());
    }
    let mut locals = [SemanticLocalIdV1::from_index(0); 4];
    let mut inputs = [ValueId(0); 4];
    for (index, parameter) in captured.parameters.iter().enumerate() {
        captured.meter.rows(function.locals().len())?;
        let mut selected = None;
        for (ordinal, local) in function.locals().iter().enumerate() {
            if local.role() == SemanticLocalRoleV1::Argument(parameter.ordinal)
                && selected.replace((ordinal, local)).is_some()
            {
                return Err("local-order ambiguous semantic input".into());
            }
        }
        let (ordinal, local) = selected.ok_or("local-order semantic input missing")?;
        if semantic
            .types()
            .get(local.ty().index() as usize)
            .map(|ty| ty.shape())
            != Some(&SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ))
        {
            return Err("local-order semantic input is not u32".into());
        }
        locals[index] = SemanticLocalIdV1::from_index(
            u32::try_from(ordinal).map_err(|_| "local-order local ordinal")?,
        );
        let binding = unique(
            correspondence.parameter_bindings(),
            &mut captured.meter,
            |row| {
                row.correspondence_owner() == *root
                    && row.semantic_function() == *root
                    && row.semantic_local() == locals[index]
            },
            "local-order input correspondence",
        )?;
        inputs[index] = binding.kernel_ir_value();
        captured.meter.rows(body.parameters.len())?;
        let mut count = 0;
        for (ordinal, input) in body.parameters.iter().enumerate() {
            if *input == inputs[index] {
                if kir.signature.parameters.get(ordinal) != Some(&Type::Scalar(ScalarType::U32)) {
                    return Err("local-order KIR parameter type changed".into());
                }
                count += 1;
            }
        }
        if count != 1 {
            return Err("local-order KIR input is not one formal parameter".into());
        }
    }
    let entry = function
        .blocks()
        .get(function.entry().index() as usize)
        .ok_or("local-order semantic entry missing")?;
    let mapped_entry = unique(
        correspondence.blocks(),
        &mut captured.meter,
        |row| {
            row.correspondence_owner() == *root
                && row.semantic_function() == *root
                && row.semantic_block() == function.entry()
        },
        "local-order entry correspondence",
    )?;
    let block = unique(
        &body.blocks,
        &mut captured.meter,
        |row| row.id == mapped_entry.kernel_ir_block(),
        "local-order KIR entry",
    )?;
    let mut destinations = [SemanticLocalIdV1::from_index(0); 3];
    let mut statements = [0u32; 3];
    let mut operations = [0u32; 3];
    let mut results = [ValueId(0); 3];
    for index in 0..3 {
        captured.meter.rows(entry.statements().len())?;
        let mut selected = None;
        for (ordinal, statement) in entry.statements().iter().enumerate() {
            if statement.source() == captured.operators[index]
                && let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::Binary { .. }
                )
                && selected.replace((ordinal, assignment)).is_some()
            {
                return Err("local-order ambiguous HIR/operator correspondence".into());
            }
        }
        let (ordinal, assignment) =
            selected.ok_or("local-order HIR/operator correspondence missing")?;
        let expected = match index {
            0 => (SemanticBinaryOpV1::BitXor, locals[0], locals[1]),
            1 => (SemanticBinaryOpV1::BitOr, locals[2], locals[3]),
            _ => (SemanticBinaryOpV1::BitAnd, destinations[0], destinations[1]),
        };
        let (opcode, left, right, destination) = binary(assignment)?;
        if (opcode, left, right) != expected {
            return Err("local-order semantic operand graph mismatch".into());
        }
        destinations[index] = destination;
        statements[index] = u32::try_from(ordinal).map_err(|_| "local-order statement ordinal")?;
        let span = unique(
            correspondence.statement_operation_spans(),
            &mut captured.meter,
            |row| {
                row.correspondence_owner() == *root
                    && row.semantic_function() == *root
                    && row.semantic_block() == function.entry()
                    && row.statement_ordinal() == statements[index]
            },
            "local-order exact statement span",
        )?;
        if span.kernel_ir_block() != block.id || span.operation_count() != 1 {
            return Err("local-order statement does not emit one exact operation".into());
        }
        operations[index] = span.first_operation_ordinal();
        let operation = block
            .operations
            .get(operations[index] as usize)
            .ok_or("local-order operation missing")?;
        let expected = match index {
            0 => (BinaryOp::BitXor, inputs[0], inputs[1]),
            1 => (BinaryOp::BitOr, inputs[2], inputs[3]),
            _ => (BinaryOp::BitAnd, results[0], results[1]),
        };
        let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
            return Err("local-order KIR operator is not binary".into());
        };
        let [result] = operation.results.as_slice() else {
            return Err("local-order KIR result arity".into());
        };
        if (op, lhs, rhs) != expected || result.ty != Type::Scalar(ScalarType::U32) {
            return Err("local-order exact KIR operand graph mismatch".into());
        }
        results[index] = result.id;
    }
    if operations[0].checked_add(1) != Some(operations[1])
        || operations[1].checked_add(1) != Some(operations[2])
        || !(statements[0] < statements[1] && statements[1] < statements[2])
    {
        return Err("local-order requires one contiguous source-ordered boundary".into());
    }
    let all = [
        inputs[0], inputs[1], inputs[2], inputs[3], results[0], results[1], results[2],
    ];
    for (index, value) in all.iter().enumerate() {
        captured.meter.scan(all.len() - index)?;
        if all[index + 1..].contains(value) {
            return Err("local-order seven SSA roles must be distinct".into());
        }
    }
    let mut census = Attribution {
        operations,
        counts: [0; 3],
    };
    captured
        .meter
        .rows(correspondence.statement_operation_spans().len())?;
    for row in correspondence.statement_operation_spans() {
        captured.meter.scan(8)?;
        if row.kernel_ir_block() == block.id {
            let index = if row.correspondence_owner() == *root
                && row.semantic_function() == *root
                && row.semantic_block() == function.entry()
            {
                statements
                    .iter()
                    .position(|ordinal| *ordinal == row.statement_ordinal())
            } else {
                None
            };
            census.visit(row.first_operation_ordinal(), row.operation_count(), index)?;
        }
    }
    captured
        .meter
        .rows(correspondence.terminator_operation_spans().len())?;
    for row in correspondence.terminator_operation_spans() {
        captured.meter.scan(8)?;
        if row.kernel_ir_block() == block.id {
            census.visit(row.first_operation_ordinal(), row.operation_count(), None)?;
        }
    }
    captured
        .meter
        .rows(correspondence.synthetic_operation_spans().len())?;
    for row in correspondence.synthetic_operation_spans() {
        captured.meter.scan(8)?;
        if row.kernel_ir_block() == block.id {
            census.visit(row.first_operation_ordinal(), row.operation_count(), None)?;
        }
    }
    census.finish()?;
    Ok(JoinedLocalOrder {
        kir,
        block,
        operations,
        #[cfg(test)]
        results,
    })
}

impl JoinedLocalOrder<'_> {
    pub(super) fn bound_region(
        &self,
        owner: &Owner,
        meter: &mut ScanMeter,
    ) -> Result<Region, String> {
        // Prepay the full structural comparison before scanning the target body.
        meter.scan(owner.canonical().canonical_bytes().len())?;
        meter.rows(owner.module().functions.len())?;
        let mut function = None;
        for (ordinal, candidate) in owner.module().functions.iter().enumerate() {
            if candidate.id == self.kir.id && function.replace((ordinal, candidate)).is_some() {
                return Err("local-order ambiguous bound function".into());
            }
        }
        let (function, candidate) = function.ok_or("local-order bound function missing")?;
        if candidate.signature != self.kir.signature || candidate.body != self.kir.body {
            return Err("local-order target binding changed the source function body".into());
        }
        let body = candidate
            .body
            .as_ref()
            .ok_or("local-order bound body missing")?;
        meter.rows(body.blocks.len())?;
        let mut block = None;
        for (ordinal, candidate) in body.blocks.iter().enumerate() {
            if candidate.id == self.block.id && block.replace(ordinal).is_some() {
                return Err("local-order ambiguous bound block".into());
            }
        }
        Ok(Region {
            expected_input: *owner.canonical().identity(),
            block: CanonicalKirBlockCoordinateV1 {
                function: CanonicalKirFunctionCoordinateV1(
                    u32::try_from(function).map_err(|_| "local-order function coordinate")?,
                ),
                block: u32::try_from(block.ok_or("local-order bound block missing")?)
                    .map_err(|_| "local-order block coordinate")?,
            },
            first_operation: self.operations[0],
            operation_count: 3,
        })
    }

    #[cfg(test)]
    pub(super) fn result_order(
        &self,
        owner: &Owner,
        region: Region,
    ) -> Result<[ValueId; 3], String> {
        let body = owner
            .module()
            .functions
            .get(region.block.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .ok_or("local-order scheduled function missing")?;
        let operations = &body
            .blocks
            .get(region.block.block as usize)
            .ok_or("local-order scheduled block missing")?
            .operations;
        let first = region.first_operation as usize;
        let selected = operations
            .get(first..first.checked_add(3).ok_or("local-order range overflow")?)
            .ok_or("local-order scheduled range missing")?;
        let mut result = [ValueId(0); 3];
        for (index, operation) in selected.iter().enumerate() {
            let [value] = operation.results.as_slice() else {
                return Err("local-order scheduled result arity changed".into());
            };
            result[index] = value.id;
        }
        Ok(result)
    }
}
