//! Independent actual-KIR scalar value derivation. This never reads a source
//! expression recipe. The enclosing context supplies exact checked call joins.
use super::native_helper_value_template_v1::{Kind, Meter, Template, Value, vector};
use super::*;
use fe2o3_kernel_ir::FunctionRole;

type Error = &'static str;
type Scalar = ProductionSemanticScalarTypeV2;

struct Binding<'a> {
    id: ValueId,
    ty: &'a Type,
    producer: Option<&'a Operation>,
    value: Option<Value>,
}

include!("native_helper_constant_shift_v1.rs");
include!("native_helper_masked_shift_v1.rs");
include!("native_helper_inline_value_v30.rs");

#[cfg(test)]
#[path = "native_helper_inline_value_v30_tests.rs"]
mod inline_tests;

fn lookup_work(length: usize, meter: &mut dyn Meter) -> Result<(), Error> {
    let depth = usize::BITS as usize - length.leading_zeros() as usize;
    meter.work(
        depth
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or("native helper lookup work overflow")?,
    )
}

fn find(rows: &[Binding<'_>], id: ValueId, meter: &mut dyn Meter) -> Result<usize, Error> {
    lookup_work(rows.len(), meter)?;
    rows.binary_search_by_key(&id, |row| row.id)
        .map_err(|_| "native helper missing SSA definition")
}

fn read(rows: &[Binding<'_>], id: ValueId, meter: &mut dyn Meter) -> Result<Value, Error> {
    rows[find(rows, id, meter)?]
        .value
        .ok_or("native helper use before definition or overflow flag")
}

fn bind(
    rows: &mut [Binding<'_>],
    id: ValueId,
    value: Value,
    template: &Template,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let index = find(rows, id, meter)?;
    let row = &mut rows[index];
    if row.value.is_some() || kir_semantic_scalar_v1(row.ty) != Some(template.scalar(value)?) {
        return Err("native helper SSA redefinition or scalar mismatch");
    }
    row.value = Some(value);
    Ok(())
}

fn scalar(rows: &[Binding<'_>], id: ValueId, meter: &mut dyn Meter) -> Result<Scalar, Error> {
    kir_semantic_scalar_v1(rows[find(rows, id, meter)?].ty)
        .ok_or("native helper non-scalar definition")
}

fn definition_count(function: &Function, meter: &mut dyn Meter) -> Result<usize, Error> {
    let body = function.body.as_ref().ok_or("native helper has no body")?;
    let mut count = body.parameters.len();
    for block in &body.blocks {
        meter.work(2)?;
        count = count
            .checked_add(block.parameters.len())
            .ok_or("native helper definition count overflow")?;
        for operation in &block.operations {
            meter.work(2)?;
            count = count
                .checked_add(operation.results.len())
                .ok_or("native helper definition count overflow")?;
        }
    }
    Ok(count)
}

fn operation<'a>(
    operation: &Operation,
    location: FunctionOperationLocation,
    rows: &mut [Binding<'_>],
    output: &mut Template,
    callee: &mut impl FnMut(
        FunctionOperationLocation,
        &Operation,
        &mut dyn Meter,
    ) -> Result<&'a Template, Error>,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    meter.work(8)?;
    let result = operation
        .results
        .first()
        .ok_or("native helper operation has no scalar result")?;
    let result_scalar = kir_semantic_scalar_v1(&result.ty)
        .ok_or("native helper result type outside scalar domain")?;
    let checked = matches!(
        operation.kind,
        OperationKind::Binary {
            op: BinaryOp::Checked(_),
            ..
        }
    );
    if operation.results.len() != if checked { 2 } else { 1 } {
        return Err("native helper result arity mismatch");
    }
    if checked && (operation.results[1].ty != Type::BOOL || !result_scalar.is_integer()) {
        return Err("native helper checked pair mismatch");
    }
    let value = match &operation.kind {
        OperationKind::InlineAssembly(_) => {
            native_helper_inline_value_v30(operation, rows, output, meter)?
        }
        OperationKind::Constant(constant) => {
            let (actual, bits) =
                normalize_kir_constant_v1(constant).ok_or("native helper constant unsupported")?;
            if actual != result_scalar {
                return Err("native helper constant type mismatch");
            }
            output.push(result_scalar, Kind::Constant(bits), meter)?
        }
        OperationKind::Unary { op, operand } => {
            let op = match (op, result_scalar) {
                (UnaryOp::Not, Scalar::Bool | Scalar::Integer { .. }) => {
                    ProductionSemanticUnaryOpV2::Not
                }
                (UnaryOp::Negate, Scalar::Float { bits: 32 }) => {
                    ProductionSemanticUnaryOpV2::Negate
                }
                _ => return Err("native helper unary has no closed total scalar rule"),
            };
            if scalar(rows, *operand, meter)? != result_scalar {
                return Err("native helper unary type mismatch");
            }
            let operand = read(rows, *operand, meter)?;
            output.push(result_scalar, Kind::Unary(op, operand), meter)?
        }
        OperationKind::Binary {
            op: op @ (BinaryOp::ShiftLeft | BinaryOp::ShiftRight),
            lhs,
            rhs,
        } => {
            if !matches!(
                result.ty,
                Type::Scalar(
                    ScalarType::I8
                        | ScalarType::U8
                        | ScalarType::I16
                        | ScalarType::U16
                        | ScalarType::I32
                        | ScalarType::U32
                        | ScalarType::I64
                        | ScalarType::U64
                )
            ) || rows[find(rows, *lhs, meter)?].ty != &result.ty
                || (!native_helper_constant_shift_v1(rows, *rhs, &result.ty, meter)?
                    && !native_helper_masked_shift_v1(rows, *rhs, &result.ty, meter)?)
            {
                return Err("native helper shift is neither an in-range literal nor an exact mask");
            }
            let (op, overflow) = normalize_kir_binary_v1(*op, operation, result.id)
                .ok_or("native helper shift value unsupported")?;
            let lhs = read(rows, *lhs, meter)?;
            let rhs = read(rows, *rhs, meter)?;
            output.push(result_scalar, Kind::Binary(op, overflow, lhs, rhs), meter)?
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let allowed = matches!(
                (op, result_scalar),
                (
                    BinaryOp::Checked(_) | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    Scalar::Integer { .. },
                ) | (
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    Scalar::Bool
                ) | (
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply,
                    Scalar::Float { .. }
                ) | (BinaryOp::Divide, Scalar::Float { bits: 32 })
            );
            if !allowed
                || scalar(rows, *lhs, meter)? != result_scalar
                || scalar(rows, *rhs, meter)? != result_scalar
            {
                return Err(
                    "native helper binary is partial, mistyped or outside closed scalar rules",
                );
            }
            let (op, overflow) = normalize_kir_binary_v1(*op, operation, result.id)
                .ok_or("native helper binary value unsupported")?;
            let lhs = read(rows, *lhs, meter)?;
            let rhs = read(rows, *rhs, meter)?;
            output.push(result_scalar, Kind::Binary(op, overflow, lhs, rhs), meter)?
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let input = scalar(rows, *lhs, meter)?;
            if result_scalar != Scalar::Bool || scalar(rows, *rhs, meter)? != input {
                return Err("native helper comparison type mismatch");
            }
            let lhs = read(rows, *lhs, meter)?;
            let rhs = read(rows, *rhs, meter)?;
            output.push(
                result_scalar,
                Kind::Compare(normalize_kir_comparison_v1(*predicate), input, lhs, rhs),
                meter,
            )?
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            if scalar(rows, *condition, meter)? != Scalar::Bool
                || scalar(rows, *true_value, meter)? != result_scalar
                || scalar(rows, *false_value, meter)? != result_scalar
            {
                return Err("native helper select type mismatch");
            }
            let condition = read(rows, *condition, meter)?;
            let yes = read(rows, *true_value, meter)?;
            let no = read(rows, *false_value, meter)?;
            output.push(result_scalar, Kind::Select(condition, yes, no), meter)?
        }
        OperationKind::Cast { kind, value, to } => {
            let source = scalar(rows, *value, meter)?;
            if !source.is_integer() || !result_scalar.is_integer() || to != &result.ty {
                return Err("native helper cast outside exact integer domain");
            }
            let kind = normalize_kir_cast_v1(*kind, source, result_scalar)
                .ok_or("native helper cast unsupported")?;
            let value = read(rows, *value, meter)?;
            if source == result_scalar {
                value
            } else {
                output.push(result_scalar, Kind::Cast(kind, source, value), meter)?
            }
        }
        OperationKind::Call { arguments, .. } => {
            let target = callee(location, operation, meter)?;
            let (mut values, storage) = vector(arguments.len(), meter)?;
            let result = (|| {
                for argument in arguments {
                    values.push(read(rows, *argument, meter)?);
                }
                output.append_call(target, &values, meter)
            })();
            drop(values);
            meter.release(storage)?;
            result?
        }
        _ => return Err("native helper has memory, ordering, execution or unknown effects"),
    };
    bind(rows, result.id, value, output, meter)
}

pub(super) fn derive<'a>(
    function: &Function,
    mut callee: impl FnMut(
        FunctionOperationLocation,
        &Operation,
        &mut dyn Meter,
    ) -> Result<&'a Template, Error>,
    meter: &mut dyn Meter,
) -> Result<Template, Error> {
    meter.work(5)?;
    if function.role != FunctionRole::InternalHelper || function.signature.results.len() != 1 {
        return Err("native value helper must have one internal scalar result");
    }
    let body = function.body.as_ref().ok_or("native helper is external")?;
    if body.parameters.len() != function.signature.parameters.len() {
        return Err("native helper parameter arity mismatch");
    }
    let returned = kir_semantic_scalar_v1(&function.signature.results[0])
        .ok_or("native helper result type unsupported")?;
    let (mut parameters, storage) = vector(function.signature.parameters.len(), meter)?;
    let result = (|| {
        for ty in &function.signature.parameters {
            meter.work(1)?;
            parameters.push(
                kir_semantic_scalar_v1(ty).ok_or("native helper parameter type unsupported")?,
            );
        }
        Template::new(&parameters, meter)
    })();
    drop(parameters);
    meter.release(storage)?;
    let mut output = result?;
    let result = derive_inner(function, returned, &mut output, &mut callee, meter);
    if let Err(error) = result {
        output.destroy(meter)?;
        return Err(error);
    }
    Ok(output)
}

fn derive_inner<'a>(
    function: &Function,
    returned: Scalar,
    output: &mut Template,
    callee: &mut impl FnMut(
        FunctionOperationLocation,
        &Operation,
        &mut dyn Meter,
    ) -> Result<&'a Template, Error>,
    meter: &mut dyn Meter,
) -> Result<(), Error> {
    let body = function.body.as_ref().ok_or("native helper is external")?;
    let count = definition_count(function, meter)?;
    let (mut rows, row_storage) = vector(count, meter)?;
    let (mut blocks, block_storage) = match vector(body.blocks.len(), meter) {
        Ok(value) => value,
        Err(error) => {
            drop(rows);
            meter.release(row_storage)?;
            return Err(error);
        }
    };
    let (mut seen, seen_storage) = match vector(body.blocks.len(), meter) {
        Ok(value) => value,
        Err(error) => {
            drop(rows);
            drop(blocks);
            meter.release(row_storage)?;
            meter.release(block_storage)?;
            return Err(error);
        }
    };
    seen.resize(body.blocks.len(), false);
    let result = (|| {
        for (id, ty) in body.parameters.iter().zip(&function.signature.parameters) {
            rows.push(Binding {
                id: *id,
                ty,
                producer: None,
                value: None,
            });
        }
        for (ordinal, block) in body.blocks.iter().enumerate() {
            blocks.push((block.id, ordinal));
            for parameter in &block.parameters {
                rows.push(Binding {
                    id: parameter.id,
                    ty: &parameter.ty,
                    producer: None,
                    value: None,
                });
            }
            for operation in &block.operations {
                for result in &operation.results {
                    rows.push(Binding {
                        id: result.id,
                        ty: &result.ty,
                        producer: Some(operation),
                        value: None,
                    });
                }
            }
        }
        if rows.len() != count {
            return Err("native helper definition census changed");
        }
        super::native_helper_value_template_v1::sort_metered(
            &mut rows,
            1,
            meter,
            |left, right| left.id.cmp(&right.id),
        )?;
        super::native_helper_value_template_v1::sort_metered(
            &mut blocks,
            1,
            meter,
            |left, right| left.0.cmp(&right.0),
        )?;
        meter.work(
            rows.len()
                .checked_add(blocks.len())
                .ok_or("native helper census work overflow")?,
        )?;
        if rows.windows(2).any(|pair| pair[0].id == pair[1].id)
            || blocks.windows(2).any(|pair| pair[0].0 == pair[1].0)
        {
            return Err("native helper duplicate definitions or blocks");
        }
        for row in &rows {
            if kir_semantic_scalar_v1(row.ty).is_none() {
                return Err("native helper non-scalar SSA definition");
            }
        }
        for (parameter, id) in body.parameters.iter().enumerate() {
            let ty = kir_semantic_scalar_v1(&function.signature.parameters[parameter])
                .ok_or("native helper parameter type unsupported")?;
            let value = output.push(ty, Kind::Parameter(parameter), meter)?;
            bind(&mut rows, *id, value, output, meter)?;
        }
        if body
            .blocks
            .first()
            .is_none_or(|block| !block.parameters.is_empty())
        {
            return Err("native helper entry block has missing argument transport");
        }
        let mut ordinal = 0;
        let mut visited = 0usize;
        loop {
            meter.work(4)?;
            if std::mem::replace(
                seen.get_mut(ordinal)
                    .ok_or("native helper edge outside body")?,
                true,
            ) {
                return Err("native helper control is cyclic");
            }
            visited += 1;
            let block = &body.blocks[ordinal];
            for (index, value) in block.operations.iter().enumerate() {
                operation(
                    value,
                    FunctionOperationLocation::new(block.id, index),
                    &mut rows,
                    output,
                    callee,
                    meter,
                )?;
            }
            match block
                .terminator
                .as_ref()
                .ok_or("native helper missing terminator")?
            {
                Terminator::Branch { target, arguments } => {
                    lookup_work(blocks.len(), meter)?;
                    let target = blocks
                        .binary_search_by_key(target, |row| row.0)
                        .map_err(|_| "native helper target outside body")?;
                    ordinal = blocks[target].1;
                    let destination = &body.blocks[ordinal];
                    if seen[ordinal] || destination.parameters.len() != arguments.len() {
                        return Err("native helper cyclic or mismatched branch transport");
                    }
                    // Read the whole edge before binding any destination. A
                    // later operand cannot refer to a parameter just assigned.
                    let (mut values, storage) = vector(arguments.len(), meter)?;
                    let transport = (|| {
                        for argument in arguments {
                            values.push(read(&rows, *argument, meter)?);
                        }
                        for (value, parameter) in values.iter().zip(&destination.parameters) {
                            bind(&mut rows, parameter.id, *value, output, meter)?;
                        }
                        Ok(())
                    })();
                    drop(values);
                    meter.release(storage)?;
                    transport?;
                }
                Terminator::Return { values } => {
                    if visited != body.blocks.len() || values.len() != 1 {
                        return Err("native helper has extra blocks or ambiguous return");
                    }
                    let value = read(&rows, values[0], meter)?;
                    output.finish(value, returned)?;
                    return Ok(());
                }
                _ => return Err("native helper control is conditional, trapping or non-returning"),
            }
        }
    })();
    drop(rows);
    drop(blocks);
    drop(seen);
    meter.release(row_storage)?;
    meter.release(block_storage)?;
    meter.release(seen_storage)?;
    result
}
